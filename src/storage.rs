//! Storage module — persist identity and data to data directory.
//!
//! Default: `~/.agent-circle/`
//! Custom: set via `--data-dir` CLI flag or `AGENT_CIRCLE_HOME` env var.
//!
//! Directory layout:
//!   {data_dir}/
//!   ├── identity.key     # Ed25519 seed bytes (0600)
//!   ├── card.json        # Latest self-signed Agent Card
//!   ├── contacts.json    # Contact list
//!   └── config.toml      # (future)

use crate::errors::{AcError, AcResult};
use crate::identity::{AgentCard, Identity};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Resolve the data directory, accepting an optional override.
///
/// Priority (highest first):
///   1. `--data-dir` CLI flag
///   2. `AGENT_CIRCLE_HOME` environment variable
///   3. `~/.agent-circle/` (cross-platform via `dirs::home_dir`)
pub fn resolve_data_dir(override_dir: Option<&PathBuf>) -> AcResult<PathBuf> {
    if let Some(dir) = override_dir {
        fs::create_dir_all(dir)?;
        return Ok(dir.clone());
    }

    // S07R74: support AGENT_CIRCLE_HOME env var for cross-platform flexibility
    if let Ok(custom) = std::env::var("AGENT_CIRCLE_HOME") {
        let dir = PathBuf::from(custom);
        fs::create_dir_all(&dir)?;
        return Ok(dir);
    }

    let home = dirs::home_dir().ok_or_else(|| {
        AcError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "cannot determine home directory",
        ))
    })?;

    let dir = home.join(".agent-circle");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Save the identity seed to {data_dir}/identity.key (0600).
