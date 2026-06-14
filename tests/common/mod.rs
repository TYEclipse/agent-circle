//! Lightweight mock P2P node for integration testing.
//!
//! Spawns a libp2p swarm that auto-ACKs incoming chat messages.
//! Tests can send messages to it and verify delivery without needing
//! a separate daemon process.

use agent_circle::chat::ChatResponse;
use agent_circle::errors::AcError;
use agent_circle::identity::Identity;
use agent_circle::network::build_swarm;
use agent_circle::network::AgentCircleBehaviourEvent;
use futures::StreamExt;
use libp2p::request_response::{self, Message};
use libp2p::swarm::SwarmEvent;
use libp2p::PeerId;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

/// A simple in-process P2P node for testing.
pub struct MockNode {
    pub peer_id: PeerId,
    pub addr: libp2p::Multiaddr,
    pub received: Arc<Mutex<Vec<String>>>,
}

impl MockNode {
    /// Spawn a mock node that auto-ACKs chat messages.
    pub async fn spawn() -> Result<Self, AcError> {
        let id = Identity::generate();
        let mut swarm = build_swarm(&id)?;
        let peer_id = *swarm.local_peer_id();

        let received: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        // Wait for a listen address
        let addr = loop {
            tokio::select! {
                event = swarm.select_next_some() => {
                    if let SwarmEvent::NewListenAddr { address, .. } = event {
                        break address;
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(3)) => {
                    panic!("MockNode: no listen address within 3s");
                }
            }
        };

        // Spawn the event loop
        tokio::spawn(async move {
            loop {
                match swarm.select_next_some().await {
                    SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Chat(
                        request_response::Event::Message {
                            peer: _,
                            message:
                                Message::Request {
                                    request, channel, ..
                                },
                            ..
                        },
                    )) => {
                        received_clone.lock().unwrap().push(request.content.clone());
                        let _ = swarm
                            .behaviour_mut()
                            .chat
                            .send_response(channel, ChatResponse { ack: true });
                    }
                    SwarmEvent::NewListenAddr { .. } => {}
                    _ => {}
                }
            }
        });

        Ok(Self {
            peer_id,
            addr,
            received,
        })
    }
}
