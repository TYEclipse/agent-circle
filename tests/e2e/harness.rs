// S18R181 — E2E Test Harness
// Spawns N agent-circle nodes with real libp2p swarms,
// connects them, and provides assertion helpers.
//
// Usage:
//   #[tokio::test]
//   async fn my_e2e_test() {
//       let mut cluster = E2eCluster::spawn(2).await; // 2 nodes
//       cluster.connect_all().await;                    // mesh them
//       let node_a = &mut cluster.nodes[0];
//       let node_b = &mut cluster.nodes[1];
//       // ... assertions ...
//   }

#![allow(dead_code)]

use agent_circle::chat::ChatRequest;
use agent_circle::identity::Identity;
use agent_circle::network;
use agent_circle::network::AgentCircleBehaviourEvent;
use futures::StreamExt;
use libp2p::gossipsub;
use libp2p::swarm::SwarmEvent;
use libp2p::{Multiaddr, PeerId, Swarm};
use std::time::Duration;

/// Default timeout for E2E operations.
pub const E2E_TIMEOUT: Duration = Duration::from_secs(30);

/// Default settle time between actions.
pub const E2E_SETTLE: Duration = Duration::from_secs(2);

/// A single E2E node holding a swarm and identity.
pub struct E2eNode {
    pub name: String,
    pub identity: Identity,
    pub peer_id: PeerId,
    pub swarm: Swarm<network::AgentCircleBehaviour>,
    pub listen_addr: Option<Multiaddr>,
}

/// A cluster of E2E nodes connected via loopback.
pub struct E2eCluster {
    pub nodes: Vec<E2eNode>,
}

impl E2eCluster {
    /// Spawn `n` nodes, each with its own identity and swarm.
    /// Returns after each node has acquired a listen address.
    pub async fn spawn(n: usize) -> Self {
        let mut nodes = Vec::with_capacity(n);
        for i in 0..n {
            let name = format!("node-{i}");
            let identity = Identity::generate();
            let mut swarm = network::build_swarm(&identity)
                .expect("build_swarm should succeed in E2E");

            // Wait for listen address
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

            let peer_id = *swarm.local_peer_id();
            tracing::info!(%name, %peer_id, %addr, "E2E node spawned");

            nodes.push(E2eNode {
                name,
                identity,
                peer_id,
                swarm,
                listen_addr: Some(addr),
            });
        }

        Self { nodes }
    }

    /// Dial every node to every other node in a full mesh.
    /// Skips self-dial.
    pub async fn connect_all(&mut self) {
        // Collect addresses first (borrow issue)
        let addrs: Vec<(usize, Multiaddr)> = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| n.listen_addr.map(|a| (i, a)))
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

        // Wait for connections to establish
        let deadline = tokio::time::Instant::now() + E2E_TIMEOUT;
        let target_peers = self.nodes.len() - 1;
        let mut connected: Vec<usize> = vec![0; self.nodes.len()];

        loop {
            if tokio::time::Instant::now() > deadline {
                break;
            }
            let all_connected = connected.iter().all(|&c| c >= target_peers);
            if all_connected {
                break;
            }

            for i in 0..self.nodes.len() {
                tokio::select! {
                    event = self.nodes[i].swarm.select_next_some() => {
                        if let SwarmEvent::ConnectionEstablished { .. } = event {
                            connected[i] += 1;
                            tracing::info!("{} connected ({} peers)", self.nodes[i].name, connected[i]);
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                }
            }
        }

        tracing::info!(
            peers = ?connected,
            "E2E cluster all connected"
        );
    }

    /// Join all nodes to the given GossipSub group.
    pub fn join_group_all(&mut self, group: &str) {
        for node in &mut self.nodes {
            network::join_group(&mut node.swarm, group)
                .expect("join_group should succeed");
        }
    }

    /// Wait for all nodes to be subscribed to a GossipSub topic.
    pub async fn wait_for_mesh(&mut self, expected_peers: usize) {
        let deadline = tokio::time::Instant::now() + E2E_TIMEOUT;
        let mut meshed = vec![false; self.nodes.len()];

        loop {
            if tokio::time::Instant::now() > deadline {
                break;
            }
            if meshed.iter().all(|&m| m) {
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
                            tracing::info!("{} meshed", self.nodes[i].name);
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(500)) => {}
                }
            }
        }

        assert!(
            meshed.iter().all(|&m| m),
            "Not all nodes meshed: {:?}",
            meshed
        );
    }

    /// Publish a chat message from a node to a group.
    /// Uses network::broadcast_chat.
    pub fn broadcast(&mut self, node_idx: usize, group: &str, content: &str) {
        let node = &self.nodes[node_idx];
        network::broadcast_chat(
            &mut self.nodes[node_idx].swarm,
            group,
            &node.identity.did,
            content,
        );
    }

    /// Wait for a message to be received by any node.
    /// Returns (node_idx, content).
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
                            tracing::info!("{} received: {}", self.nodes[i].name, content);
                            return Some((i, content));
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                }
            }
        }
    }
}

impl Drop for E2eCluster {
    fn drop(&mut self) {
        // Swarms are cleaned up by their Drop impl.
        tracing::info!("E2E cluster shutting down");
    }
}
