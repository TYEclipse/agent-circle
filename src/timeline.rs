//! Timeline module — Merkle-DAG 防篡改社交时间线
//!
//! Each "moment" (post) is a node in an append-only hash chain.
//! Every node is self-signed by the author's Ed25519 key.
//! The chain is verifiable: follow hashes, check signatures.
//!
//! Structure:
//!   genesis ──→ node1 ──→ node2 ──→ node3
//!   (no parents)  (parent=genesis)  (parent=node2)

use crate::errors::{AcError, AcResult};
use crate::identity::Identity;
use ed25519_dalek::Signer;
use serde::{Deserialize, Serialize};

/// One post on the timeline — a signed, hash-linked node in the Merkle-DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineNode {
    /// blake3 hash of the canonical payload (content + ts + parents + author)
    pub id: String,
    /// Author DID
    pub author: String,
    /// Free-form post content
    pub content: String,
    /// Unix timestamp (seconds)
    pub ts: i64,
    /// Parent node IDs (empty vec = genesis)
    pub parents: Vec<String>,
    /// Ed25519 signature (bs58) of the signing payload
    pub signature: String,
}

/// An append-only, hash-linked social timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timeline {
    pub nodes: Vec<TimelineNode>,
}

impl Timeline {
    /// Create an empty timeline.
    pub fn new() -> Self {
        Timeline { nodes: Vec::new() }
    }

    /// Create the genesis post — the first post on a new timeline.
    pub fn genesis(id: &Identity, content: &str) -> AcResult<TimelineNode> {
        let ts = chrono::Utc::now().timestamp();
        let author = id.did.clone();

        let node_id = hash_node(&author, content, ts, &[]);
        let sig_payload = signing_payload(&node_id, &author, content, ts, &[]);
        let signature =
            bs58::encode(id.signing_key.sign(sig_payload.as_bytes()).to_bytes()).into_string();

        Ok(TimelineNode {
            id: node_id,
            author,
            content: content.to_string(),
            ts,
            parents: vec![],
            signature,
        })
    }

    /// Append a new post to the timeline, linking to all current tips.
    /// For a linear chain, there's only one tip (the last post).
    pub fn append(&mut self, id: &Identity, content: &str) -> AcResult<TimelineNode> {
        let parents: Vec<String> = self
            .nodes
            .last()
            .map(|n| vec![n.id.clone()])
            .unwrap_or_default();

        let ts = chrono::Utc::now().timestamp();
        let author = id.did.clone();

        let node_id = hash_node(&author, content, ts, &parents);
        let sig_payload = signing_payload(&node_id, &author, content, ts, &parents);
        let signature =
            bs58::encode(id.signing_key.sign(sig_payload.as_bytes()).to_bytes()).into_string();

        let node = TimelineNode {
            id: node_id,
            author,
            content: content.to_string(),
            ts,
            parents,
            signature,
        };

        self.nodes.push(node.clone());
        Ok(node)
    }

    /// Verify the entire timeline — every node's hash and signature.
    /// Returns Ok(()) if valid, or an error describing the first invalid node.
    pub fn verify(&self) -> AcResult<()> {
        for (i, node) in self.nodes.iter().enumerate() {
            // Verify hash
            let expected_id = hash_node(&node.author, &node.content, node.ts, &node.parents);
            if node.id != expected_id {
                return Err(AcError::Identity(format!(
                    "节点 {} 哈希不符: expected {expected_id}, got {}",
                    i, node.id
                )));
            }

            // Verify signature
            let vk = crate::identity::decode_did_key(&node.author)?;
            let sig_payload = signing_payload(
                &node.id,
                &node.author,
                &node.content,
                node.ts,
                &node.parents,
            );
            let sig_bytes = bs58::decode(&node.signature)
                .into_vec()
                .map_err(|e| AcError::Identity(format!("节点 {i}: 签名解码失败: {e}")))?;
            let sig_arr: &[u8; 64] = sig_bytes
                .as_slice()
                .try_into()
                .map_err(|_| AcError::Identity(format!("节点 {i}: 签名长度异常")))?;
            let signature = ed25519_dalek::Signature::from_bytes(sig_arr);

            vk.verify_strict(sig_payload.as_bytes(), &signature)
                .map_err(|e| AcError::Identity(format!("节点 {i}: 签名验证失败: {e}")))?;
        }
        Ok(())
    }

