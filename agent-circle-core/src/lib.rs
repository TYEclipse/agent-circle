//! Agent Circle Core — shared types and utilities.
//!
//! S08R83-R86 Workspace split.  This crate contains all wire-format
//! types, identity primitives, error types, and protocol constants so
//! they can be shared between the CLI binary and future crates
//! (agent-circle-net, agent-circle-timeline, etc.) without pulling in
//! the full libp2p dependency tree.
//!
//! ## Crate layout
//!
//!   identity    — DID, Ed25519 keypair, AgentCard, Identity
//!   chat        — ChatRequest / ChatResponse wire types
//!   errors      — AcError / AcResult
//!   protocol    — version constants, SemVer policy
//!   keys        — BIP-39 mnemonic ↔ Ed25519 seed

pub mod chat;
pub mod errors;
pub mod identity;
pub mod keys;
pub mod protocol;
