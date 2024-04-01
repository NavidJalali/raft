use std::collections::{HashMap, HashSet};

use axum::async_trait;
use serde::Serialize;
use tokio::sync::mpsc::UnboundedSender;
use tracing::error;

use crate::{
    cluster::Cluster, message::NodeToNodeMessage, node_id::NodeId, remote_node_ref::RemoteNodeRef,
};

#[derive(Clone)]
pub struct StaticCluster<A: Clone + Eq> {
    pub nodes: HashMap<NodeId, (RemoteNodeRef, UnboundedSender<NodeToNodeMessage<A>>)>,
}

impl<A: Clone + Eq + Serialize + Send + Sync + 'static> StaticCluster<A> {
    pub fn new(refs: HashMap<NodeId, RemoteNodeRef>) -> Self {
        let http_client = reqwest::Client::new();
        let nodes = refs
            .into_iter()
            .map(|(node_id, remote_ref)| {
                let client = http_client.clone();
                let url = remote_ref.url();
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                tokio::spawn(async move {
                    while let Some(message) = rx.recv().await {
                        let response = client.post(&url).json(&message).send().await;
                        match response {
                            Ok(response) => {
                                if !response.status().is_success() {
                                    error!(
                                        "Error sending message to {}: {}",
                                        url,
                                        response.status()
                                    )
                                }
                            }
                            Err(e) => {
                                error!("Error sending message to {}: {}", url, e)
                            }
                        }
                    }
                });
                (node_id.clone(), (remote_ref.clone(), tx))
            })
            .collect();
        Self { nodes }
    }
}

#[async_trait]
impl<A: Clone + Eq + Send + Sync> Cluster<A> for StaticCluster<A> {
    async fn send_message(&self, node_id: NodeId, message: NodeToNodeMessage<A>) {
        match self.nodes.get(&node_id) {
            Some((_, tx)) => {
                tx.send(message).unwrap();
            }
            None => {
                error!("Node {:?} not found", node_id);
            }
        }
    }

    async fn nodes(&self) -> HashSet<NodeId> {
        self.nodes.keys().cloned().collect()
    }

    async fn majority(&self) -> usize {
        self.nodes.len() / 2 + 1
    }
}