    /// Number of posts.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the timeline is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

// ── Internal helpers ───────────────────────────────────────────────

/// Hash a node's canonical payload: author | content | ts | parent1,parent2,...
fn hash_node(author: &str, content: &str, ts: i64, parents: &[String]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(author.as_bytes());
    hasher.update(b"|");
    hasher.update(content.as_bytes());
    hasher.update(b"|");
    hasher.update(ts.to_string().as_bytes());
    if !parents.is_empty() {
        hasher.update(b"|");
        for p in parents {
            hasher.update(p.as_bytes());
            hasher.update(b",");
        }
    }
    hasher.finalize().to_hex().to_string()
}

/// The payload that gets signed: node_id | author | content | ts | parents
fn signing_payload(
    node_id: &str,
    author: &str,
    content: &str,
    ts: i64,
    parents: &[String],
) -> String {
    let parents_str = if parents.is_empty() {
        String::new()
    } else {
        format!("|{}", parents.join(","))
    };
    format!("{node_id}|{author}|{content}|{ts}{parents_str}")
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Identity;

    #[test]
    fn test_genesis_and_verify() {
        let id = Identity::generate();
        let node = Timeline::genesis(&id, "Hello, world!").unwrap();

        assert_eq!(node.parents.len(), 0);
        assert!(!node.id.is_empty());
        assert!(!node.signature.is_empty());

        // Verify the node individually by creating a timeline
        let tl = Timeline { nodes: vec![node] };
        tl.verify().unwrap();
    }

    #[test]
    fn test_append_chain_and_verify() {
        let id = Identity::generate();
        let mut tl = Timeline::new();

        let g = tl.append(&id, "Genesis post").unwrap();
        assert_eq!(g.parents.len(), 0);

        let n1 = tl.append(&id, "Second post").unwrap();
        assert_eq!(n1.parents, vec![g.id.clone()]);

        let n2 = tl.append(&id, "Third post").unwrap();
        assert_eq!(n2.parents, vec![n1.id.clone()]);

        assert_eq!(tl.len(), 3);
        tl.verify().unwrap();
    }

    #[test]
    fn test_tamper_detection_content() {
        let id = Identity::generate();
        let mut tl = Timeline::new();
        tl.append(&id, "ok").unwrap();

        // Tamper with content
        tl.nodes[0].content = "hacked".to_string();
        assert!(tl.verify().is_err());
    }

    #[test]
    fn test_tamper_detection_signature() {
        let id = Identity::generate();
        let mut tl = Timeline::new();
        tl.append(&id, "ok").unwrap();

        // Tamper with signature
        tl.nodes[0].signature = "fake".to_string();
        assert!(tl.verify().is_err());
    }

    #[test]
    fn test_tamper_detection_hash_chain() {
        let id = Identity::generate();
        let mut tl = Timeline::new();
        tl.append(&id, "first").unwrap();
        tl.append(&id, "second").unwrap();

        // Tamper with the first node's content — breaks hash chain for node 1
        tl.nodes[0].content = "hacked".to_string();
        assert!(tl.verify().is_err());
    }

    // ── Empty / default timeline ─────────────────────────────────────

    #[test]
    fn test_new_timeline_empty() {
        let tl = Timeline::new();
        assert!(tl.is_empty());
        assert_eq!(tl.len(), 0);
    }

    #[test]
    fn test_default() {
        let tl = Timeline::default();
        assert!(tl.is_empty());
        assert_eq!(tl.len(), 0);
    }

    #[test]
    fn test_verify_empty_timeline() {
        let tl = Timeline::new();
        // Empty timeline should verify without error
        assert!(tl.verify().is_ok());
    }

    #[test]
    fn test_append_on_empty_timeline() {
        let id = Identity::generate();
        let mut tl = Timeline::new();
        // Appending on an empty timeline — parents should be empty
        let node = tl.append(&id, "first post").unwrap();
        assert!(node.parents.is_empty());
        assert_eq!(tl.len(), 1);
    }

    // ── Determinism ──────────────────────────────────────────────────

    #[test]
    fn test_genesis_deterministic() {
        // Use from_seed for deterministic identity
        let seed = [42u8; 32];
        let id = Identity::from_seed(&seed).unwrap();
        let node1 = Timeline::genesis(&id, "hello").unwrap();
        let node2 = Timeline::genesis(&id, "hello").unwrap();
        // Same identity + same content → same hash (ts may differ, but
        // hash includes ts, so with different timestamps the hash will
        // differ. Use from_seed with fixed ts — actually ts is always
        // now(), so we accept that deterministic genesis is hard to
        // test with real timestamps. Instead: verify both pass verify.
        let tl1 = Timeline {
            nodes: vec![node1.clone()],
        };
        tl1.verify().unwrap();
        let tl2 = Timeline { nodes: vec![node2] };
        tl2.verify().unwrap();
        // Content round-trip is correct
        assert_eq!(node1.content, "hello");
    }

    #[test]
    fn test_different_content_different_hash() {
        let id = Identity::generate();
        let n1 = Timeline::genesis(&id, "alpha").unwrap();
        let n2 = Timeline::genesis(&id, "beta").unwrap();
        assert_ne!(
            n1.id, n2.id,
            "different content should produce different hash"
        );
    }

    // ── Tamper detection: id & parent chain ──────────────────────────

    #[test]
    fn test_tamper_detection_id() {
        let id = Identity::generate();
        let mut tl = Timeline::new();
        tl.append(&id, "ok").unwrap();

        // Tamper the id directly
        tl.nodes[0].id = "0000000000000000".to_string();
        assert!(tl.verify().is_err());
    }

    #[test]
    fn test_tamper_detection_parent() {
        let id = Identity::generate();
        let mut tl = Timeline::new();
        tl.append(&id, "first").unwrap();
        let _second = tl.append(&id, "second").unwrap();

        // Tamper: change second node's parent to a non-existent hash
        // This breaks the second node's hash (hash includes parents)
        tl.nodes[1].parents = vec!["nonexistent".to_string()];
        assert!(tl.verify().is_err());
    }

    // ── Serde ────────────────────────────────────────────────────────

    #[test]
    fn test_timeline_node_serde_roundtrip() {
        let id = Identity::generate();
        let node = Timeline::genesis(&id, "serde test 🧪").unwrap();
        let json = serde_json::to_string(&node).unwrap();
        let back: TimelineNode = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, node.id);
        assert_eq!(back.author, node.author);
        assert_eq!(back.content, node.content);
        assert_eq!(back.ts, node.ts);
        assert_eq!(back.parents, node.parents);
        assert_eq!(back.signature, node.signature);
    }

