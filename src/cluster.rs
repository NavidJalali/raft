use std::collections::HashSet;

use async_trait::async_trait;

use crate::{message::NodeToNodeMessage, node_id::NodeId};

#[async_trait]
pub trait Cluster<Data: Clone + Eq> {
    async fn send_message(&self, node_id: NodeId, message: NodeToNodeMessage<Data>);
    async fn nodes(&self) -> HashSet<NodeId>;
    async fn majority(&self) -> usize;
}
