use serde::{Deserialize, Serialize};

use crate::term::Term;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LogEntry<T: Sized + Clone + Eq> {
    pub data: T,
    pub term: Term,
}
