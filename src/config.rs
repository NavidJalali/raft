use crate::time_window::TimeWindow;

#[derive(Debug, Copy, Clone)]
pub struct Config {
    pub election_time_window: TimeWindow,
    pub heartbeat_time_window: TimeWindow,
}
