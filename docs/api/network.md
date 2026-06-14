# Network API

## `build_swarm`

Build a P2P swarm with all protocols (QUIC, mDNS, Kademlia DHT, GossipSub, relay, chat, doctor).

```rust
use agent_circle::network::build_swarm;
use agent_circle_core::identity::Identity;

let id = Identity::generate();
let swarm = build_swarm(&id)?;
// swarm automatically listens on IPv4 QUIC
```

**Protocols included**:
- **Transport**: QUIC + TCP + relay circuit
- **Discovery**: mDNS (LAN) + Kademlia DHT (WAN)
- **Messaging**: Chat (1:1 request/response) + Group (GossipSub)
- **Diagnostics**: Doctor (remote self-check)
- **Relay**: NAT traversal fallback
- **Connection limits**: 50 in / 50 out / 10 pending

## `send_chat`

Send a one-to-one message to a peer.

```rust
use agent_circle::network::send_chat;
use libp2p::PeerId;

let peer_id: PeerId = "12D3KooW...".parse()?;
let rid = send_chat(&mut swarm, peer_id, "fb41e829", "Hello!");
// returns OutboundRequestId for tracking
```

## `run_daemon`

Main event loop — blocks until shutdown.

```rust
use agent_circle::network::run_daemon;

run_daemon(
    &id,              // Identity
    &["general"],     // groups to join at startup
    false,            // relay mode
    &data_dir,        // data directory
).await?;
```

**Event loop handles**:
- Connection lifecycle (establish / close)
- mDNS discovery → Kademlia DHT bootstrap
- Chat: incoming messages + ACK + retry + offline queue
- Doctor: incoming remote diagnostics requests
- GossipSub: group messages + service announcements
- Periodic: stats log (30s), queue cleanup (5min), service republish (60s)

## `send_doctor`

Send a remote diagnostics request.

```rust
use agent_circle::network::send_doctor;

let rid = send_doctor(
    &mut swarm,
    peer_id,
    "fb41e829",
    Some("network".to_string()),
);
```

## `join_group` / `send_group_message` / `list_group_topics`

Group chat over GossipSub.

```rust
use agent_circle::network::{join_group, send_group_message, list_group_topics};

join_group(&mut swarm, "general")?;
send_group_message(&mut swarm, "general", "fb41e829", "Hello everyone!")?;
let topics = list_group_topics(&swarm);
```

## `group_topic`

Derive a deterministic topic hash from a group name.

```rust
use agent_circle::network::group_topic;

let topic = group_topic("general");
// → IdentTopic("agent-circle/group/f3241a7b...")
```

## Behaviour Type Aliases

```rust
pub type ChatBehaviour = request_response::json::Behaviour<ChatRequest, ChatResponse>;
pub type DoctorBehaviour = request_response::json::Behaviour<DoctorRequest, DoctorResponse>;
```

## Event Types

```rust
// Access via SwarmEvent matching
use libp2p::swarm::SwarmEvent;
use agent_circle::network::AgentCircleBehaviourEvent;

match event {
    SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Chat(msg)) => { /* ... */ }
    SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Doctor(msg)) => { /* ... */ }
    SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Gossip(msg)) => { /* ... */ }
    _ => {}
}
```
