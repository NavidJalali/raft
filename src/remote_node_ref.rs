use crate::node_id::NodeId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteNodeRef {
    pub id: NodeId,
    pub host: String,
    pub port: u16,
}

impl RemoteNodeRef {
    pub fn new(id: NodeId, host: String, port: u16) -> Self {
        Self { id, host, port }
    }

    pub fn url(&self) -> String {
        format!("http://{}:{}/raft", self.host, self.port)
    }
}
