//! Agent Circle Core — shared types and utilities.
//!
//! Workspace split.  This crate contains all wire-format
//! types, identity primitives, error types, and protocol constants so
//! they can be shared between the CLI binary and future crates
//! (agent-circle-net, agent-circle-timeline, etc.) without pulling in
//! the full libp2p dependency tree.
//!
//! # Crate layout
//!
//! | Module | Purpose | Stability |
//! |--------|---------|-----------|
//! | [`identity`] | DID, Ed25519 keypair, AgentCard, Identity | ✅ stable (0.1) |
//! | [`chat`] | ChatRequest / ChatResponse wire types | ✅ stable (0.1) |
//! | [`errors`] | AcError / AcResult | ✅ stable (0.1) |
//! | [`protocol`] | Version constants, SemVer policy | ✅ stable (0.1) |
//! | [`keys`] | BIP-39 mnemonic ↔ Ed25519 seed | ✅ stable (0.1) |
//!
//! # API Stability Contract
//!
//! ## Stability levels
//!
//! - ✅ **Stable** — no breaking changes within the same MAJOR version.
//!   Deprecation notice at least one MINOR release before removal.
//! - ⚠️ **Unstable** — may change in any MINOR release.  Gated behind
//!   `#[doc(hidden)]` or feature flag.
//! - ❌ **Deprecated** — will be removed in the next MAJOR.  Use the
//!   documented replacement.
//!
//! ## Backward compatibility guarantees
//!
//! 1. **Wire format** — `ChatRequest`, `ChatResponse`, `AgentCard`,
//!    `TimelineNode` use `#[serde(default)]` on all optional fields.
//!    New fields added in MINOR releases are silently ignored by old
//!    peers.
//!
//! 2. **Identity** — `Identity::from_seed()`, `to_seed_bytes()`,
//!    `verifying_key()` are stable.  The 32-byte seed format is
//!    permanent (BIP-39 compatible).
//!
//! 3. **Errors** — `AcError` variants MAY gain new arms in MINOR
//!    releases (non-exhaustive by design via `#[non_exhaustive]` on
//!    key enums where applicable).
//!
//! 4. **Protocol** — `VERSION`, `identify_agent()`, `chat_protocol()`,
//!    `relay_dht_key()` follow the SemVer rules in
//!    `docs/protocol-versioning.md`.
//!
//! ## Deprecation policy
//!
//! 1. Mark with `#[deprecated(since = \"x.y.z\", note = \"use Y\")]`
//! 2. Keep for at least one full MINOR release cycle
//! 3. Remove only on MAJOR bump
//! 4. Document in CHANGELOG under `### Changed`
//!
//! ## Crate dependency policy
//!
//! `agent-circle-core` intentionally has a **minimal dependency tree**
//! (no libp2p, no tokio, no rusqlite):
//!
//!   serde + serde_json  (wire format)
//!   ed25519-dalek       (identity keys)
//!   bip39               (mnemonic seed phrases)
//!   chrono              (timestamps)
//!   thiserror           (error derive macros)
//!   zeroize             (secure memory clearing)
//!   blake3 / bs58 / hex (hashing, encoding)
//!
//! This ensures downstream crates pay only for what they need.

pub mod chat;
pub mod errors;
pub mod identity;
pub mod keys;
pub mod plugin;
pub mod protocol;
pub mod publication;
