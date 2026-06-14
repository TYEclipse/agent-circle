//! Integration tests — mock-based, no external daemon required.

mod common;

use agent_circle::identity::Identity;
use agent_circle::network::build_swarm;
use agent_circle::network::AgentCircleBehaviourEvent;
use common::fixtures::{chat_request_seq, valid_chat_request};
use common::MockNode;
use futures::StreamExt;
use libp2p::request_response::{self, Message};
use libp2p::swarm::SwarmEvent;
use std::time::Duration;

/// Wait for a connection to establish after dialing.
async fn wait_connected(
    swarm: &mut libp2p::Swarm<agent_circle::network::AgentCircleBehaviour>,
    target: &libp2p::PeerId,
    timeout: Duration,
) {
    let dl = tokio::time::Instant::now() + timeout;
    loop {
        if swarm.is_connected(target) {
            return;
        }
        if tokio::time::Instant::now() > dl {
            return;
        }
        tokio::select! {
            _ = swarm.select_next_some() => {}
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
    }
}

/// Sanity: dial a MockNode and confirm it receives a chat message.
#[tokio::test]
async fn mock_node_receives_chat() {
    let node = MockNode::spawn().await.unwrap();
    let id = Identity::generate();
    let mut swarm = build_swarm(&id).unwrap();

    swarm.dial(node.addr.clone()).unwrap();
    wait_connected(&mut swarm, &node.peer_id, Duration::from_secs(5)).await;
    assert!(
        swarm.is_connected(&node.peer_id),
        "connection should be established"
    );

    // ── USE FIXTURE ──
    let msg = valid_chat_request("tester", "hello mock");
    let req_id = swarm.behaviour_mut().chat.send_request(&node.peer_id, msg);

    // Wait for ACK
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    let mut acked = false;
    loop {
        if tokio::time::Instant::now() > deadline {
            break;
        }
        tokio::select! {
            event = swarm.select_next_some() => {
                if let SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Chat(
                    request_response::Event::Message {
                        message: Message::Response { request_id: rid, .. },
                        ..
                    }
                )) = event {
                    if rid == req_id { acked = true; break; }
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(500)) => {}
        }
    }

    tokio::time::sleep(Duration::from_millis(500)).await;
    let received = node.received.lock().unwrap();
    assert!(
        !received.is_empty(),
        "Mock should have received the message, got: {:?}",
        *received
    );
    assert_eq!(received[0], "hello mock");
    assert!(acked, "Sender should have received ACK");
}

/// Multi-message delivery: send 3 messages, verify all received + ACK'd.
#[tokio::test]
async fn mock_node_delivers_multiple() {
    let node = MockNode::spawn().await.unwrap();
    let id = Identity::generate();
    let mut swarm = build_swarm(&id).unwrap();

    swarm.dial(node.addr.clone()).unwrap();
    wait_connected(&mut swarm, &node.peer_id, Duration::from_secs(5)).await;
    assert!(swarm.is_connected(&node.peer_id));

    let count = 3;
    let mut delivered = 0;

    for i in 1..=count {
        // ── USE FIXTURE ──
        let msg = chat_request_seq("tester", &format!("msg-{i}"), i as u64);
        let req_id = swarm.behaviour_mut().chat.send_request(&node.peer_id, msg);

        let dl = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if tokio::time::Instant::now() > dl {
                break;
            }
            tokio::select! {
                event = swarm.select_next_some() => {
                    if let SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Chat(
                        request_response::Event::Message {
                            message: Message::Response { request_id: rid, .. },
                            ..
                        }
                    )) = event {
                        if rid == req_id { delivered += 1; }
                        break;
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(500)) => { break; }
            }
        }
    }

    tokio::time::sleep(Duration::from_millis(500)).await;
    let received = node.received.lock().unwrap();
    assert_eq!(received.len(), 3, "all messages received");
    assert_eq!(delivered, 3, "all ACKs received");
}
