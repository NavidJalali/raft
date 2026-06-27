use std::collections::HashMap;

use tokio::{
  sync::{
    mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
    oneshot,
  },
  task::JoinHandle,
};
use tracing::info;

use crate::{
  cluster::Cluster,
  config::Config,
  local_node_ref::LocalNodeRef,
  log_entry::LogEntry,
  message::{
    LocalClientToNodeMessage, Message, NodeToNodeMessage, NodeToSelfMessage,
    Outcome,
  },
  node_id::NodeId,
  node_role::NodeRole,
  node_state::NodeState,
  persistence::{Checkpoint, Persistence},
  term::Term,
};

pub struct RaftNode<
  A: Clone + Eq + Send + Sync,
  C: Cluster<A>,
  Storage: Persistence<A>,
> {
  cluster: C,
  storage: Storage,
  config: Config,
  state: NodeState<A>,
  mailbox: UnboundedReceiver<Message<A>>,
  // Sender to own mailbox.
  self_ref: UnboundedSender<Message<A>>,
  // Join Handle to the election/heartbeat timer.
  timer: tokio::task::JoinHandle<()>,
  on_commit_promises: HashMap<usize, oneshot::Sender<Outcome>>,
  message_delivery_sender: UnboundedSender<A>,
}

impl<
  A: std::fmt::Debug + Clone + Eq + Send + Sync + 'static,
  C: Cluster<A> + Send + Sync + 'static,
  Storage: Persistence<A> + Send + Sync + 'static,
