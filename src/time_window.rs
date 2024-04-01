use rand::Rng;

#[derive(Debug, Copy, Clone)]
pub struct TimeWindow {
    min_duration: std::time::Duration,
    max_duration: std::time::Duration,
}

impl TimeWindow {
    pub fn new(min_duration: std::time::Duration, max_duration: std::time::Duration) -> Self {
        Self {
            min_duration,
            max_duration,
        }
    }
    pub fn choose(&self) -> std::time::Duration {
        let mut rnd = rand::thread_rng();
        let range = self.min_duration..=self.max_duration;
        rnd.gen_range(range)
    }
}
