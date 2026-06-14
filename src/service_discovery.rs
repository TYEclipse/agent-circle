//! S10R102 — Service Discovery via GossipSub.
//!
//! Agents broadcast their `ServiceInfo` list on a shared GossipSub topic
//! so peers can discover what services are available on the network.
//!
//! # Flow
//!
//! 1. Daemon subscribes to `services_topic` on startup
//! 2. Daemon publishes its own AgentCard.services periodically
//! 3. Incoming announcements are deserialized and stored in `ServiceRegistry`
//! 4. CLI / API queries the registry for search + listing

use crate::errors::{AcError, AcResult};
use crate::identity::ServiceInfo;
use libp2p::{gossipsub, PeerId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info};

/// A service announcement broadcast over GossipSub.
///
/// Contains the announcing peer's ID and their service list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAnnouncement {
    /// Peer ID of the announcing agent.
    pub peer_id: String,
    /// The services this agent provides.
    pub services: Vec<ServiceInfo>,
    /// Unix timestamp of announcement.
    pub ts: i64,
}

/// Local cache of discovered services, indexed by peer.
#[derive(Debug, Default)]
pub struct ServiceRegistry {
    /// peer_id → list of services known about this peer.
    peers: HashMap<String, Vec<ServiceInfo>>,
    /// Timestamp of last update per peer (for staleness detection).
    last_seen: HashMap<String, i64>,
}

impl ServiceRegistry {
    /// Record a service announcement from a peer.
    pub fn ingest(&mut self, announcement: ServiceAnnouncement) {
        let peer = announcement.peer_id.clone();
        debug!(
            peer = %peer,
            count = announcement.services.len(),
            "收到服务公告"
        );
        self.peers.insert(peer.clone(), announcement.services);
        self.last_seen.insert(peer, announcement.ts);
    }

    /// Look up services for a specific peer.
    #[allow(dead_code)] // used by R103 CLI
    pub fn get(&self, peer_id: &str) -> Option<&[ServiceInfo]> {
        self.peers.get(peer_id).map(|v| v.as_slice())
    }

    /// Return all known services across all peers, as (peer_id, ServiceInfo) pairs.
    #[allow(dead_code)] // used by R103 CLI
    pub fn all_services(&self) -> Vec<(String, ServiceInfo)> {
        self.peers
            .iter()
            .flat_map(|(peer, svcs)| svcs.iter().map(|s| (peer.clone(), s.clone())))
            .collect()
    }