> RaftNode<A, C, Storage>
{
  pub async fn make(
    node_id: NodeId,
    config: Config,
    cluster: C,
    storage: Storage,
    message_delivery_queue: UnboundedSender<A>,
  ) -> LocalNodeRef<A> {
    let (sender, receiver) = unbounded_channel();
    let election_fiber = Self::election_timer(&config, sender.clone());

    let state = match storage
      .load()
      .await
      .expect("Failed to load state from storage")
    {
      Some(checkpoint) => {
        info!("Loaded state from storage: {:?}", checkpoint);
        let committed = checkpoint.commit_length.min(checkpoint.log.len());
        for log_entry in checkpoint.log[0..committed].iter() {
          info!("Replaying log entry: {:?}", log_entry);
          message_delivery_queue.send(log_entry.data.clone()).expect(
            "Failed to send log entry to delivery queue during recovery",
          )
        }
        NodeState::recover(
          checkpoint.node_id,
          checkpoint.current_term,
          checkpoint.voted_for,
          checkpoint.log,
          checkpoint.commit_length,
        )
      }
      None => {
        info!("No state found in storage");
        NodeState::initialize(node_id)
      }
    };

    let node = Self {
      config,
      state,
      cluster,
      storage,
      mailbox: receiver,
      self_ref: sender.clone(),
      timer: election_fiber,
      on_commit_promises: HashMap::new(),
      message_delivery_sender: message_delivery_queue,
    };

    tokio::spawn(node.start());

    LocalNodeRef::new(node_id, sender)
  }

  async fn checkpoint(&self) {
    let checkpoint = Checkpoint {
      node_id: self.state.node_id,
      current_term: self.state.current_term,
      voted_for: self.state.voted_for,
      log: self.state.log.clone(),
      commit_length: self.state.commit_length,
    };

    self
      .storage
      .save(checkpoint)
      .await
      .expect("Failed to save checkpoint");
  }

  fn reset_on_commit_promises(&mut self) {
    // Cancel all the promises.
    for (_, promise) in self.on_commit_promises.drain() {
      let _ = promise.send(Outcome::Failure("Leader changed.".to_string()));
    }

    // resize to 0
    self.on_commit_promises.clear();
  }

  fn election_timer(
    config: &Config,
    self_sender: UnboundedSender<Message<A>>,
  ) -> JoinHandle<()> {
    let delay = config.election_time_window.choose();
    tokio::spawn(async move {
      tokio::time::sleep(delay).await;
      self_sender
        .send(Message::NodeToSelf(NodeToSelfMessage::StartElection))
        .expect("Failed to send election message")
    })
  }

  fn log_replication_timer(
    config: &Config,
    self_sender: UnboundedSender<Message<A>>,
  ) -> JoinHandle<()> {
    let delay = config.heartbeat_time_window.choose();
    tokio::spawn(async move {
      tokio::time::sleep(delay).await;
      self_sender
        .send(Message::NodeToSelf(NodeToSelfMessage::Heartbeat))
        .expect("Failed to send heartbeat message");
    })
  }

  fn reset_election_timer(&mut self) {
    self.timer.abort();
    let self_ref = self.self_ref.clone();
    let fiber = Self::election_timer(&self.config, self_ref);
    self.timer = fiber;
  }

  fn reset_heartbeat_timer(&mut self) {
    self.timer.abort();
    let self_ref = self.self_ref.clone();
    let fiber = Self::log_replication_timer(&self.config, self_ref);
    self.timer = fiber;
  }

  async fn start(mut self) {
    while let Some(message) = self.mailbox.recv().await {
      self.process_message(message).await;
    }
  }

  async fn replicate_log(&self, follower: NodeId) {
    let prefix_length = *self
      .state
      .sent_length
      .get(&follower)
      .expect("Failed to find follower in sent_length map");

    let suffix = self.state.log[prefix_length..].to_vec();

    let prefix_term = if prefix_length > 0 {
      self.state.log[prefix_length - 1].term
    } else {
      Term::zero()
    };

    self
      .cluster
      .send_message(
        follower,
        NodeToNodeMessage::AppendEntriesRequest {
          node_id: self.state.node_id,
          current_term: self.state.current_term,
          prefix_length,
          prefix_term,
          leader_commit_length: self.state.commit_length,
          suffix,
        },
      )
      .await;
  }

  async fn append_entries(
    &mut self,
    prefix_length: usize,
    leader_commit_length: usize,
    suffix: Vec<LogEntry<A>>,
  ) {
    // First, if we already have some of the logs, we should check if they are the same.
    if !suffix.is_empty() && self.state.log.len() > prefix_length {
      let index =
        (self.state.log.len() - 1).min(prefix_length + suffix.len() - 1);

      if self.state.log[index].term != suffix[index - prefix_length].term {
        // We should truncate the log to be the same as the prefix length.
        self.state.log.truncate(prefix_length);
      }
    }

    // Append the suffix
    if prefix_length + suffix.len() > self.state.log.len() {
      self.state.log.extend(suffix);
    }

    // Update the commit length
    if leader_commit_length > self.state.commit_length {
      for i in self.state.commit_length..leader_commit_length {
        self.deliver_log_entry(i);
      }
      self.state.commit_length = leader_commit_length;
    }
    self.checkpoint().await;
  }

  fn deliver_log_entry(&mut self, index: usize) {
    let entry = self.state.log[index].clone();
    info!("Node {:?} is delivering {:?}", self.state.node_id, entry);
    if let Some(promise) = self.on_commit_promises.remove(&index) {
      let _ = promise.send(Outcome::Success);
    }
    self
      .message_delivery_sender
      .send(entry.data)
      .expect("Failed to send delivered log entry to delivery queue");
  }

  async fn commit_log_entries(&mut self) {
    while self.state.commit_length < self.state.log.len() {
      let acks = self.cluster.nodes().await.iter().fold(0, |acc, node| {
        if *self.state.acked_length.get(node).unwrap()
          >= self.state.commit_length
        {
          acc + 1
        } else {
          acc
        }
      });

      if acks >= self.cluster.majority().await {
        self.deliver_log_entry(self.state.commit_length);
        self.state.commit_length += 1;
      } else {
        break;
      }
    }
    self.checkpoint().await;
  }

  async fn process_message(&mut self, message: Message<A>) {
    match message {
      Message::NodeToNode(NodeToNodeMessage::VoteRequest {
        candidate_node_id,
        candidate_current_term,
        candidate_log_length,
        candidate_last_log_term,
      }) => {
        if candidate_node_id == self.state.node_id {
          return;
        }

        if candidate_current_term > self.state.current_term {
          self.state.current_term = candidate_current_term;
          self.state.current_role = NodeRole::Follower;
          self.state.current_leader = None;
          self.state.voted_for = None;
          self.reset_on_commit_promises();
          self.reset_election_timer();
          self.checkpoint().await;
        }

        let last_term = self
          .state
          .log
          .last()
          .map(|m| m.term)
          .unwrap_or(Term::zero());

        let log_ok = (candidate_last_log_term >= last_term)
          || (candidate_last_log_term == last_term
            && candidate_log_length >= self.state.log.len());

        let voted_ok = self.state.voted_for.is_none()
          || self.state.voted_for == Some(candidate_node_id);

        let term_ok = candidate_current_term == self.state.current_term;

        let vote_granted = log_ok && voted_ok && term_ok;

        if vote_granted {
          self.state.voted_for = Some(candidate_node_id);
          self.checkpoint().await;
        }

        self
          .cluster
          .send_message(
            candidate_node_id,
            NodeToNodeMessage::VoteResponse {
              node_id: self.state.node_id,
              current_term: self.state.current_term,
              vote_granted,
            },
          )
          .await;
      }
      Message::NodeToNode(NodeToNodeMessage::VoteResponse {
        node_id,
        current_term,
        vote_granted,
      }) => {
        let is_candidate = self.state.current_role == NodeRole::Candidate;
        let term_ok = self.state.current_term >= current_term;

        if is_candidate && term_ok && vote_granted {
          self.state.votes_received.insert(node_id);

          let quarum = self.cluster.majority().await;

          if self.state.votes_received.len() >= quarum {
            self.state.current_role = NodeRole::Leader;
            self.state.current_leader = Some(self.state.node_id);
            self.reset_heartbeat_timer();

            info!(
              "{:?} is the leader in {:?}",
              self.state.node_id, self.state.current_term
            );

            self.checkpoint().await;

            for follower in self.cluster.nodes().await {
              if follower != self.state.node_id {
                self
                  .state
                  .sent_length
                  .insert(follower, self.state.log.len());

                self.state.acked_length.insert(follower, 0);
                self.replicate_log(follower).await;
              }
            }
          }
        } else if current_term > self.state.current_term {
          self.state.current_term = current_term;
          self.state.current_role = NodeRole::Follower;
          self.state.current_leader = None;
          self.state.voted_for = None;
          self.reset_on_commit_promises();
          self.reset_election_timer();
          self.checkpoint().await;
        }
      }
      Message::NodeToNode(NodeToNodeMessage::AppendEntriesRequest {
        node_id,
        current_term,
        prefix_length,
        prefix_term,
        leader_commit_length,
        suffix,
      }) => {
        if current_term > self.state.current_term {
          self.state.current_term = current_term;
          self.state.voted_for = None;
          // Will always fall through to the bottom if condition
        }

        if self.state.current_term == current_term {
          self.state.current_role = NodeRole::Follower;
          self.state.current_leader = Some(node_id);
          self.reset_on_commit_promises();
          self.reset_election_timer();
          self.checkpoint().await;
        }

        // We should check if that we have the prefix the leader assumed we do. I.e there are no gaps in the log.
        let log_length_is_at_least_prefix_length =
          self.state.log.len() >= prefix_length;

        // Raft guarantees that if the prefix term is the same, the log is the same up to the prefix.
        // Basically this is an efficient way to check if the logs are the same up to the prefix.
        let prefix_term_ok = (prefix_length == 0)
          || (prefix_length > 0
            && self.state.log[prefix_length - 1].term == prefix_term);

        let log_ok = log_length_is_at_least_prefix_length && prefix_term_ok;

        if self.state.current_term == current_term && log_ok {
          let suffix_length = suffix.len();
          self
            .append_entries(prefix_length, leader_commit_length, suffix)
            .await;
          let acked_length = prefix_length + suffix_length;
          self.checkpoint().await;
          let accept = NodeToNodeMessage::AppendEntriesResponse {
            node_id: self.state.node_id,
            current_term: self.state.current_term,
            acked_length,
            success: true,
          };
          self.cluster.send_message(node_id, accept).await;
        } else {
          let rejection = NodeToNodeMessage::AppendEntriesResponse {
            node_id: self.state.node_id,
            current_term: self.state.current_term,
            acked_length: 0,
            success: false,
          };
          self.cluster.send_message(node_id, rejection).await;
        }
      }
      Message::NodeToNode(NodeToNodeMessage::AppendEntriesResponse {
        node_id,
        current_term,
        acked_length,
        success,
      }) => {
        let is_leader = self.state.current_role == NodeRole::Leader;
        if self.state.current_term == current_term && is_leader {
          if success
            && acked_length >= *self.state.acked_length.get(&node_id).unwrap()
          {
            self.state.sent_length.insert(node_id, acked_length);
            self.state.acked_length.insert(node_id, acked_length);
            self.commit_log_entries().await;
          } else if *self.state.sent_length.get(&node_id).unwrap() > 0 {
            // If the follower rejected the entries, we should decrement the sent length.
            // This is inefficient, but it is the simplest way to handle this.
            self.state.sent_length.insert(
              node_id,
              *self.state.sent_length.get(&node_id).unwrap() - 1,
            );
          }
        } else if current_term > self.state.current_term {
          self.state.current_term = current_term;
          self.state.current_role = NodeRole::Follower;
          self.state.current_leader = None;
          self.state.voted_for = None;
          self.reset_on_commit_promises();
          self.reset_election_timer();
          self.checkpoint().await;
        }
      }
      // Client
      Message::LocalClientToNode(LocalClientToNodeMessage::Broadcast {
        entry,
        on_commit,
      }) => {
        if self.state.current_role == NodeRole::Leader {
          // Append to log
          let log_entry = LogEntry {
            data: entry.clone(),
            term: self.state.current_term,
          };

          self.state.log.push(log_entry);

          let index = self.state.log.len() - 1;

          // Register the promise
          self.on_commit_promises.insert(index, on_commit);

          // self acknowledge the entry
          self
            .state
            .acked_length
            .insert(self.state.node_id, self.state.log.len());

          self
            .state
            .sent_length
            .insert(self.state.node_id, self.state.log.len());

          self.checkpoint().await;

          // replicate to followers
          for follower in self.cluster.nodes().await {
            if follower != self.state.node_id {
              self.replicate_log(follower).await;
            }
          }
        } else {
          match self.state.current_leader {
            Some(leader) => {
              let current_term = self.state.current_term;
              info!(
                "Not the leader. Current leader is {:?} in term {:?}",
                leader, current_term
              );
              let _ = on_commit.send(Outcome::Redirect(leader));
            }
            None => {
              let _ =
                on_commit.send(Outcome::Failure("Not the leader".to_string()));
            }
          }
        }
      }
      // Internal
      Message::NodeToSelf(NodeToSelfMessage::StartElection) => {
        self.state.current_term = self.state.current_term.increment();
        self.state.current_role = NodeRole::Candidate;
        self.state.voted_for = Some(self.state.node_id);
        self.state.votes_received.insert(self.state.node_id);
        self.reset_on_commit_promises();
        let last_term = self
          .state
          .log
          .last()
          .map(|m| m.term)
          .unwrap_or(Term::zero());

        self.checkpoint().await;

        for node in self.cluster.nodes().await {
          if node != self.state.node_id {
            self
              .cluster
              .send_message(
                node,
                NodeToNodeMessage::VoteRequest {
                  candidate_node_id: self.state.node_id,
                  candidate_current_term: self.state.current_term,
                  candidate_log_length: self.state.log.len(),
                  candidate_last_log_term: last_term,
                },
              )
              .await;
          }
        }

        self.reset_election_timer();
      }
      Message::NodeToSelf(NodeToSelfMessage::Heartbeat) => {
        for follower in self.cluster.nodes().await {
          if follower != self.state.node_id {
            self.replicate_log(follower).await;
          }
          self.reset_heartbeat_timer()
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    cluster::stub::StubCluster, persistence::in_memory::InMemoryPersistence,
    time_window::TimeWindow,
  };
  use std::time::Duration;

  fn entry(term: u64, data: &str) -> LogEntry<String> {
    LogEntry {
      data: data.to_string(),
      term: Term(term),
    }
  }

  #[tokio::test]
  async fn recovery_only_replays_committed_log_entries() {
    // Log has 4 entries, but only the first 2 are committed. The bug
    // replayed all 4 — only "a" and "b" should be delivered.
    let checkpoint = Checkpoint {
      node_id: NodeId(1),
      current_term: Term(2),
      voted_for: None,
      log: vec![entry(1, "a"), entry(1, "b"), entry(2, "c"), entry(2, "d")],
      commit_length: 2,
    };

    // Long timer windows so neither election nor heartbeat fires during the test.
    let config = Config {
      election_time_window: TimeWindow::new(
        Duration::from_secs(60),
        Duration::from_secs(60),
      ),
      heartbeat_time_window: TimeWindow::new(
        Duration::from_secs(60),
        Duration::from_secs(60),
      ),
    };

    let (delivery_tx, mut delivery_rx) = unbounded_channel::<String>();

    let _node_ref = RaftNode::make(
      NodeId(1),
      config,
      StubCluster,
      InMemoryPersistence::with(checkpoint),
      delivery_tx,
    )
    .await;

    let mut delivered = Vec::new();
    loop {
      match tokio::time::timeout(Duration::from_millis(50), delivery_rx.recv())
        .await
      {
        Ok(Some(v)) => delivered.push(v),
        Ok(None) => break,
        Err(_) => break,
      }
    }

    assert_eq!(delivered, vec!["a".to_string(), "b".to_string()]);
  }
}
