# Agent Circle API Reference

**Version**: 0.1.0  

This reference documents all public APIs exposed by the `agent-circle` workspace.

## Crate Overview

| Crate | Purpose | Docs |
|-------|---------|------|
| `agent-circle` | Main binary + library | [Modules](#modules) |
| `agent-circle-core` | Shared types (identity, chat, errors, keys, protocol) | [Core API](core.md) |

## Modules

### Core (`agent-circle-core`)

| Module | Description |
|--------|-------------|
| [`identity`](core.md#identity) | Identity generation, DID, AgentCard |
| [`chat`](core.md#chat) | ChatRequest/Response, DoctorRequest/Response, ServiceCall |
| [`errors`](core.md#errors) | Unified error types (E0001–E0005) |
| [`keys`](core.md#keys) | BIP-39 mnemonic, key derivation, Ed25519 |
| [`protocol`](core.md#protocol) | Version constants, protocol identifiers |

### Networking

| Module | Description |
|--------|-------------|
| [`network`](network.md) | P2P swarm, send_chat, send_doctor, run_daemon |

### Data

| Module | Description |
|--------|-------------|
| [`storage`](storage.md) | File-based persistence (identity, card, contacts, timeline) |
| [`message_queue`](message-queue.md) | Offline message queue (SQLite) |

### Services

| Module | Description |
|--------|-------------|
| [`service_discovery`](service-discovery.md) | ServiceRegistry, publish/subscribe |

### Reliability

| Module | Description |
|--------|-------------|
| [`reliability`](reliability.md) | PendingTracker (ACK/retry), SequenceTracker (ordering) |
| [`dedup`](reliability.md#dedupfilter) | Message deduplication |
| [`diag`](reliability.md#diagnostic-counters) | DiagCounters, DiagSnapshot |

### Observability

| Module | Description |
|--------|-------------|
| [`metrics`](observability.md#metrics) | OpenMetrics/Prometheus exporter |
| [`health`](observability.md#health) | HTTP health check server (:9099) |
| [`crash`](observability.md#crash) | Panic crash dump handler |

## Quick Reference

### Most-used types

```rust
use agent_circle_core::identity::Identity;
use agent_circle_core::chat::{ChatRequest, ChatResponse};
use agent_circle_core::errors::{AcError, AcResult};
```

### Most-used functions

```rust
use agent_circle::network::{build_swarm, send_chat, run_daemon};
use agent_circle::storage::{load_identity, save_identity, load_card, save_card};
use agent_circle::metrics::collect;  // OpenMetrics text
use agent_circle::health::spawn;     // Start HTTP server
use agent_circle::crash::init;       // Install panic hook
```

### Error handling pattern

```rust
fn do_something() -> AcResult<()> {
    let id = load_identity(data_dir)?.ok_or(AcError::Identity("未创建身份".into()))?;
    // ...
    Ok(())
}
```
