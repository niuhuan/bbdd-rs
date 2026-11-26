//! 下载和合并的进度与取消。
//! 调用方拿着 `Receiver` 画界面，拿着 `CancellationToken` 停止。库不打印进度条。

use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    Download,
    Merge,
}

#[derive(Debug, Clone)]
pub struct Update {
    pub stage: Stage,
    pub name: String,
    pub done: u64,
    pub total: Option<u64>,
}

#[derive(Clone)]
pub struct Control {
    pub cancel: CancellationToken,
    tx: mpsc::Sender<Update>,
}

impl Control {
    pub fn channel(cancel: CancellationToken) -> (Self, mpsc::Receiver<Update>) {
        let (tx, rx) = mpsc::channel(64);
        (Self { cancel, tx }, rx)
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    pub async fn cancelled(&self) {
        self.cancel.cancelled().await;
    }

    pub fn emit(&self, update: Update) {
        let _ = self.tx.try_send(update);
    }

    pub async fn emit_final(&self, update: Update) {
        let _ = self.tx.send(update).await;
    }

    pub fn emit_final_blocking(&self, update: Update) {
        let _ = self.tx.blocking_send(update);
    }
}

pub(crate) struct Throttle {
    interval: Duration,
    last: Option<Instant>,
}

impl Throttle {
    pub(crate) fn new(interval: Duration) -> Self {
        Self {
            interval,
            last: None,
        }
    }

    pub(crate) fn allow(&mut self, force: bool) -> bool {
        if force {
            self.last = Some(Instant::now());
            return true;
        }
        let now = Instant::now();
        if self.last.is_some_and(|last| now.duration_since(last) < self.interval) {
            return false;
        }
        self.last = Some(now);
        true
    }
}

pub(crate) fn update(stage: Stage, name: &str, done: u64, total: Option<u64>) -> Update {
    Update {
        stage,
        name: name.to_string(),
        done,
        total,
    }
}

#[cfg(test)]
mod tests {
    use super::{Throttle, PROGRESS_INTERVAL};
    use std::time::Duration;

    #[test]
    fn throttle_keeps_the_first_sample_and_drops_the_burst() {
        let mut throttle = Throttle::new(PROGRESS_INTERVAL);
        assert!(throttle.allow(true));
        assert!(!throttle.allow(false));
        assert!(throttle.allow(true));
    }

    #[test]
    fn throttle_allows_a_sample_after_the_interval() {
        let mut throttle = Throttle::new(Duration::from_millis(0));
        assert!(throttle.allow(false));
        assert!(throttle.allow(false));
    }
}
