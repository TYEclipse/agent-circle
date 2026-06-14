# Service Discovery API

P2P service registry with GossipSub broadcast and local caching.

## `ServiceRegistry`

```rust
use agent_circle::service_discovery::{ServiceRegistry, load_registry};

let mut registry = ServiceRegistry::default();

// Query
println!("{} peers, {} services", registry.peer_count(), registry.service_count());

// Detailed listing
let services = registry.all_services_with_meta();
for (peer_id, svc_info, last_seen) in &services {
    println!("{}: {} @ {} @ {}", peer_id, svc_info.name, svc_info.endpoint, last_seen);
}

// Freshness
if !registry.is_peer_fresh("12D3KooW...", 120) { // 2 minute threshold
    println!("Peer may be offline");
}

// Persistence
let pb = PathBuf::from("~/.agent-circle");
let loaded = load_registry(&pb)?;

// Maintenance
registry.prune(600); // remove entries older than 10 minutes
```

## Publish / Subscribe

```rust
use agent_circle::service_discovery::{publish_services, subscribe_services, handle_service_message};

// Daemon: subscribe to service announcements
subscribe_services(&mut swarm)?;

// Daemon: publish own services
let own = vec![ServiceInfo {
    id: "weather-v1".into(),
    name: "Weather Service".into(),
    endpoint: "/ac/weather/1.0.0".into(),
    description: Some("Get weather forecasts".into()),
    tags: vec!["weather".into()],
    protocol_versions: vec!["0.1.0".into()],
    input_schema: None,
}];
publish_services(&mut swarm, local_peer_id, &own)?;

// Daemon: handle incoming announcements
handle_service_message(&data, &mut registry, data_dir, Some(&mut subs));
```

## Subscriptions

```rust
use agent_circle::service_discovery::{ServiceSubscriptions, load_subscriptions};

let mut subs = ServiceSubscriptions::new();

subs.subscribe("weather-v1", Some("12D3KooW..."), "weather");
subs.list(); // => ["weather-v1@12D3KooW... (weather)"]
subs.unsubscribe("weather-v1", Some("12D3KooW..."));

// Persist
let loaded = load_subscriptions(data_dir)?;
```

## CLI Equivalents

| CLI Command | API Equivalent |
|-------------|---------------|
| `service list` | `registry.all_services_with_meta()` |
| `service search <q>` | `registry.search(q)` |
| `service subscribe <s> -l <l>` | `subs.subscribe(s, None, l)` |
| `service unsubscribe <s>` | `subs.unsubscribe(s, None)` |
| `service publish <id> -n <n> -e <e>` | `publish_services(...)` |
| `service cache --stats` | `registry.peer_count()` / `.service_count()` |
| `service cache --flush` | `registry.prune(0)` |
