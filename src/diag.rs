//! Full-chain diagnostics — message lifecycle counters and stats.
//!
//! Every state transition increments a counter.  Aggregate stats are
//! logged every 30 seconds (or on SIGUSR2) so operators can monitor
//! delivery health without external tooling.
//!
//! Per-message tracing is handled by structured logging — every event
//! already carries `msg_id` and `peer` in the log output.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Cheap atomic counters — safe to increment from the event loop.
#[derive(Default)]
pub struct DiagCounters {
    pub sent: AtomicU64,
    pub acked: AtomicU64,
    pub retried: AtomicU64,
    pub failed: AtomicU64,
    pub queued: AtomicU64,
    pub duplicate: AtomicU64,
}

/// Snapshot of counters at a point in time.
#[derive(Debug, Clone, Default)]
pub struct DiagSnapshot {
    pub sent: u64,
    pub acked: u64,
    pub retried: u64,
    pub failed: u64,
    pub queued: u64,
    pub duplicate: u64,
    pub pending: usize,
    pub queue_pending: i64,
    pub queue_delivered: i64,
    pub queue_failed: i64,
    pub elapsed: Duration,
}

impl DiagCounters {
    pub fn snapshot(
        &self,
        pending: usize,
        queue: (i64, i64, i64),
        started: Instant,
    ) -> DiagSnapshot {
        DiagSnapshot {
            sent: self.sent.load(Ordering::Relaxed),
            acked: self.acked.load(Ordering::Relaxed),
            retried: self.retried.load(Ordering::Relaxed),
            failed: self.failed.load(Ordering::Relaxed),
            queued: self.queued.load(Ordering::Relaxed),
            duplicate: self.duplicate.load(Ordering::Relaxed),
            pending,
            queue_pending: queue.0,
            queue_delivered: queue.1,
            queue_failed: queue.2,
            elapsed: started.elapsed(),
        }
    }

    pub fn inc_sent(&self) {
        self.sent.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_acked(&self) {
        self.acked.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_retried(&self) {
        self.retried.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_failed(&self) {
        self.failed.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_queued(&self) {
        self.queued.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_duplicate(&self) {
        self.duplicate.fetch_add(1, Ordering::Relaxed);
    }
}

/// Render a snapshot as a human-readable summary line.
pub fn format_snapshot(s: &DiagSnapshot) -> String {
    let elapsed = s.elapsed.as_secs();
    let ack_rate = if s.sent > 0 {
        (s.acked as f64 / s.sent as f64) * 100.0
    } else {
        100.0
    };
    format!(
        "📊 诊断 | 运行 {}s | 发送 {} 送达 {} ({:.1}%) 重试 {} 失败 {} 入队 {} 重复 {} | \
         飞行中 {} 队列:待{}已{}死{}",
        elapsed,
        s.sent,
        s.acked,
        ack_rate,
        s.retried,
        s.failed,
        s.queued,
        s.duplicate,
        s.pending,
        s.queue_pending,
        s.queue_delivered,
        s.queue_failed,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_formatting() {
        let snap = DiagSnapshot {
            sent: 100,
            acked: 95,
            retried: 5,
            failed: 2,
            queued: 2,
            duplicate: 1,
            pending: 3,
            queue_pending: 2,
            queue_delivered: 10,
            queue_failed: 1,
            elapsed: std::time::Duration::from_secs(3600),
        };
        let s = format_snapshot(&snap);
        assert!(s.contains("100"));
        assert!(s.contains("95.0%"));
        assert!(s.contains("3600s"));
    }
}
