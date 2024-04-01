use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use rand::Rng;
use tokio::sync::RwLock;
use tracing::debug;

use crate::{
    cluster::Cluster,
    local_node_ref::LocalNodeRef,
    message::{Message, NodeToNodeMessage},
    node_id::NodeId,
};

pub struct LocalCluster<A: Clone + Eq> {
    nodes: RwLock<HashMap<NodeId, LocalNodeRef<A>>>,
}

impl<A: Clone + Eq> LocalCluster<A> {
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
        }
    }

    pub async fn register(&self, node: &LocalNodeRef<A>) {
        self.nodes.write().await.insert(node.id, node.clone());
    }

    fn delay() -> std::time::Duration {
        let mut rng = rand::thread_rng();
        let arbitrary_delay = rng.gen_range(0..1000);
        std::time::Duration::from_millis(arbitrary_delay)
    }
}

#[async_trait]
impl<A: Clone + Eq + std::fmt::Debug + Send + Sync + 'static> Cluster<A> for LocalCluster<A> {
    async fn send_message(&self, node_id: NodeId, message: NodeToNodeMessage<A>) {
        let delay = Self::delay();
        debug!("Sending {:?} to {:?} after {:?}", message, node_id, delay);
        tokio::time::sleep(delay).await;
        match self.nodes.read().await.get(&node_id) {
            Some(node) => node.offer(Message::NodeToNode(message)),
            None => panic!("Node {:?} not found", node_id),
        }
    }

    async fn nodes(&self) -> HashSet<NodeId> {
        self.nodes.read().await.keys().cloned().collect()
    }

    async fn majority(&self) -> usize {
        self.nodes.read().await.len() / 2 + 1
    }
}
