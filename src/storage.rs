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
