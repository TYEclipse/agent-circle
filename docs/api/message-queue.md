# Message Queue API

Offline message queue backed by SQLite (`~/.agent-circle/messages.db`).

## `Queue`

```rust
use agent_circle::message_queue::Queue;
use std::path::Path;

let q = Queue::open(data_dir)?;

// Enqueue (auto-assigned TTL)
q.push_with_ttl("12D3KooW...", "Hello!", Some(ttl))?;

// Enqueue pending (with tracking)
q.push_pending(request_id, peer, from, content, ts, msg_id, ttl, seq, retries)?;

// Query
let msgs = q.pending_for("12D3KooW...")?;
for m in &msgs {
    println!("{}: {}", m.content, m.id);
}

// Stats
let (pending, delivered, failed) = q.stats()?;
println!("{} pending, {} delivered, {} failed", pending, delivered, failed);

// Maintenance
q.mark_delivered(msg_id)?;      // Mark as delivered
q.remove_pending(req_id)?;      // Remove from tracking
q.expire_before(timestamp)?;    // Purge expired
q.prune_delivered()?;           // Clean delivered records
q.expire_pending(timestamp)?;   // Purge stale pending
q.load_all_pending()?;          // Crash recovery: reload all tracked
```

## Schema

```
messages:
  id         INTEGER PRIMARY KEY AUTOINCREMENT
  peer       TEXT NOT NULL
  content    TEXT NOT NULL
  created_at INTEGER NOT NULL
  delivered  INTEGER DEFAULT 0
  expires_at INTEGER

pending:
  request_id INTEGER PRIMARY KEY
  peer       TEXT NOT NULL
  from_did   TEXT NOT NULL
  content    TEXT NOT NULL
  ts         INTEGER NOT NULL
  msg_id     INTEGER NOT NULL
  ttl        INTEGER NOT NULL
  seq        INTEGER NOT NULL
  retries    INTEGER DEFAULT 0
  created_at INTEGER NOT NULL
```

## Persistence Guarantees

- Queue survives daemon restart / crash
- Pending entries are reloaded and re-sent on startup (crash recovery)
- Expired messages are auto-skipped during recovery
- WAL mode for concurrent read/write safety
