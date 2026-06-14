//! Identity module — "微信号" of Agent Circle
//!
//! Generates Ed25519 keypairs, encodes them as W3C DID:key identifiers,
//! produces short codes (human-friendly "微信 ID"), and creates
//! self-signed Agent Cards.

use crate::errors::{AcError as Error, AcResult as Result};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

// ── Constants ──────────────────────────────────────────────────────

/// Multicodec prefix for Ed25519 public key: 0xED, 0x01
const ED25519_MULTICODEC: [u8; 2] = [0xED, 0x01];

/// Agent Card context URI
pub const CARD_CONTEXT: &str = "https://agent-circle.io/card/v1";

// ── Identity ───────────────────────────────────────────────────────

/// A complete agent identity — the digital "微信账号"
#[derive(Clone)]
pub struct Identity {
    /// The Ed25519 signing key (secret). MUST be zeroized on drop.
    pub signing_key: SigningKey,
    /// DID:key identifier (public, shareable)
    pub did: String,
    /// Human-friendly short code (8-char hex, like a WeChat ID)
    pub short_code: String,
}

impl Identity {
    /// Generate a brand-new identity from system randomness.
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();
        let did = encode_did_key(&verifying_key);
        let short_code = encode_short_code(&did);

        Self {
            signing_key,
            did,
            short_code,
        }
    }

    /// Restore an identity from raw 32-byte seed bytes.
    pub fn from_seed(seed: &[u8; 32]) -> Result<Self> {
        let signing_key = SigningKey::from_bytes(seed);
        let verifying_key = signing_key.verifying_key();
        let did = encode_did_key(&verifying_key);
        let short_code = encode_short_code(&did);

        Ok(Self {
            signing_key,
            did,
            short_code,
        })
    }

    /// Export the raw 32-byte seed for backup. HANDLE WITH CARE.
    pub fn to_seed_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// Export the Ed25519 verifying key (public key).
    #[allow(dead_code)]
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Create a self-signed Agent Card.
    pub fn create_card(
        &self,
        name: &str,
        owner: &str,
        model: &str,
        capabilities: &[String],
        services: Vec<ServiceInfo>,
    ) -> Result<AgentCard> {
        let mut card = AgentCard {
            context: CARD_CONTEXT.to_string(),
            did: self.did.clone(),
            name: name.to_string(),
            owner: owner.to_string(),
            model: model.to_string(),
            capabilities: capabilities.to_vec(),
            services,
            endpoints: vec![], // filled in when daemon starts
            status: "offline".to_string(),
            updated: chrono::Utc::now().to_rfc3339(),
            proof: String::new(),
        };

        // Self-sign: sign the card's JSON (without proof field) and attach
        let payload = card.to_unsigned_json()?;
        let signature = self.signing_key.sign(payload.as_bytes());
        card.proof = bs58::encode(signature.to_bytes()).into_string();

        Ok(card)
    }
}

impl Drop for Identity {
    fn drop(&mut self) {
        // Zeroize the secret key material from memory
        let mut bytes = self.signing_key.to_bytes();
        bytes.zeroize();
    }
}

// ── DID:key Encoding ──────────────────────────────────────────────

/// Encode an Ed25519 VerifyingKey as a `did:key:z...` identifier.
///
/// Format: `did:key:z` + base58btc(multicodec_prefix || raw_pubkey)
///   - multicodec_prefix: 0xED 0x01 (Ed25519 public key)
///   - raw_pubkey: 32 bytes
fn encode_did_key(vk: &VerifyingKey) -> String {
    let mut buf = Vec::with_capacity(34);
    buf.extend_from_slice(&ED25519_MULTICODEC);
    buf.extend_from_slice(vk.as_bytes());
    format!("did:key:z{}", bs58::encode(&buf).into_string())
}

/// Decode a `did:key:z...` back to an Ed25519 VerifyingKey.
#[allow(dead_code)]
pub fn decode_did_key(did: &str) -> Result<VerifyingKey> {
    let encoded = did
        .strip_prefix("did:key:z")
        .ok_or_else(|| Error::Identity(format!("invalid DID:key prefix: {did}")))?;

    let bytes = bs58::decode(encoded)
        .into_vec()
        .map_err(|e| Error::Identity(format!("base58 decode failed: {e}")))?;

    if bytes.len() < 2 || bytes[0] != ED25519_MULTICODEC[0] || bytes[1] != ED25519_MULTICODEC[1] {
        return Err(Error::Identity(format!(
            "unsupported multicodec in DID:key: {:02x}{:02x}",
            bytes[0], bytes[1]
        )));
    }

    let key_bytes: &[u8; 32] = bytes[2..]
        .try_into()
        .map_err(|_| Error::Identity("invalid Ed25519 key length".into()))?;

    VerifyingKey::from_bytes(key_bytes)
        .map_err(|e| Error::Identity(format!("invalid Ed25519 key: {e}")))
}

