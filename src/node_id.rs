use serde::{Deserialize, Serialize};

#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub struct NodeId(pub u32);