pub fn save_identity(id: &Identity, data_dir: Option<&PathBuf>) -> AcResult<()> {
    let dir = resolve_data_dir(data_dir)?;
    let path = dir.join("identity.key");

    let tmp = dir.join(".identity.key.tmp");
    {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            f.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        f.write_all(&id.to_seed_bytes())?;
        f.flush()?;
    }
    fs::rename(&tmp, &path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

/// Load an identity from {data_dir}/identity.key.
pub fn load_identity(data_dir: Option<&PathBuf>) -> AcResult<Option<Identity>> {
    let dir = resolve_data_dir(data_dir)?;
    let path = dir.join("identity.key");

    if !path.exists() {
        return Ok(None);
    }

    let bytes = fs::read(&path)?;
    if bytes.len() != 32 {
        return Err(AcError::Identity(format!(
            "identity.key: expected 32 bytes, got {}",
            bytes.len()
        )));
    }

    let seed: &[u8; 32] = bytes.as_slice().try_into().unwrap();
    Identity::from_seed(seed).map(Some)
}

/// Save an Agent Card to {data_dir}/card.json.
pub fn save_card(card: &AgentCard, data_dir: Option<&PathBuf>) -> AcResult<()> {
    let dir = resolve_data_dir(data_dir)?;
    let path = dir.join("card.json");
    let json = serde_json::to_string_pretty(card)?;
    fs::write(&path, json)?;
    Ok(())
}

/// Load the saved Agent Card from {data_dir}/card.json.
pub fn load_card(data_dir: Option<&PathBuf>) -> AcResult<Option<AgentCard>> {
    let dir = resolve_data_dir(data_dir)?;
    let path = dir.join("card.json");

    if !path.exists() {
        return Ok(None);
    }

    let json = fs::read_to_string(&path)?;
    let card: AgentCard = serde_json::from_str(&json)?;
    Ok(Some(card))
}

// ── Contacts ───────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub name: String,
    pub peer_id: String,
    pub did: String,
    pub added_at: String,
}

pub fn load_contacts(data_dir: Option<&PathBuf>) -> AcResult<Vec<Contact>> {
    let dir = resolve_data_dir(data_dir)?;
    let path = dir.join("contacts.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let json = fs::read_to_string(&path)?;
    let contacts: Vec<Contact> = serde_json::from_str(&json).unwrap_or_default();
    Ok(contacts)
}

pub fn save_contacts(contacts: &[Contact], data_dir: Option<&PathBuf>) -> AcResult<()> {
    let dir = resolve_data_dir(data_dir)?;
    let path = dir.join("contacts.json");
    let json = serde_json::to_string_pretty(contacts)?;
    fs::write(&path, json)?;
    Ok(())
}

pub fn add_contact(
    name: &str,
    peer_id: &str,
    did: &str,
    data_dir: Option<&PathBuf>,
) -> AcResult<()> {
    let mut contacts = load_contacts(data_dir)?;
    // Don't add duplicates
    if contacts.iter().any(|c| c.peer_id == peer_id) {
        return Err(AcError::Identity(format!("联系人 {peer_id} 已存在")));
    }
    contacts.push(Contact {
        name: name.to_string(),
        peer_id: peer_id.to_string(),
        did: did.to_string(),
        added_at: chrono::Utc::now().to_rfc3339(),
    });
    save_contacts(&contacts, data_dir)
}

// ── Timeline ───────────────────────────────────────────────────────

use crate::timeline::Timeline;

/// Load the timeline from {data_dir}/timeline.json.
pub fn load_timeline(data_dir: Option<&PathBuf>) -> AcResult<Timeline> {
    let dir = resolve_data_dir(data_dir)?;
    let path = dir.join("timeline.json");
    if !path.exists() {
        return Ok(Timeline::new());
    }
    let json = fs::read_to_string(&path)?;
    let tl: Timeline = serde_json::from_str(&json)?;
    Ok(tl)
}

/// Save the timeline to {data_dir}/timeline.json.
pub fn save_timeline(tl: &Timeline, data_dir: Option<&PathBuf>) -> AcResult<()> {
    let dir = resolve_data_dir(data_dir)?;
    let path = dir.join("timeline.json");
    let json = serde_json::to_string_pretty(tl)?;
    fs::write(&path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("ac_storage_{}", rand::random::<u32>()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn resolve_default_dir() {
        let dir = resolve_data_dir(None).unwrap();
        assert!(dir.ends_with(".agent-circle"));
    }

    #[test]
    fn resolve_override_dir() {
        let tmp = temp_dir();
        let dir = resolve_data_dir(Some(&tmp)).unwrap();
        assert_eq!(dir, tmp);
    }

    #[test]
    fn identity_save_and_load() {
        let tmp = temp_dir();
        let id = Identity::generate();
        save_identity(&id, Some(&tmp)).unwrap();
        let loaded = load_identity(Some(&tmp)).unwrap().unwrap();
        assert_eq!(loaded.did, id.did);
        assert_eq!(loaded.short_code, id.short_code);
    }

    #[test]
    fn identity_load_not_found() {
        let tmp = temp_dir();
        let result = load_identity(Some(&tmp)).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn identity_load_wrong_size() {
        let tmp = temp_dir();
        let path = tmp.join("identity.key");
        std::fs::write(&path, [0u8; 16]).unwrap();
        let result = load_identity(Some(&tmp));
        assert!(result.is_err());
    }

    #[test]
    fn card_save_and_load() {
        let tmp = temp_dir();
        let id = Identity::generate();
        let card = id
            .create_card("TestBot", "h:test", "gpt", &["code".into()], vec![])
            .unwrap();
        save_card(&card, Some(&tmp)).unwrap();
        let loaded = load_card(Some(&tmp)).unwrap().unwrap();
        assert_eq!(loaded.name, "TestBot");
        assert_eq!(loaded.capabilities, vec!["code"]);
    }

    #[test]
    fn card_load_not_found() {
        let tmp = temp_dir();
        let result = load_card(Some(&tmp)).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn contacts_add_and_list() {
        let tmp = temp_dir();
        let contacts = load_contacts(Some(&tmp)).unwrap();
        assert!(contacts.is_empty());

        add_contact("alice", "peer1", "did:alice", Some(&tmp)).unwrap();
        add_contact("bob", "peer2", "did:bob", Some(&tmp)).unwrap();

        let contacts = load_contacts(Some(&tmp)).unwrap();
        assert_eq!(contacts.len(), 2);
        assert_eq!(contacts[0].name, "alice");
        assert_eq!(contacts[1].peer_id, "peer2");
    }

    #[test]
    fn contact_duplicate_rejected() {
        let tmp = temp_dir();
        add_contact("alice", "peer1", "did:a", Some(&tmp)).unwrap();
        let result = add_contact("alice_dup", "peer1", "did:a", Some(&tmp));
        assert!(result.is_err());
    }

    #[test]
    fn timeline_save_and_load() {
        let tmp = temp_dir();
        let id = Identity::generate();
        let mut tl = Timeline::new();
        let node = Timeline::genesis(&id, "Hello timeline").unwrap();
        tl.nodes.push(node);

        save_timeline(&tl, Some(&tmp)).unwrap();
        let loaded = load_timeline(Some(&tmp)).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.nodes[0].content, "Hello timeline");
    }

    #[test]
    fn timeline_load_not_found() {
        let tmp = temp_dir();
        let tl = load_timeline(Some(&tmp)).unwrap();
        assert!(tl.is_empty());
    }
}

// ── S13R131 Publication history (公众号) ─────────────────────────

use agent_circle_core::publication::PublicationHistory;
use std::path::Path;

/// Path for a service's publication history file.
fn publication_history_path(data_dir: &Path, service_id: &str) -> PathBuf {
    let safe_id = str::replace(service_id, &['/', '\\'][..], "_");
    data_dir.join(format!("publications-{}.json", safe_id))
}

/// Load publication history for a service. Returns empty if not found.
pub fn load_publication_history(data_dir: &Path, service_id: &str) -> AcResult<PublicationHistory> {
    let path = publication_history_path(data_dir, service_id);
    if !path.exists() {
        return Ok(PublicationHistory::new(service_id.to_string()));
    }
    let json = std::fs::read_to_string(&path)?;
    let history: PublicationHistory = serde_json::from_str(&json)?;
    Ok(history)
}

/// Save publication history for a service.
pub fn save_publication_history(history: &PublicationHistory, data_dir: &Path) -> AcResult<()> {
    let path = publication_history_path(data_dir, &history.service_id);
    let json = serde_json::to_string_pretty(history)?;
    std::fs::write(&path, json)?;
    Ok(())
}

// ── Publication notifications (S13R133) ──────────────────────────

use std::collections::HashMap;

/// Notification manifest: maps service_id → list of unread publication IDs.
type Notifications = HashMap<String, Vec<String>>;

fn notifications_path(data_dir: &Path) -> PathBuf {
    data_dir.join("notifications.json")
}

/// Load the notification manifest. Returns empty map if not found.
pub fn load_notifications(data_dir: &Path) -> AcResult<Notifications> {
    let path = notifications_path(data_dir);
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let json = std::fs::read_to_string(&path)?;
    let n: Notifications = serde_json::from_str(&json)?;
    Ok(n)
}

/// Save the notification manifest.
pub fn save_notifications(notifications: &Notifications, data_dir: &Path) -> AcResult<()> {
    let path = notifications_path(data_dir);
    let json = serde_json::to_string_pretty(notifications)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Add a publication ID to the notification list for a service.
pub fn notify_subscriber(data_dir: &Path, service_id: &str, publication_id: &str) -> AcResult<()> {
    let mut n = load_notifications(data_dir)?;
    n.entry(service_id.to_string())
        .or_default()
        .push(publication_id.to_string());
    save_notifications(&n, data_dir)
}

/// Clear notifications for a service (mark as read).
pub fn clear_notifications(data_dir: &Path, service_id: &str) -> AcResult<()> {
    let mut n = load_notifications(data_dir)?;
    n.remove(service_id);
    save_notifications(&n, data_dir)
}

// ── Ratings storage (S13R137) ────────────────────────────────────

use agent_circle_core::publication::Rating;

fn ratings_path(data_dir: &Path, service_id: &str) -> PathBuf {
    let safe_id = str::replace(service_id, &['/', '\\'][..], "_");
    data_dir.join(format!("ratings-{}.json", safe_id))
}

/// Load all ratings for a service. Returns empty vec if not found.
pub fn load_ratings(data_dir: &Path, service_id: &str) -> AcResult<Vec<Rating>> {
    let path = ratings_path(data_dir, service_id);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let json = std::fs::read_to_string(&path)?;
    let ratings: Vec<Rating> = serde_json::from_str(&json)?;
    Ok(ratings)
}

/// Save ratings for a service.
pub fn save_ratings(data_dir: &Path, service_id: &str, ratings: &[Rating]) -> AcResult<()> {
    let path = ratings_path(data_dir, service_id);
    let json = serde_json::to_string_pretty(ratings)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Add a rating for a service (upserts by reviewer_did).
pub fn add_rating(data_dir: &Path, rating: &Rating) -> AcResult<()> {
    let mut ratings = load_ratings(data_dir, &rating.service_id)?;
    ratings.retain(|r| r.reviewer_did != rating.reviewer_did);
    ratings.push(rating.clone());
    save_ratings(data_dir, &rating.service_id, &ratings)
}

/// Return the aggregated rating summary for a service.
pub fn rating_summary(
    data_dir: &Path,
    service_id: &str,
) -> AcResult<agent_circle_core::publication::RatingSummary> {
    let ratings = load_ratings(data_dir, service_id)?;
    Ok(agent_circle_core::publication::RatingSummary::from_ratings(
        &ratings,
    ))
}

// ── Service permissions (S13R139) ─────────────────────────────────

use agent_circle_core::publication::ServicePermission;

fn permission_path(data_dir: &Path, service_id: &str) -> PathBuf {
    let safe_id = str::replace(service_id, &['/', '\\'][..], "_");
    data_dir.join(format!("permissions-{}.json", safe_id))
}

/// Load permission for a service. Defaults to Public if not set.
pub fn load_permission(data_dir: &Path, service_id: &str) -> AcResult<ServicePermission> {
    let path = permission_path(data_dir, service_id);
    if !path.exists() {
        return Ok(ServicePermission::Public);
    }
    let json = std::fs::read_to_string(&path)?;
    let perm: ServicePermission = serde_json::from_str(&json)?;
    Ok(perm)
}

/// Save permission for a service.
pub fn save_permission(
    data_dir: &Path,
    service_id: &str,
    perm: &ServicePermission,
) -> AcResult<()> {
    let path = permission_path(data_dir, service_id);
    let json = serde_json::to_string_pretty(perm)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Format permission for display.
pub fn permission_display(perm: &ServicePermission) -> &'static str {
    match perm {
        ServicePermission::Public => "🔓 公开",
        ServicePermission::ApprovalRequired => "🔐 需审批",
        ServicePermission::Whitelist(_) => "🔒 白名单",
    }
}

#[cfg(test)]
mod publication_storage_tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("ac-pub-test-{}", rand::random::<u16>()))
    }

    #[test]
    fn publication_history_save_and_load() {
        use agent_circle_core::publication::Publication;
        use chrono::Utc;

        let tmp = temp_dir();
        let _ = std::fs::create_dir_all(&tmp);
        let svc = "test-svc-v1";
        let mut history = PublicationHistory::new(svc.to_string());

        let pub_msg = Publication {
            id: "abc123".into(),
            service_id: svc.to_string(),
            title: "Hello".into(),
            content: "World".into(),
            content_type: agent_circle_core::publication::ContentType::Text,
            timestamp: Utc::now(),
            version: 1,
            signature: String::new(),
        };
        history.push(pub_msg);

        save_publication_history(&history, &tmp).unwrap();
        let loaded = load_publication_history(&tmp, svc).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.publications[0].title, "Hello");
        assert_eq!(loaded.publications[0].version, 1);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn publication_history_load_not_found() {
        let tmp = temp_dir();
        let _ = std::fs::create_dir_all(&tmp);
        let history = load_publication_history(&tmp, "nonexistent").unwrap();
        assert!(history.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

// ── S14R142 Notification / Rating / Permission storage tests ────

#[cfg(test)]
mod notification_storage_tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("ac-ntf-test-{}", rand::random::<u16>()))
    }

    #[test]
    fn notifications_load_empty() {
        let tmp = temp_dir();
        let _ = std::fs::create_dir_all(&tmp);
        let n = load_notifications(&tmp).unwrap();
        assert!(n.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn notifications_add_and_load() {
        let tmp = temp_dir();
        let _ = std::fs::create_dir_all(&tmp);
        notify_subscriber(&tmp, "weather-v1", "pub-123").unwrap();
        notify_subscriber(&tmp, "weather-v1", "pub-456").unwrap();
        notify_subscriber(&tmp, "news-v1", "pub-789").unwrap();

        let n = load_notifications(&tmp).unwrap();
        assert_eq!(n.get("weather-v1").unwrap().len(), 2);
        assert_eq!(n.get("news-v1").unwrap().len(), 1);
        assert_eq!(n.get("weather-v1").unwrap()[0], "pub-123");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn notifications_clear() {
        let tmp = temp_dir();
        let _ = std::fs::create_dir_all(&tmp);
        notify_subscriber(&tmp, "svc", "id1").unwrap();
        notify_subscriber(&tmp, "svc", "id2").unwrap();
        notify_subscriber(&tmp, "other", "id3").unwrap();

        clear_notifications(&tmp, "svc").unwrap();
        let n = load_notifications(&tmp).unwrap();
        assert!(!n.contains_key("svc"));
        assert_eq!(n.get("other").unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

#[cfg(test)]
mod rating_storage_tests {
    use super::*;
    use agent_circle_core::publication::Rating;
    use chrono::Utc;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("ac-rat-test-{}", rand::random::<u16>()))
    }

    fn sample_rating(svc: &str, reviewer: &str, score: u8) -> Rating {
        Rating {
            service_id: svc.into(),
            reviewer_did: reviewer.into(),
            score,
            comment: None,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn ratings_load_empty() {
        let tmp = temp_dir();
        let _ = std::fs::create_dir_all(&tmp);
        let ratings = load_ratings(&tmp, "nonexistent").unwrap();
        assert!(ratings.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn ratings_add_and_load() {
        let tmp = temp_dir();
        let _ = std::fs::create_dir_all(&tmp);
        let r1 = sample_rating("svc-1", "did:a", 5);
        let r2 = sample_rating("svc-1", "did:b", 3);

        add_rating(&tmp, &r1).unwrap();
        add_rating(&tmp, &r2).unwrap();

        let ratings = load_ratings(&tmp, "svc-1").unwrap();
        assert_eq!(ratings.len(), 2);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn ratings_upsert_same_reviewer() {
        let tmp = temp_dir();
        let _ = std::fs::create_dir_all(&tmp);
        let r1 = sample_rating("svc-1", "did:a", 3);
        let r2 = sample_rating("svc-1", "did:a", 5);

        add_rating(&tmp, &r1).unwrap();
        add_rating(&tmp, &r2).unwrap();

        let ratings = load_ratings(&tmp, "svc-1").unwrap();
        assert_eq!(ratings.len(), 1);
        assert_eq!(ratings[0].score, 5);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn rating_summary_from_storage() {
        let tmp = temp_dir();
        let _ = std::fs::create_dir_all(&tmp);
        add_rating(&tmp, &sample_rating("svc", "did:a", 4)).unwrap();
        add_rating(&tmp, &sample_rating("svc", "did:b", 2)).unwrap();

        let summary = rating_summary(&tmp, "svc").unwrap();
        assert_eq!(summary.count, 2);
        assert!((summary.average - 3.0).abs() < 0.01);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

#[cfg(test)]
mod permission_storage_tests {
    use super::*;
    use agent_circle_core::publication::ServicePermission;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("ac-perm-test-{}", rand::random::<u16>()))
    }

    #[test]
    fn permission_default_is_public() {
        let tmp = temp_dir();
        let _ = std::fs::create_dir_all(&tmp);
        let perm = load_permission(&tmp, "nonexistent").unwrap();
        assert_eq!(perm, ServicePermission::Public);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn permission_save_and_load() {
        let tmp = temp_dir();
        let _ = std::fs::create_dir_all(&tmp);
        let perm = ServicePermission::ApprovalRequired;
        save_permission(&tmp, "svc-1", &perm).unwrap();
        let loaded = load_permission(&tmp, "svc-1").unwrap();
        assert_eq!(loaded, ServicePermission::ApprovalRequired);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn permission_whitelist() {
        let tmp = temp_dir();
        let _ = std::fs::create_dir_all(&tmp);
        let perm = ServicePermission::Whitelist(vec!["did:a".into()]);
        save_permission(&tmp, "svc-1", &perm).unwrap();
        let loaded = load_permission(&tmp, "svc-1").unwrap();
        assert_eq!(loaded, ServicePermission::Whitelist(vec!["did:a".into()]));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn permission_display_labels() {
        assert_eq!(permission_display(&ServicePermission::Public), "🔓 公开");
        assert_eq!(
            permission_display(&ServicePermission::ApprovalRequired),
            "🔐 需审批"
        );
        assert_eq!(
            permission_display(&ServicePermission::Whitelist(vec![])),
            "🔒 白名单"
        );
    }
}
