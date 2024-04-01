#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeRole {
    Follower,
    Candidate,
    Leader,
}
