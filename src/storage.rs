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
pub fn resolve_data_dir(override_dir: Option<&PathBuf>) -> AcResult<PathBuf> {
    if let Some(dir) = override_dir {
        fs::create_dir_all(dir)?;
        return Ok(dir.clone());
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
