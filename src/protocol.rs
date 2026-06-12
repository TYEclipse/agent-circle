//! Contact handshake protocol — the "加好友" flow.
//!
//! Protocol ID: `/agent-circle/handshake/0.1.0`
//!
//! Flow:
//!   Alice ──HELLO(Alice's AgentCard + nonce)──→ Bob
//!   Alice ←──ACCEPT(Bob's AgentCard + sig(nonce))── Bob

use crate::errors::{AcError, AcResult};
use crate::identity::AgentCard;
use libp2p::request_response;
use serde::{Deserialize, Serialize};

pub const HANDSHAKE_PROTOCOL: &str = "/agent-circle/handshake/0.1.0";

// ── Handshake messages ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeRequest {
    pub card: AgentCard,
    pub nonce: [u8; 16],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeResponse {
    pub card: AgentCard,
    pub proof: Vec<u8>,
    pub session_key: Vec<u8>,
}

// ── Protocol type ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HandshakeProtocol;

impl AsRef<str> for HandshakeProtocol {
    fn as_ref(&self) -> &str {
        HANDSHAKE_PROTOCOL
    }
}

// ── CBOR Codec ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HandshakeCodec {
    max_size: usize,
}

impl HandshakeCodec {
    pub fn new(max_size: usize) -> Self {
        Self { max_size }
    }
}

impl request_response::Codec for HandshakeCodec {
    type Protocol = HandshakeProtocol;
    type Request = HandshakeRequest;
    type Response = HandshakeResponse;

    async fn read_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Request>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        read_cbor_msg(io, self.max_size).await
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> std::io::Result<Self::Response>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        read_cbor_msg(io, self.max_size).await
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> std::io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        write_cbor_msg(io, &req).await
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> std::io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        write_cbor_msg(io, &res).await
    }
}

// ── CBOR framing helpers ───────────────────────────────────────────

use futures::{AsyncReadExt, AsyncWriteExt};

async fn read_cbor_msg<T, M: serde::de::DeserializeOwned>(
    io: &mut T,
    max_size: usize,
) -> std::io::Result<M>
where
    T: futures::AsyncRead + Unpin + Send,
{
    use std::io::{Error, ErrorKind};

    let mut len_buf = [0u8; 4];
    io.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;

    if len > max_size {
        return Err(Error::new(ErrorKind::InvalidData, "message too large"));
    }

    let mut buf = vec![0u8; len];
    io.read_exact(&mut buf).await?;

    ciborium::from_reader(&buf[..])
        .map_err(|e| Error::new(ErrorKind::InvalidData, format!("cbor: {e}")))
}

async fn write_cbor_msg<T, M: serde::Serialize>(
    io: &mut T,
    msg: &M,
) -> std::io::Result<()>
where
    T: futures::AsyncWrite + Unpin + Send,
{
    use std::io::{Error, ErrorKind};

    let mut buf = Vec::new();
    ciborium::into_writer(msg, &mut buf)
        .map_err(|e| Error::new(ErrorKind::InvalidData, format!("cbor: {e}")))?;

    let len = buf.len() as u32;
    io.write_all(&len.to_be_bytes()).await?;
    io.write_all(&buf).await?;
    Ok(())
}

// ── Handshake logic ────────────────────────────────────────────────

use rand::RngCore;

pub fn build_handshake_request(card: &AgentCard) -> HandshakeRequest {
    let mut nonce = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut nonce);
    HandshakeRequest {
        card: card.clone(),
        nonce,
    }
}

pub fn process_handshake(
    req: &HandshakeRequest,
    my_card: &AgentCard,
    my_signing_key: &ed25519_dalek::SigningKey,
) -> AcResult<HandshakeResponse> {
    use ed25519_dalek::Signer;

    if !req.card.verify()? {
        return Err(AcError::Identity("handshake: initiator card invalid".into()));
    }

    let proof = my_signing_key.sign(&req.nonce).to_bytes().to_vec();
    let session_key = vec![0u8; 32]; // TODO: real X25519

    Ok(HandshakeResponse {
        card: my_card.clone(),
        proof,
        session_key,
    })
}

pub fn verify_handshake_response(
    req: &HandshakeRequest,
    res: &HandshakeResponse,
) -> AcResult<()> {
    if !res.card.verify()? {
        return Err(AcError::Identity("handshake: responder card invalid".into()));
    }

    let vk = crate::identity::decode_did_key(&res.card.did)?;
    let sig = ed25519_dalek::Signature::from_slice(&res.proof)
        .map_err(|e| AcError::Identity(format!("invalid sig: {e}")))?;
    vk.verify_strict(&req.nonce, &sig)
        .map_err(|e| AcError::Identity(format!("nonce sig invalid: {e}")))?;

    Ok(())
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Identity;

    #[test]
    fn test_handshake_full_flow() {
        let alice = Identity::generate();
        let bob = Identity::generate();
        let alice_card = alice.create_card("Alice", "human:alice@test", "gpt-4", &[]).unwrap();
        let bob_card = bob.create_card("Bob", "human:bob@test", "gpt-4", &[]).unwrap();

        let req = build_handshake_request(&alice_card);
        let res = process_handshake(&req, &bob_card, &bob.signing_key).unwrap();
        verify_handshake_response(&req, &res).unwrap();
    }

    #[test]
    fn test_handshake_rejects_invalid_card() {
        let alice = Identity::generate();
        let bob = Identity::generate();
        let mut tampered = alice.create_card("Alice", "human:alice@test", "gpt-4", &[]).unwrap();
        tampered.name = "Mallory".to_string();
        let bob_card = bob.create_card("Bob", "human:bob@test", "gpt-4", &[]).unwrap();

        let req = build_handshake_request(&tampered);
        let result = process_handshake(&req, &bob_card, &bob.signing_key);
        assert!(result.is_err());
    }
}