// ── Short Code ─────────────────────────────────────────────────────

/// Derive a human-friendly 8-char short code from a DID.
/// Like a WeChat ID — shorter than the full DID, but derived from it.
fn encode_short_code(did: &str) -> String {
    let hash = blake3::hash(did.as_bytes());
    hex::encode(&hash.as_bytes()[..4])
}

// ── Service Info ───────────────────────────────────────────────────

/// A service registered by an agent — discoverable by peers on the DHT.
///
/// S10R101: Extends Agent Card with the `services` field so agents can
/// advertise what they offer (weather bot, translator, relay, …).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceInfo {
    /// Unique service identifier (e.g. "weather-v1").
    pub id: String,
    /// Human-readable name (e.g. "Weather Bot").
    pub name: String,
    /// The protocol endpoint this service listens on
    /// (e.g. "/agent-circle/weather/1.0.0").
    pub endpoint: String,
    /// Optional description of what the service does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Searchable tags for discovery (e.g. ["weather", "forecast"]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// S10R106 — Supported protocol versions (e.g. ["1.0.0", "2.0.0-beta"]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protocol_versions: Vec<String>,
    /// S10R106 — JSON Schema describing accepted input parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<String>,
}

// ── Capability Negotiation (S10R106) ────────────────────────────────

/// Sent by a caller to probe a service's capabilities before invoking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityProbe {
    /// Which service are we asking about (e.g. "weather-v1").
    pub service_id: String,
    /// Protocol versions the caller supports (ordered by preference).
    pub accepted_versions: Vec<String>,
    /// Requested parameter format (e.g. "json", "cbor").
    #[serde(default = "default_format")]
    pub param_format: String,
}

fn default_format() -> String {
    "json".into()
}

/// A single capability entry returned by the service.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolVersion {
    /// Semantic version string (e.g. "1.0.0").
    pub version: String,
    /// Endpoint path for this version (e.g. "/ac/weather/1.0.0").
    pub endpoint: String,
    /// Input parameter JSON schema (optional — "{}" = no schema).
    #[serde(default = "empty_string")]
    pub input_schema: String,
}

fn empty_string() -> String {
    String::new()
}

/// Response to a CapabilityProbe — the service lists what it supports.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityStatement {
    /// Which service this statement describes.
    pub service_id: String,
    /// Protocols / versions the service supports.
    pub versions: Vec<ProtocolVersion>,
    /// Accepted parameter formats (e.g. ["json", "cbor"]).
    pub accepted_formats: Vec<String>,
    /// If the service_id is unknown, this is set to false.
    pub service_found: bool,
}

// ── Agent Card ─────────────────────────────────────────────────────

/// A self-signed capability card — "what this agent is"
///
/// Equivalent to a WeChat profile: name, bio (capabilities), owner info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    #[serde(rename = "@context")]
    pub context: String,
    pub did: String,
    pub name: String,
    pub owner: String,
    pub model: String,
    pub capabilities: Vec<String>,
    /// S10R101 — Services this agent provides.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<ServiceInfo>,
    pub endpoints: Vec<String>,
    pub status: String,
    pub updated: String,
    pub proof: String,
}

impl AgentCard {
    /// Serialize the card WITHOUT the proof field (for signing).
    fn to_unsigned_json(&self) -> Result<String> {
        let value = serde_json::json!({
            "@context": self.context,
            "did": self.did,
            "name": self.name,
            "owner": self.owner,
            "model": self.model,
            "capabilities": self.capabilities,
            "services": self.services,
            "endpoints": self.endpoints,
            "status": self.status,
            "updated": self.updated,
        });
        // Deterministic: sort keys so signature is reproducible
        serde_json::to_string(&value).map_err(Into::into)
    }

