// S18 E2E Test Harness — shared module for e2e_tests.rs

#![allow(dead_code)]

use agent_circle::identity::Identity;
use agent_circle::network;
use agent_circle::network::AgentCircleBehaviourEvent;
use futures::StreamExt;
use libp2p::gossipsub;
use libp2p::request_response;
use libp2p::swarm::SwarmEvent;
use libp2p::{Multiaddr, PeerId, Swarm};
use std::time::Duration;

pub const E2E_TIMEOUT: Duration = Duration::from_secs(30);

pub struct E2eNode {
    pub name: String,
    pub identity: Identity,
    pub peer_id: PeerId,
    pub swarm: Swarm<network::AgentCircleBehaviour>,
    pub listen_addr: Option<Multiaddr>,
}

pub struct E2eCluster {
    pub nodes: Vec<E2eNode>,
}

impl E2eCluster {
    pub async fn spawn(n: usize) -> Self {
        let mut nodes = Vec::with_capacity(n);
        for i in 0..n {
            let name = format!("node-{i}");
            let identity = Identity::generate();
            let mut swarm =
                network::build_swarm(&identity).expect("build_swarm should succeed in E2E");

            let addr = loop {
                tokio::select! {
                    event = swarm.select_next_some() => {
                        if let SwarmEvent::NewListenAddr { address, .. } = event {
                            break address;
                        }
                    }
                    _ = tokio::time::sleep(E2E_TIMEOUT) => {
                        panic!("{name}: no listen addr within timeout");
                    }
                }
            };

            nodes.push(E2eNode {
                name,
                peer_id: *swarm.local_peer_id(),
                identity,
                swarm,
                listen_addr: Some(addr),
            });
        }
        Self { nodes }
    }

    #[allow(clippy::needless_range_loop)]
    pub async fn connect_all(&mut self) {
        let addrs: Vec<(usize, Multiaddr)> = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| n.listen_addr.as_ref().map(|a| (i, a.clone())))
            .collect();

        for i in 0..self.nodes.len() {
            for (j, addr) in &addrs {
                if i == *j {
                    continue;
                }
                self.nodes[i]
                    .swarm
                    .dial(addr.clone())
                    .expect("dial should succeed in loopback E2E");
            }
        }

        let deadline = tokio::time::Instant::now() + E2E_TIMEOUT;
        let target = self.nodes.len() - 1;
        let mut connected = vec![0; self.nodes.len()];

        loop {
            if tokio::time::Instant::now() > deadline {
                break;
            }
            if connected.iter().all(|&c| c >= target) {
                break;
            }
            for i in 0..self.nodes.len() {
                tokio::select! {
                    event = self.nodes[i].swarm.select_next_some() => {
                        if let SwarmEvent::ConnectionEstablished { .. } = event {
                            connected[i] += 1;
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                }
            }
        }
    }

    pub fn join_group_all(&mut self, group: &str) {
        for node in &mut self.nodes {
            network::join_group(&mut node.swarm, group).expect("join_group");
        }
    }

    #[allow(clippy::needless_range_loop)]
    pub async fn wait_for_mesh(&mut self) {
        let deadline = tokio::time::Instant::now() + E2E_TIMEOUT;
        let mut meshed = vec![false; self.nodes.len()];

        loop {
            if tokio::time::Instant::now() > deadline || meshed.iter().all(|&m| m) {
                break;
            }
            for i in 0..self.nodes.len() {
                if meshed[i] {
                    continue;
                }
                tokio::select! {
                    event = self.nodes[i].swarm.select_next_some() => {
                        if let SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Gossip(
                            gossipsub::Event::Subscribed { .. },
                        )) = event {
                            meshed[i] = true;
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(500)) => {}
                }
            }
        }
        assert!(meshed.iter().all(|&m| m), "Not all meshed: {meshed:?}");
    }

    pub fn broadcast(&mut self, node_idx: usize, group: &str, content: &str) {
        let did = self.nodes[node_idx].identity.did.clone();
        let _ = network::send_group_message(&mut self.nodes[node_idx].swarm, group, &did, content);
    }

    pub fn send_chat(&mut self, from_idx: usize, to_idx: usize, content: &str) {
        let to_peer = self.nodes[to_idx].peer_id;
        let did = self.nodes[from_idx].identity.did.clone();
        network::send_chat(&mut self.nodes[from_idx].swarm, to_peer, &did, content);
    }

    pub async fn wait_for_chat(&mut self, node_idx: usize, timeout: Duration) -> Option<String> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if tokio::time::Instant::now() > deadline {
                return None;
            }
            tokio::select! {
                event = self.nodes[node_idx].swarm.select_next_some() => {
                    if let SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Chat(
                        request_response::Event::Message {
                            message: request_response::Message::Request { request, .. },
                            ..
                        },
                    )) = event {
                        return Some(request.content);
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {}
            }
        }
    }

    pub async fn wait_for_message(&mut self, timeout: Duration) -> Option<(usize, String)> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if tokio::time::Instant::now() > deadline {
                return None;
            }
            for i in 0..self.nodes.len() {
                tokio::select! {
                    event = self.nodes[i].swarm.select_next_some() => {
                        if let SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Gossip(
                            gossipsub::Event::Message { message, .. },
                        )) = event {
                            let content = String::from_utf8_lossy(&message.data).to_string();
                            return Some((i, content));
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                }
            }
        }
    }

    pub fn peer_id(&self, node_idx: usize) -> PeerId {
        *self.nodes[node_idx].swarm.local_peer_id()
    }
}

impl Drop for E2eCluster {
    fn drop(&mut self) {
        tracing::info!("E2E cluster shutting down");
    }
}
