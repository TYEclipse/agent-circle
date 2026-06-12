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
}
