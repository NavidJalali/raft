use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyValueCommand {
    Put(String, String),
    Get(String),
    Delete(String),
}
