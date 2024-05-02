use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::{log_entry::LogEntry, node_id::NodeId, term::Term};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NodeToNodeMessage<Data: Clone + Eq> {
    VoteRequest {
        candidate_node_id: NodeId,
        candidate_current_term: Term,
        candidate_log_length: u64,
        candidate_last_log_term: Term,
    },
    VoteResponse {
        node_id: NodeId,
        current_term: Term,
        vote_granted: bool,
    },
    AppendEntriesRequest {
        node_id: NodeId,
        current_term: Term,
        prefix_length: u64,
        prefix_term: Term,
        leader_commit_length: u64,
        suffix: Vec<LogEntry<Data>>,
    },
    AppendEntriesResponse {
        node_id: NodeId,
        current_term: Term,
        acked_length: u64,
        success: bool,
    },
}

#[derive(Debug)]
pub enum NodeToSelfMessage {
    StartElection,
    Heartbeat,
}

#[derive(Debug)]
pub enum Outcome {
    Success,
    Redirect(NodeId),
    Failure(String),
}

#[derive(Debug)]
pub enum LocalClientToNodeMessage<Data: Clone + Eq> {
    Broadcast {
        entry: Data,
        on_commit: oneshot::Sender<Outcome>,
    },
}

#[derive(Debug)]
pub enum Message<Data: Clone + Eq> {
    NodeToNode(NodeToNodeMessage<Data>),
    NodeToSelf(NodeToSelfMessage),
    LocalClientToNode(LocalClientToNodeMessage<Data>),
}
