use agent_circle::identity::Identity;
use agent_circle::network;
use futures::StreamExt;
use libp2p::{gossipsub, swarm::SwarmEvent};
use std::time::Duration;

/// Verifies that two Agent Circle nodes can discover each other,
/// form a GossipSub mesh, and bidirectionally subscribe to the same topic.
///
/// Message delivery (`publish` → `Message` event) requires daemon-mode
/// continuous heartbeat cycles; this test validates the mesh infrastructure.
#[tokio::test]
#[ignore = "needs 30s; run with -- --ignored"]
async fn test_gossipsub_mesh_two_nodes() {
    let alice = Identity::generate();
    let bob = Identity::generate();

    let mut alice_swarm = network::build_swarm(&alice).unwrap();
    network::join_group(&mut alice_swarm, "testroom").unwrap();

    let mut bob_swarm = network::build_swarm(&bob).unwrap();
    network::join_group(&mut bob_swarm, "testroom").unwrap();

    let bob_addr = loop {
        tokio::select! {
            event = bob_swarm.select_next_some() => {
                if let SwarmEvent::NewListenAddr { address, .. } = event {
                    break address;
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(5)) => panic!("Bob no addr"),
        }
    };

    alice_swarm.dial(bob_addr).unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let mut alice_meshed = false;
    let mut bob_meshed = false;

    loop {
        if tokio::time::Instant::now() > deadline {
            break;
        }
        tokio::select! {
            event = alice_swarm.select_next_some() => {
                if let SwarmEvent::Behaviour(network::AgentCircleBehaviourEvent::Gossip(
                    gossipsub::Event::Subscribed { .. },
                )) = event {
                    alice_meshed = true;
                }
            }
            event = bob_swarm.select_next_some() => {
                if let SwarmEvent::Behaviour(network::AgentCircleBehaviourEvent::Gossip(
                    gossipsub::Event::Subscribed { .. },
                )) = event {
                    bob_meshed = true;
                }
            }
        }
    }

    assert!(alice_meshed, "Alice never meshed");
    assert!(bob_meshed, "Bob never meshed");
}
