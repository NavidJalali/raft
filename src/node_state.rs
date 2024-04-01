use std::collections::{HashMap, HashSet};

use crate::{log_entry::LogEntry, node_id::NodeId, node_role::NodeRole, term::Term};

#[derive(Debug)]
pub struct NodeState<A: Clone + Eq> {
    // Stable. Stored in durable storage.
    pub node_id: NodeId,
    pub current_term: Term,
    pub voted_for: Option<NodeId>,
    pub log: Vec<LogEntry<A>>,
    pub commit_length: u64,

    // Transient. Resets on recovery.
    pub current_role: NodeRole,
    pub current_leader: Option<NodeId>,
    pub votes_received: HashSet<NodeId>,
    pub sent_length: HashMap<NodeId, u64>,
    pub acked_length: HashMap<NodeId, u64>,
}

impl<A: Clone + Eq> NodeState<A> {
    /// Recover after a crash or restart. Stable state must be loaded from durable storage.
    pub fn recover(
        node_id: NodeId,
        current_term: Term,
        voted_for: Option<NodeId>,
        log: Vec<LogEntry<A>>,
        commit_length: u64,
    ) -> Self {
        Self {
            node_id,
            current_term,
            voted_for,
            log,
            commit_length,
            current_role: NodeRole::Follower,
            current_leader: None,
            votes_received: HashSet::new(),
            sent_length: HashMap::new(),
            acked_length: HashMap::new(),
        }
    }

    /// On the very first boot of the node.
    pub fn initialize(node_id: NodeId) -> Self {
        Self {
            node_id,
            current_term: Term(0),
            voted_for: None,
            log: Vec::new(),
            commit_length: 0,
            current_role: NodeRole::Follower,
            current_leader: None,
            votes_received: HashSet::new(),
            sent_length: HashMap::new(),
            acked_length: HashMap::new(),
        }
    }
}
