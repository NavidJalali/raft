use std::collections::HashSet;

use async_trait::async_trait;

use crate::{message::NodeToNodeMessage, node_id::NodeId, remote_node_ref::RemoteNodeRef};

#[async_trait]
pub trait Cluster<Data: Clone + Eq> {
    async fn send_message(&self, node_id: NodeId, message: NodeToNodeMessage<Data>);
    async fn nodes(&self) -> HashSet<NodeId>;
    async fn majority(&self) -> usize;
    async fn node_ref(&self, node_id: NodeId) -> Option<RemoteNodeRef>;
}

#[cfg(test)]
pub(crate) mod stub {
    use super::*;

    /// No-op cluster: nodes() is empty, sends are dropped. Useful for tests
    /// that exercise a single node in isolation (recovery, message handling).
    pub struct StubCluster;

    #[async_trait]
    impl<A: Clone + Eq + Send + Sync + 'static> Cluster<A> for StubCluster {
        async fn send_message(&self, _node_id: NodeId, _message: NodeToNodeMessage<A>) {}
        async fn nodes(&self) -> HashSet<NodeId> {
            HashSet::new()
        }
        async fn majority(&self) -> usize {
            1
        }
        async fn node_ref(&self, _node_id: NodeId) -> Option<RemoteNodeRef> {
            None
        }
    }
}