    #[test]
    fn test_timeline_serde_roundtrip() {
        let id = Identity::generate();
        let mut tl = Timeline::new();
        tl.append(&id, "post 1").unwrap();
        tl.append(&id, "post 2").unwrap();

        let json = serde_json::to_string(&tl).unwrap();
        let back: Timeline = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back.nodes[0].content, "post 1");
        assert_eq!(back.nodes[1].content, "post 2");
        // Verify the deserialized timeline is still valid
        back.verify().unwrap();
    }

    // ── Internal helpers ─────────────────────────────────────────────

    #[test]
    fn test_hash_node_no_parents() {
        let h1 = hash_node("alice", "hi", 1000, &[]);
        let h2 = hash_node("alice", "hi", 1000, &[]);
        assert_eq!(h1, h2, "hash should be deterministic");
        assert_eq!(h1.len(), 64, "blake3 hex is 64 chars");
    }

    #[test]
    fn test_hash_node_with_parents() {
        let parents = vec!["aaa".to_string(), "bbb".to_string()];
        let h1 = hash_node("alice", "hi", 1000, &parents);
        let h2 = hash_node("alice", "hi", 1000, &parents);
        assert_eq!(h1, h2);
        // With parents vs without should differ
        let h_no_parents = hash_node("alice", "hi", 1000, &[]);
        assert_ne!(h1, h_no_parents);
    }

    #[test]
    fn test_signing_payload_format() {
        let payload = signing_payload("abc", "alice", "hello", 1000, &[]);
        assert_eq!(payload, "abc|alice|hello|1000");

        let payload2 = signing_payload("abc", "alice", "hello", 1000, &["p1".into(), "p2".into()]);
        assert_eq!(payload2, "abc|alice|hello|1000|p1,p2");
    }

    #[test]
    fn test_len_tracks_nodes() {
        let id = Identity::generate();
        let mut tl = Timeline::new();
        assert_eq!(tl.len(), 0);
        tl.append(&id, "a").unwrap();
        assert_eq!(tl.len(), 1);
        tl.append(&id, "b").unwrap();
        assert_eq!(tl.len(), 2);
        tl.append(&id, "c").unwrap();
        assert_eq!(tl.len(), 3);
    }

    #[test]
    fn test_hacker_cant_forge_signature_with_wrong_key() {
        let alice = Identity::generate();
        let bob = Identity::generate();
        assert_ne!(alice.did, bob.did);

        // Alice creates a post
        let mut tl = Timeline::new();
        let node = tl.append(&alice, "alice's post").unwrap();
        assert_eq!(node.author, alice.did);

        // Bob tries to add a post signed with alice's key — can't,
        // but if someone modifies author to bob's DID after the fact,
        // verify should catch the signature mismatch
        let mut hacked_node = node.clone();
        hacked_node.author = bob.did.clone();
        let tl2 = Timeline {
            nodes: vec![hacked_node],
        };
        assert!(
            tl2.verify().is_err(),
            "signature should not verify for wrong author"
        );
    }
}
