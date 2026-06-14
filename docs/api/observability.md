# Observability API

## Metrics

OpenMetrics/Prometheus exporter. Zero new dependencies.

```rust
use agent_circle::metrics::{collect, collect_for_dir};

// CLI mode
let text = collect()?;
print!("{}", text);

// Daemon-side (with explicit data dir)
let text = collect_for_dir(data_dir)?;
```

**Exposed metrics** (15+ gauges):

| Metric | Description |
|--------|-------------|
| `agent_circle_info` | Version info (always 1) |
| `agent_circle_daemon_up` | Daemon running (1=up) |
| `agent_circle_storage_size_bytes` | Data directory size |
| `agent_circle_storage_{file}_present` | card/identity/contacts/timeline/services |
| `agent_circle_contacts_count` | Contact count |
| `agent_circle_timeline_posts` | Post count |
| `agent_circle_timeline_verified` | Merkle-DAG verified (1=yes) |
| `agent_circle_services_peers` | Peer count in registry |
| `agent_circle_services_total` | Service count |
| `agent_circle_queue_pending` | Offline queue pending |
| `agent_circle_queue_delivered` | Offline queue delivered |
| `agent_circle_queue_failed` | Offline queue failed |
| `agent_circle_queue_delivery_rate` | Delivery % |

## Health Server

HTTP server bound to `127.0.0.1:9099` inside the daemon process.

```rust
use agent_circle::health::spawn;

// Start inside daemon startup
let addr = health::spawn(data_dir.clone(), id.short_code.clone()).await?;
tracing::info!(%addr, "Health server started");
```

**Endpoints**:

```bash
# JSON health status
curl http://127.0.0.1:9099/health
# {"status":"ok","daemon":"running","peer_id":"fb41e829","checks":{...},"stats":{...}}

# OpenMetrics text
curl http://127.0.0.1:9099/metrics
# # HELP agent_circle_info Version and host info
# # TYPE agent_circle_info gauge
# agent_circle_info 1
# ...
# # EOF
```

## Crash Dump

Automatic panic → structured JSON dump.

```rust
use agent_circle::crash;

// Install early in main()
crash::init(data_dir)?;
```

**Dump format** (`~/.agent-circle/crash/<iso8601>.dump`):

```json
{
  "crash": {
    "timestamp": "2026-06-14T12:00:00.123Z",
    "message": "panicked at ...",
    "location": "src/network.rs:123:45"
  },
  "system": {
    "os": "linux x86_64",
    "hostname": "my-host",
    "pid": 12345
  },
  "backtrace": "...",
  "agent_state": {
    "identity": {"exists": true},
    "card": {"name": "...", ...},
    "contacts": {"count": 0},
    "timeline": {"posts": 0, "verified": true},
    "services": {"peers": 0, "services": 0},
    "queue": {"pending": 0, "delivered": 0, "failed": 0}
  }
}
```

Also writes `latest.dump` and `latest.txt` for quick access.
