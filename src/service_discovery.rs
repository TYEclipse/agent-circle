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
use tracing::{debug, info, warn};

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

    /// Get last-seen timestamp for a peer.
    #[allow(dead_code)] // public API, used by future CLI display
    pub fn last_seen_for(&self, peer_id: &str) -> Option<i64> {
        self.last_seen.get(peer_id).copied()
    }

    /// Return all known services across all peers, with their last-seen timestamp.
    /// Returns Vec of (peer_id, ServiceInfo, last_seen).
    pub fn all_services_with_meta(&self) -> Vec<(String, ServiceInfo, i64)> {
        self.peers
            .iter()
            .flat_map(|(peer, svcs)| {
                let ts = self.last_seen.get(peer).copied().unwrap_or(0);
                svcs.iter().map(move |s| (peer.clone(), s.clone(), ts))
            })
            .collect()
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

    // ── Cache freshness (S10R108) ──────────────────────────────────

    /// Whether the registry has any cached data.
    pub fn has_cached_data(&self) -> bool {
        self.peer_count() > 0
    }

    /// Check if a specific peer's records are still fresh.
    #[allow(dead_code)] // public API used by cache diagnostics
    pub fn is_peer_fresh(&self, peer_id: &str, max_age_secs: i64) -> bool {
        match self.last_seen.get(peer_id) {
            Some(ts) => {
                let now = chrono::Utc::now().timestamp();
                now - ts <= max_age_secs
            }
            None => false,
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
/// If subscriptions are provided, checks for matching subscriptions and logs notifications.
pub fn handle_service_message(
    data: &[u8],
    registry: &mut ServiceRegistry,
    data_dir: &std::path::Path,
    subs: Option<&mut ServiceSubscriptions>,
) {
    match serde_json::from_slice::<ServiceAnnouncement>(data) {
        Ok(ann) => {
            registry.ingest(ann.clone());
            // Persist after each new announcement
            let _ = save_registry(registry, data_dir);

            // Check subscriptions for matching services
            if let Some(subscriptions) = subs {
                for svc in &ann.services {
                    if subscriptions.is_subscribed(&svc.id, Some(&ann.peer_id))
                        || subscriptions.is_subscribed(&svc.id, None)
                    {
                        let versions = if svc.protocol_versions.is_empty() {
                            "latest".to_string()
                        } else {
                            svc.protocol_versions.join(", ")
                        };
                        info!(
                            peer_id = %ann.peer_id,
                            service_id = %svc.id,
                            name = %svc.name,
                            versions = %versions,
                            "🔔 订阅服务更新通知"
                        );
                        // Update last_seen_version in subscription
                        for sub in &mut subscriptions.items {
                            if sub.service_id == svc.id {
                                sub.last_seen_version = versions.clone();
                            }
                        }
                        let _ = save_subscriptions(subscriptions, data_dir);
                    }
                }
            }
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

// ── Service Subscriptions (S10R107) ────────────────────────────────

/// A single subscription entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Subscription {
    /// The service ID we're watching (e.g. "weather-v1").
    pub service_id: String,
    /// Optional: only watch announcements from a specific peer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_id: Option<String>,
    /// Human-readable label (e.g. "Weather Bot").
    #[serde(default)]
    pub label: String,
    /// Unix timestamp when subscription was created.
    pub created_at: i64,
    /// The last version we've seen (used for change detection).
    #[serde(default)]
    pub last_seen_version: String,
}

/// Tracks user subscriptions to services.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ServiceSubscriptions {
    pub items: Vec<Subscription>,
}

impl ServiceSubscriptions {
    /// Add a new subscription (idempotent — skips if already exists).
    pub fn subscribe(&mut self, service_id: &str, peer_id: Option<&str>, label: &str) {
        let key = (service_id, peer_id);
        if self
            .items
            .iter()
            .any(|s| s.service_id == key.0 && s.peer_id.as_deref() == key.1)
        {
            return;
        }
        self.items.push(Subscription {
            service_id: service_id.to_string(),
            peer_id: peer_id.map(|s| s.to_string()),
            label: label.to_string(),
            created_at: chrono::Utc::now().timestamp(),
            last_seen_version: String::new(),
        });
    }

    /// Remove a subscription by service_id + optional peer_id.
    pub fn unsubscribe(&mut self, service_id: &str, peer_id: Option<&str>) -> bool {
        let len_before = self.items.len();
        self.items
            .retain(|s| !(s.service_id == service_id && s.peer_id.as_deref() == peer_id));
        self.items.len() < len_before
    }

    /// List all active subscriptions.
    pub fn list(&self) -> &[Subscription] {
        &self.items
    }

    /// Check if we're subscribed to a specific service.
    pub fn is_subscribed(&self, service_id: &str, peer_id: Option<&str>) -> bool {
        self.items
            .iter()
            .any(|s| s.service_id == service_id && s.peer_id.as_deref() == peer_id)
    }
}

/// Save subscriptions to disk.
pub fn save_subscriptions(subs: &ServiceSubscriptions, data_dir: &std::path::Path) -> AcResult<()> {
    let path = data_dir.join("subscriptions.json");
    let json = serde_json::to_string_pretty(subs)?;
    std::fs::write(&path, json).map_err(AcError::Io)?;
    debug!(path = %path.display(), "订阅列表已保存");
    Ok(())
}

/// Load subscriptions from disk.
pub fn load_subscriptions(data_dir: &std::path::Path) -> AcResult<ServiceSubscriptions> {
    let path = data_dir.join("subscriptions.json");
    if !path.exists() {
        return Ok(ServiceSubscriptions::default());
    }
    let json = std::fs::read_to_string(&path).map_err(AcError::Io)?;
    let subs: ServiceSubscriptions = serde_json::from_str(&json).map_err(AcError::Serialization)?;
    Ok(subs)
}

// ── Publication push via GossipSub (S13R135) ─────────────────────

/// Broadcast a publication to all network subscribers via GossipSub.
/// The caller should save the publication locally first; this function
/// handles the network broadcast to the publications topic.
#[allow(dead_code)] // called from daemon dispatch path (not CLI)
pub fn publish_publication(
    swarm: &mut libp2p::Swarm<crate::network::AgentCircleBehaviour>,
    publication: &agent_circle_core::publication::Publication,
) -> AcResult<()> {
    let topic = gossipsub::IdentTopic::new(crate::protocol::publications_topic());
    let data = serde_json::to_vec(publication)?;
    swarm
        .behaviour_mut()
        .gossip
        .publish(topic, data)
        .map_err(|e| AcError::Network(format!("publication push failed: {e}")))?;
    info!(
        service_id = %publication.service_id,
        version = publication.version,
        title = %publication.title,
        "📡 公众号推送已广播 (GossipSub)"
    );
    Ok(())
}

/// Handle an incoming GossipSub publication message.
/// If the user is subscribed to the service, stores a notification.
pub fn handle_publication_message(
    data: &[u8],
    subs: &ServiceSubscriptions,
    data_dir: &std::path::Path,
) {
    match serde_json::from_slice::<agent_circle_core::publication::Publication>(data) {
        Ok(pub_msg) => {
            if subs.is_subscribed(&pub_msg.service_id, None) {
                info!(
                    service_id = %pub_msg.service_id,
                    version = pub_msg.version,
                    title = %pub_msg.title,
                    "🔔 收到订阅服务推送"
                );
                if let Err(e) =
                    crate::storage::notify_subscriber(data_dir, &pub_msg.service_id, &pub_msg.id)
                {
                    warn!(error = %e, "存储推送通知失败");
                }
            }
        }
        Err(_e) => {
            // Not a publication message — silently skip (could be other GossipSub data)
        }
    }
}
