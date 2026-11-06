//! The identify queue: it controls when a shard can send Identify.
//!
//! Discord permits `max_concurrency` Identify payloads in each 5 seconds. Shard `n` is in
//! bucket `n % max_concurrency`. Each bucket permits one Identify every 5 seconds.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use cordial_http::transport::BoxFuture;
use tokio::sync::Mutex;
use tokio::time::Instant;

/// The time between two Identify payloads in one bucket.
pub const IDENTIFY_INTERVAL: Duration = Duration::from_secs(5);

/// Controls when a shard can identify.
///
/// The default [`LocalIdentifyQueue`] works in one process. Give your own queue (for example
/// with Redis) when shards run in more than one process.
pub trait IdentifyQueue: Send + Sync + fmt::Debug + 'static {
    /// Waits until the shard can identify.
    fn acquire(&self, shard_id: u32) -> BoxFuture<'static, ()>;
}

/// An identify queue in memory, for all shards of one process.
#[derive(Clone, Debug)]
pub struct LocalIdentifyQueue {
    buckets: Arc<Vec<Mutex<Option<Instant>>>>,
}

impl LocalIdentifyQueue {
    /// Makes a queue for a `max_concurrency` value from `GET /gateway/bot`.
    #[must_use]
    pub fn new(max_concurrency: u32) -> Self {
        let count = usize::try_from(max_concurrency.max(1)).unwrap_or(1);
        Self {
            buckets: Arc::new((0..count).map(|_| Mutex::new(None)).collect()),
        }
    }
}

impl Default for LocalIdentifyQueue {
    fn default() -> Self {
        Self::new(1)
    }
}

impl IdentifyQueue for LocalIdentifyQueue {
    fn acquire(&self, shard_id: u32) -> BoxFuture<'static, ()> {
        let buckets = Arc::clone(&self.buckets);
        Box::pin(async move {
            let index = usize::try_from(shard_id).unwrap_or(0) % buckets.len();
            let mut last = buckets[index].lock().await;
            if let Some(previous) = *last {
                tokio::time::sleep_until(previous + IDENTIFY_INTERVAL).await;
            }
            *last = Some(Instant::now());
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn spaces_identifies_in_one_bucket() {
        let queue = LocalIdentifyQueue::new(1);
        let start = Instant::now();

        queue.acquire(0).await;
        queue.acquire(1).await;
        queue.acquire(2).await;

        assert!(start.elapsed() >= Duration::from_secs(10));
    }

    #[tokio::test(start_paused = true)]
    async fn buckets_run_in_parallel() {
        let queue = LocalIdentifyQueue::new(4);
        let start = Instant::now();

        for shard in 0..4 {
            queue.acquire(shard).await;
        }

        assert!(start.elapsed() < Duration::from_secs(1));
    }
}
