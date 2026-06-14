# Reliability + Diagnostics API

## `PendingTracker`

Tracks in-flight messages waiting for ACK.

```rust
use agent_circle::reliability::{PendingTracker, MAX_RETRIES};

let mut tracker = PendingTracker::new();

// Start tracking a sent message
tracker.track(req_id, peer_id, from_did, content, ts, msg_id, ttl, seq);

// ACK received
if let Some(entry) = tracker.ack(&req_id) {
    println!("Delivered after {}ms", entry.created_at.elapsed().as_millis());
}

// Failure — check if retries remain
match tracker.fail(&req_id) {
    Some(entry) if entry.retries < MAX_RETRIES => {
        // Re-send with new request_id
        tracker.retrack(new_id, entry);
    }
    Some(entry) => {
        // MAX_RETRIES exhausted → queue for offline delivery
    }
    None => {}
}
```

## `SequenceTracker`

Per-peer message ordering with gap-tolerant buffers.

```rust
use agent_circle::sequence::SequenceTracker;

let mut seq = SequenceTracker::new();

// Process incoming messages
let ordered = seq.ingest(&peer_id, chat_request);
for msg in &ordered {
    // delivered in order, with gaps buffered
}

// Reset on new connection (peer may have restarted)
seq.reset_peer(&peer_id);
```

## `DedupFilter`

Prevent duplicate message processing.

```rust
use agent_circle::dedup::DedupFilter;

let mut dedup = DedupFilter::new();

if dedup.is_dup(msg_id) {
    // Skip — already seen
} else {
    // Process and auto-register
}
```

## `DiagCounters`

Atomic counters for message lifecycle statistics.

```rust
use agent_circle::diag::{DiagCounters, DiagSnapshot, format_snapshot};

let counters = DiagCounters::default();

// Increment at each lifecycle stage
counters.inc_sent();
counters.inc_acked();
counters.inc_retried();
counters.inc_failed();
counters.inc_queued();
counters.inc_duplicate();

// Snapshot
let snap: DiagSnapshot = counters.snapshot(
    pending_count,  // in-flight
    (queue_pending, queue_delivered, queue_failed),  // offline queue
    start_time,     // Instant::now()
);

println!("{}", format_snapshot(&snap));
// 📊 诊断 | 运行 120s | 发送 15 送达 14 (93.3%) 重试 1 失败 0 入队 0 重复 0 | 飞行中 0 队列:待0已0死0
```

## `DiagSnapshot` fields

```rust
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
```
