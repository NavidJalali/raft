use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

use crate::key_value_command::KeyValueCommand;
use async_trait::async_trait;

#[async_trait]
pub trait Store {
    async fn get(&self, key: String) -> Option<String>;
}

#[derive(Clone)]
pub struct KVApp {
    store: Arc<RwLock<HashMap<String, String>>>,
    pub sender: tokio::sync::mpsc::UnboundedSender<KeyValueCommand>,
}

impl KVApp {
    pub async fn new() -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let store = Arc::new(RwLock::new(HashMap::new()));
        let inner_store = store.clone();
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                match message {
                    KeyValueCommand::Get(_) => {}
                    KeyValueCommand::Put(k, v) => {
                        let mut store = inner_store.write().await;
                        store.insert(k, v);
                    }
                    KeyValueCommand::Delete(k) => {
                        let mut store = inner_store.write().await;
                        store.remove(&k);
                    }
                }
            }
        });
        Self {
            store: store,
            sender: tx,
        }
    }
}

#[async_trait]
impl Store for KVApp {
    async fn get(&self, key: String) -> Option<String> {
        let store = self.store.read().await;
        store.get(&key).cloned()
    }
}
