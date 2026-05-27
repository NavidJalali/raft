use tokio::sync::mpsc::UnboundedSender;

use crate::{message::Message, node_id::NodeId};

#[derive(Clone, Debug)]
pub struct LocalNodeRef<A: Clone + Eq> {
  #[allow(unused)]
  pub id: NodeId,
  mailbox: UnboundedSender<Message<A>>,
}

impl<A: Clone + Eq> LocalNodeRef<A> {
  pub fn new(id: NodeId, mailbox: UnboundedSender<Message<A>>) -> Self {
    Self { id, mailbox }
  }
  pub fn offer(&self, message: Message<A>) {
    self
      .mailbox
      .send(message)
      .expect("Failed to send message to node mailbox")
  }
}