    /// Search for services by name or tag.
    #[allow(dead_code)] // used by R103 CLI
    pub fn search(&self, query: &str) -> Vec<(String, ServiceInfo)> {
        let q = query.to_lowercase();
        self.all_services()
            .into_iter()
            .filter(|(_, s)| {
                s.name.to_lowercase().contains(&q)
                    || s.id.to_lowercase().contains(&q)
                    || s.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .collect()
    }

    /// Return the number of peers we have services for.
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Return the total number of services known.
    pub fn service_count(&self) -> usize {
        self.peers.values().map(|v| v.len()).sum()
    }

    /// Remove expired entries (peers not seen for `max_age_secs`).
    pub fn prune(&mut self, max_age_secs: i64) {
        let now = chrono::Utc::now().timestamp();
        let stale: Vec<_> = self
            .last_seen
            .iter()
            .filter(|(_, ts)| now - *ts > max_age_secs)
            .map(|(peer, _)| peer.clone())
            .collect();
        for peer in &stale {
            self.peers.remove(peer);
            self.last_seen.remove(peer);
        }
        if !stale.is_empty() {
            info!(count = stale.len(), "清理过期服务记录");
        }
    }
}

/// Publish the agent's services on the service discovery GossipSub topic.
pub fn publish_services(
    swarm: &mut libp2p::Swarm<crate::network::AgentCircleBehaviour>,
    peer_id: PeerId,
    services: &[ServiceInfo],
) -> AcResult<()> {
    let topic = gossipsub::IdentTopic::new(crate::protocol::services_topic());
    let announcement = ServiceAnnouncement {
        peer_id: peer_id.to_string(),
        services: services.to_vec(),
        ts: chrono::Utc::now().timestamp(),
    };
    let data = serde_json::to_vec(&announcement)?;
    swarm
        .behaviour_mut()
        .gossip
        .publish(topic, data)
        .map_err(|e| AcError::Network(format!("服务公告发布失败: {e}")))?;
    debug!(count = services.len(), "服务公告已发布");
    Ok(())
}

/// Subscribe to the service discovery topic.
pub fn subscribe_services(
    swarm: &mut libp2p::Swarm<crate::network::AgentCircleBehaviour>,
) -> AcResult<()> {
    let topic = gossipsub::IdentTopic::new(crate::protocol::services_topic());
    swarm
        .behaviour_mut()
        .gossip
        .subscribe(&topic)
        .map_err(|e| AcError::Network(format!("服务订阅失败: {e}")))?;
    info!(topic = %topic, "已订阅服务发现频道");
    Ok(())
}

/// Handle an incoming GossipSub message — try to deserialize as a ServiceAnnouncement.
/// Saves the registry to disk after a successful ingest.
pub fn handle_service_message(
    data: &[u8],
    registry: &mut ServiceRegistry,
    data_dir: &std::path::Path,
) {
    match serde_json::from_slice::<ServiceAnnouncement>(data) {
        Ok(ann) => {
            registry.ingest(ann);
            // Persist after each new announcement
            let _ = save_registry(registry, data_dir);
        }
        Err(_e) => {
            // Not a service announcement — could be a group chat message or other data.
            // Silently skip; the caller routes messages by topic.
        }
    }
}

// ── Disk persistence ──────────────────────────────────────────────

/// Serializable snapshot of the service registry for disk storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrySnapshot {
    pub entries: Vec<RegistryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub peer_id: String,
    pub services: Vec<ServiceInfo>,
    pub last_seen: i64,
}

impl ServiceRegistry {
    /// Convert to a serializable snapshot.
    pub fn to_snapshot(&self) -> RegistrySnapshot {
        let entries: Vec<_> = self
            .peers
            .iter()
            .map(|(peer, svcs)| RegistryEntry {
                peer_id: peer.clone(),
                services: svcs.clone(),
                last_seen: self.last_seen.get(peer).copied().unwrap_or(0),
            })
            .collect();
        RegistrySnapshot { entries }
    }

    /// Restore from a snapshot.
    pub fn from_snapshot(snapshot: RegistrySnapshot) -> Self {
        let mut registry = ServiceRegistry::default();
        for entry in snapshot.entries {
            registry.peers.insert(entry.peer_id.clone(), entry.services);
            registry.last_seen.insert(entry.peer_id, entry.last_seen);
        }
        registry
    }
}

/// Save the service registry to a JSON file.
pub fn save_registry(registry: &ServiceRegistry, data_dir: &std::path::Path) -> AcResult<()> {
    let path = data_dir.join("services.json");
    let snapshot = registry.to_snapshot();
    let json = serde_json::to_string_pretty(&snapshot)?;
    std::fs::write(&path, json).map_err(AcError::Io)?;
    debug!(path = %path.display(), "服务注册表已保存");
    Ok(())
}

/// Load the service registry from a JSON file.
/// Returns an empty registry if the file doesn't exist.
pub fn load_registry(data_dir: &std::path::Path) -> AcResult<ServiceRegistry> {
    let path = data_dir.join("services.json");
    if !path.exists() {
        return Ok(ServiceRegistry::default());
    }
    let json = std::fs::read_to_string(&path).map_err(AcError::Io)?;
    let snapshot: RegistrySnapshot = serde_json::from_str(&json).map_err(AcError::Serialization)?;
    Ok(ServiceRegistry::from_snapshot(snapshot))
}
