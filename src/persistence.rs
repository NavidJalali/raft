use crate::{log_entry::LogEntry, node_id::NodeId, term::Term};
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Checkpoint<A: Clone + Eq> {
    pub node_id: NodeId,
    pub current_term: Term,
    pub voted_for: Option<NodeId>,
    pub log: Vec<LogEntry<A>>,
    pub commit_length: u64,
}

#[async_trait]
pub trait Persistence<A: Clone + Eq> {
    async fn save(&self, checkpoint: Checkpoint<A>) -> Result<(), String>;
    async fn load(&self) -> Result<Option<Checkpoint<A>>, String>;
}

pub struct FileOnDiskPersistence {
    node_id: NodeId,
}

impl FileOnDiskPersistence {
    pub fn new(node_id: NodeId) -> Self {
        Self { node_id }
    }

    fn path(&self) -> String {
        format!("{}.json", self.node_id.0)
    }
}

#[async_trait]
impl<A: Clone + Eq + Serialize + Send + Sync + 'static + DeserializeOwned> Persistence<A>
    for FileOnDiskPersistence
{
    async fn save(&self, checkpoint: Checkpoint<A>) -> Result<(), String> {
        tokio::fs::write(self.path(), serde_json::to_string(&checkpoint).unwrap())
            .await
            .map_err(|e| e.to_string())
    }

    async fn load(&self) -> Result<Option<Checkpoint<A>>, String> {
        match tokio::fs::read(self.path()).await {
            Ok(bytes) => {
                let checkpoint: Checkpoint<A> = serde_json::from_slice(&bytes).unwrap();
                Ok(Some(checkpoint))
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(None)
                } else {
                    Err(e.to_string())
                }
            }
        }
    }
}