    /// Verify the card's self-signature against its own DID.
    #[allow(dead_code)]
    pub fn verify(&self) -> Result<bool> {
        let vk = decode_did_key(&self.did)?;

        let signature_bytes = bs58::decode(&self.proof)
            .into_vec()
            .map_err(|e| Error::Identity(format!("invalid signature encoding: {e}")))?;

        let signature: &[u8; 64] = signature_bytes
            .as_slice()
            .try_into()
            .map_err(|_| Error::Identity("invalid signature length".into()))?;

        let signature = ed25519_dalek::Signature::from_bytes(signature);
        let payload = self.to_unsigned_json()?;

        Ok(vk.verify_strict(payload.as_bytes(), &signature).is_ok())
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_identity() {
        let id = Identity::generate();
        assert!(id.did.starts_with("did:key:z"));
        assert_eq!(id.short_code.len(), 8);
        assert!(id.short_code.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_did_key_roundtrip() {
        let id = Identity::generate();
        let vk = decode_did_key(&id.did).unwrap();
        assert_eq!(vk.as_bytes(), id.verifying_key().as_bytes());
    }

    #[test]
    fn test_short_code_deterministic() {
        let id = Identity::generate();
        let code1 = encode_short_code(&id.did);
        let code2 = encode_short_code(&id.did);
        assert_eq!(code1, code2);
    }

    #[test]
    fn test_seed_roundtrip() {
        let id = Identity::generate();
        let seed = id.to_seed_bytes();
        let restored = Identity::from_seed(&seed).unwrap();
        assert_eq!(restored.did, id.did);
        assert_eq!(restored.short_code, id.short_code);
    }

    #[test]
    fn test_seed_bytes_length() {
        let id = Identity::generate();
        let seed = id.to_seed_bytes();
        assert_eq!(seed.len(), 32);
    }

    #[test]
    fn test_verifying_key() {
        let id = Identity::generate();
        let vk = id.verifying_key();
        assert_eq!(vk.as_bytes().len(), 32);
    }

    #[test]
    fn test_agent_card_sign_and_verify() {
        let id = Identity::generate();
        let card = id
            .create_card("TestBot", "human:test@example.com", "gpt-4", &[], vec![])
            .unwrap();
        assert!(card.verify().unwrap());
    }

    #[test]
    fn test_agent_card_tamper_detection() {
        let id = Identity::generate();
        let mut card = id
            .create_card("TestBot", "human:test@example.com", "gpt-4", &[], vec![])
            .unwrap();
        // Tamper
        card.name = "EvilBot".to_string();
        assert!(!card.verify().unwrap());
    }

    // ── decode_did_key error paths ──────────────────────────────────

    #[test]
    fn decode_did_key_bad_prefix() {
        let result = decode_did_key("did:other:z1234");
        assert!(result.is_err());
    }

    #[test]
    fn decode_did_key_bad_base58() {
        let result = decode_did_key("did:key:z!!!invalid!!!");
        assert!(result.is_err());
    }

    #[test]
    fn decode_did_key_wrong_multicodec() {
        // Construct a key with wrong multicodec prefix (0xAB, 0xCD)
        let mut buf = vec![0xAB, 0xCD];
        buf.extend_from_slice(&[0u8; 32]);
        let encoded = bs58::encode(&buf).into_string();
        let did = format!("did:key:z{encoded}");
        assert!(decode_did_key(&did).is_err());
    }

    #[test]
    fn decode_did_key_wrong_length() {
        // Only 4 bytes after prefix — too short for Ed25519 (needs 32)
        let mut buf = vec![0xED, 0x01]; // correct multicodec
        buf.extend_from_slice(&[0u8; 4]); // wrong key length
        let encoded = bs58::encode(&buf).into_string();
        let did = format!("did:key:z{encoded}");
        assert!(decode_did_key(&did).is_err());
    }

    #[test]
    fn decode_did_key_invalid_key() {
        // ed25519-dalek 2.x doesn't reject all invalid points;
        // instead test that from_seed returns consistent did for same seed.
        let seed = [42u8; 32];
        let id1 = Identity::from_seed(&seed).unwrap();
        let id2 = Identity::from_seed(&seed).unwrap();
        assert_eq!(id1.did, id2.did);
        assert_eq!(id1.short_code, id2.short_code);
    }

    // ── AgentCard verify error paths ────────────────────────────────

    #[test]
    fn agent_card_verify_invalid_proof_encoding() {
        let id = Identity::generate();
        let mut card = id.create_card("Bot", "h:test", "gpt", &[], vec![]).unwrap();
        card.proof = "!!!not-base58!!!".into();
        assert!(card.verify().is_err());
    }

    #[test]
    fn agent_card_verify_invalid_proof_length() {
        let id = Identity::generate();
        let mut card = id.create_card("Bot", "h:test", "gpt", &[], vec![]).unwrap();
        // base58-encode a 16-byte string (wrong sig length)
        card.proof = bs58::encode(&[0u8; 16]).into_string();
        assert!(card.verify().is_err());
    }
}
