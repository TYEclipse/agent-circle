# Agent Circle — Protocol Versioning Strategy

## SemVer for wire protocols

Agent Circle uses **Semantic Versioning** ([SemVer 2.0](https://semver.org/))
for application-level protocol identifiers carried over the wire.

### Version format

```
MAJOR.MINOR.PATCH   e.g. 0.2.1
```

### Rules

| Change type | Bump | Protocol string | Interop |
|---|---|---|---|
| Wire-format **breaking** change | **MAJOR** (`0.1` → `0.2`) | `/agent-circle/chat/0.2.0` | Old peers see new protocol; interop only if both versions registered in `SUPPORTED_CHAT_PROTOCOLS` |
| Backward-compatible **feature** | **MINOR** (`0.2` → `0.3`) | `/agent-circle/chat/0.2.0` unchanged | Old peers ignore unknown fields (`#[serde(default)]`) |
| **Bug fix** (no format change) | **PATCH** (`0.2.0` → `0.2.1`) | Unchanged | Full interop |

### Backward compatibility mechanism

Each node advertises ALL supported protocol versions via libp2p
`identify`.  When connecting, peers negotiate the **highest common**
version using the `request-response` protocol support mechanism.

Example: if a 0.2.0 node advertises:
```
/agent-circle/chat/0.1.0   (Full)
/agent-circle/chat/0.2.0   (Full)
```

A 0.1.0-only peer picks 0.1.0, while a 0.2.0 peer picks 0.2.0.

### Adding a new protocol version

1. Bump `VERSION` in `src/protocol.rs`
2. Add the old version string to `SUPPORTED_CHAT_PROTOCOLS`
3. In `network.rs`, register all versions in `ChatBehaviour::new()`
4. Ensure all `ChatRequest`/`ChatResponse` fields use `#[serde(default)]`
   so old peers can deserialize new messages

### JSON forward-compatibility rules

- All message fields use `#[serde(default)]` for optional new fields
- Unknown top-level fields are silently ignored by serde
- `String`/`Vec<String>` types are used for extensibility
- Field removal: deprecate for one MAJOR cycle before removal

### Current version: **0.1.0**

| Protocol | Identifier |
|---|---|
| Identify | `/agent-circle/0.1.0` |
| Chat (1:1) | `/agent-circle/chat/0.1.0` |
| Relay DHT | `/agent-circle/relays/0.1.0` |
| GossipSub topics | `agent-circle/group/{hash}` |
