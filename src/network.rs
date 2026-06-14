//! Network module — P2P swarm powered by libp2p
//!
//! Transports: QUIC + TCP + Relay (circuit)
//! Discovery: mDNS (LAN) + Kademlia DHT (WAN)
//! Chat: request/response (1-to-1) + GossipSub (group)
//! Relay: enables NAT traversal when DCUtR hole-punching fails
//! Relay discovery: relay nodes broadcast their address via DHT

use crate::chat::{ChatRequest, ChatResponse};
use crate::dedup::DedupFilter;
use crate::diag::DiagCounters;
use crate::errors::{AcError, AcResult};
use crate::identity::Identity;
use crate::message_queue;
use crate::reliability::{PendingTracker, MAX_RETRIES};
use futures::StreamExt;
use libp2p::{
    dcutr, gossipsub, identify, kad,
    kad::{Record, RecordKey},
    mdns, relay,
    request_response::{self, Message},
    swarm::{NetworkBehaviour, SwarmEvent},
    yamux, PeerId, StreamProtocol, Swarm, SwarmBuilder,
};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::net::Ipv4Addr;
use std::time::Duration;
use tracing::{debug, info, warn};

/// DHT key for relay node discovery. Relay nodes publish their addresses under this record key.
const RELAY_DHT_KEY: &str = "/agent-circle/relays/0.1.0";

pub type ChatBehaviour = request_response::json::Behaviour<ChatRequest, ChatResponse>;

#[derive(NetworkBehaviour)]
pub struct AgentCircleBehaviour {
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub mdns: mdns::tokio::Behaviour,
    pub identify: identify::Behaviour,
    pub dcutr: dcutr::Behaviour,
    pub relay: relay::Behaviour,
    pub chat: ChatBehaviour,
    pub gossip: gossipsub::Behaviour,
}

pub fn build_swarm(id: &Identity) -> AcResult<Swarm<AgentCircleBehaviour>> {
    let libp2p_keypair = ed25519_to_libp2p_keypair(id)?;
    let local_peer_id = PeerId::from(libp2p_keypair.public());

    let relay_config = relay::Config::default();

    let mut swarm = SwarmBuilder::with_existing_identity(libp2p_keypair.clone())
        .with_tokio()
        .with_quic()
        .with_relay_client(libp2p::noise::Config::new, yamux::Config::default)
        .expect("relay transport")
        .with_behaviour(move |key, _relay_client_behaviour| {
            let mut kademlia =
                kad::Behaviour::new(local_peer_id, kad::store::MemoryStore::new(local_peer_id));
            kademlia.set_mode(Some(kad::Mode::Server));
            let mdns =
                mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id).expect("mDNS");
            let identify = identify::Behaviour::new(
                identify::Config::new("/agent-circle/0.1.0".to_string(), libp2p_keypair.public())
                    .with_agent_version(format!("agent-circle/{}", env!("CARGO_PKG_VERSION"))),
            );
            let dcutr = dcutr::Behaviour::new(local_peer_id);
            let chat = ChatBehaviour::new(
                [(
                    StreamProtocol::new("/agent-circle/chat/0.1.0"),
                    request_response::ProtocolSupport::Full,
                )],
                request_response::Config::default(),
            );

            // GossipSub for group chat — mesh-based pub/sub
            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(2))
                .flood_publish(true)
                .build()
                .expect("gossipsub config");
            let gossip = gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(libp2p_keypair.clone()),
                gossipsub_config,
            )
            .expect("gossipsub");

            Ok(AgentCircleBehaviour {
                kademlia,
                mdns,
                identify,
                dcutr,
                relay: relay::Behaviour::new(key.public().to_peer_id(), relay_config),
                chat,
                gossip,
            })
        })
        .map_err(|e| AcError::Network(format!("behaviour: {e}")))?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    swarm
        .listen_on(
            libp2p::multiaddr::Multiaddr::empty()
                .with(libp2p::multiaddr::Protocol::Ip4(Ipv4Addr::UNSPECIFIED))
                .with(libp2p::multiaddr::Protocol::Udp(0))
                .with(libp2p::multiaddr::Protocol::QuicV1),
        )
        .map_err(|e| AcError::Network(format!("listen: {e}")))?;

    info!(peer_id = %local_peer_id, "Swarm已构建");
    Ok(swarm)
}

// ── 1-to-1 Chat ────────────────────────────────────────────────────

pub fn send_chat(
    swarm: &mut Swarm<AgentCircleBehaviour>,
    peer_id: PeerId,
    from: &str,
    content: &str,
) {
    let msg = ChatRequest {
        from: from.to_string(),
        content: content.to_string(),
        ts: chrono::Utc::now().timestamp(),
        msg_id: crate::chat::new_msg_id(),
        ttl: crate::chat::default_ttl(),
    };
    swarm.behaviour_mut().chat.send_request(&peer_id, msg);
}

// ── Group Chat (GossipSub) ─────────────────────────────────────────

/// Derive a deterministic topic identifier from a human-readable group name.
pub fn group_topic(name: &str) -> gossipsub::IdentTopic {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    let hash = hasher.finish();
    gossipsub::IdentTopic::new(format!("agent-circle/group/{hash:x}"))
}

/// Join a group topic. Call this before sending/receiving group messages.
pub fn join_group(swarm: &mut Swarm<AgentCircleBehaviour>, name: &str) -> AcResult<()> {
    let topic = group_topic(name);
    swarm
        .behaviour_mut()
        .gossip
        .subscribe(&topic)
        .map_err(|e| AcError::Network(format!("订阅群组失败: {e}")))?;
    Ok(())
}

/// Send a message to a group topic.
pub fn send_group_message(
    swarm: &mut Swarm<AgentCircleBehaviour>,
    name: &str,
    from: &str,
    content: &str,
) -> AcResult<()> {
    let topic = group_topic(name);
    let msg = serde_json::json!({
        "from": from,
        "content": content,
        "ts": chrono::Utc::now().timestamp(),
    });
    let data = serde_json::to_vec(&msg)?;
    swarm
        .behaviour_mut()
        .gossip
        .publish(topic, data)
        .map_err(|e| AcError::Network(format!("群发失败: {e}")))?;
    Ok(())
}

/// List topics the node is currently subscribed to.
pub fn list_group_topics(swarm: &Swarm<AgentCircleBehaviour>) -> Vec<String> {
    swarm
        .behaviour()
        .gossip
        .topics()
        .map(|t| t.to_string())
        .collect()
}

// ── Key bridge ─────────────────────────────────────────────────────

fn ed25519_to_libp2p_keypair(id: &Identity) -> AcResult<libp2p::identity::Keypair> {
    let mut seed = id.to_seed_bytes();
    let secret = libp2p::identity::ed25519::SecretKey::try_from_bytes(&mut seed)
        .map_err(|e| AcError::Key(format!("libp2p key: {e}")))?;
    Ok(libp2p::identity::Keypair::from(
        libp2p::identity::ed25519::Keypair::from(secret),
    ))
}

// ── Daemon ─────────────────────────────────────────────────────────

pub async fn run_daemon(
    id: &Identity,
    groups: &[String],
    relay_mode: bool,
    data_dir: &std::path::Path,
) -> AcResult<()> {
    let mut swarm = build_swarm(id)?;
    let local_peer_id = *swarm.local_peer_id();

    // Join specified groups at startup
    for name in groups {
        join_group(&mut swarm, name)?;
        let topic = group_topic(name);
        info!(name = %name, topic = %topic, "已加入群组");
    }

    info!("Agent Circle 守护进程已启动");
    info!(peer_id = %local_peer_id, relay_mode = relay_mode, "PeerId");

    let mut bootstrapped = false;
    let mut relay_registered_or_discovered = false;
    let mut pending = PendingTracker::new();
    let mut dedup = DedupFilter::new();
    let counters = DiagCounters::default();
    let started = std::time::Instant::now();
    let mut stats_timer = tokio::time::interval(std::time::Duration::from_secs(30));
    stats_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut tick_count: u64 = 0;

    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!(addr = %address, "监听地址");
            }

            SwarmEvent::ConnectionEstablished {
                peer_id,
                num_established,
                ..
            } => {
                info!(peer_id = %peer_id, connections = num_established, "已连接");
                // Flush offline queue for this peer
                if let Ok(q) = message_queue::Queue::open(data_dir) {
                    let peer_str = peer_id.to_string();
                    let pending_msgs = q.pending_for(&peer_str).unwrap_or_default();
                    for entry in pending_msgs {
                        info!(peer = %peer_str, msg = %entry.content, "📤 重试离线消息");
                        let chat_req = ChatRequest {
                            from: id.short_code.clone(),
                            content: entry.content.clone(),
                            ts: chrono::Utc::now().timestamp(),
                            msg_id: crate::chat::new_msg_id(),
                            ttl: entry.expires_at.unwrap_or(i64::MAX),
                        };
                        let req_id = swarm
                            .behaviour_mut()
                            .chat
                            .send_request(&peer_id, chat_req.clone());
                        pending.track(
                            req_id,
                            peer_id,
                            id.short_code.clone(),
                            entry.content.clone(),
                            chat_req.ts,
                            chat_req.msg_id,
                            chat_req.ttl,
                        );
                        counters.inc_sent();
                        let _ = q.mark_delivered(entry.id);
                        info!(peer = %peer_str, "✅ 离线消息已发送 （等待ACK）");
                    }
                }
            }

            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                info!(peer_id = %peer_id, cause = ?cause, "断开");
            }

            SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Mdns(mdns::Event::Discovered(
                list,
            ))) => {
                for (peer_id, addr) in list {
                    info!(peer_id = %peer_id, addr = %addr, "mDNS发现");
                    swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, addr.clone());
                    swarm.behaviour_mut().gossip.add_explicit_peer(&peer_id);
                    // Dial the peer so GossipSub can form a mesh
                    let _ = swarm.dial(addr);
                }
            }

            SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Identify(
                identify::Event::Received { peer_id, info, .. },
            )) => {
                info!(peer_id = %peer_id, agent_version = %info.agent_version, "Identify");
            }

            // ── Chat: incoming 1-to-1 message ──────────────────────
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
                if dedup.is_dup(request.msg_id) {
                    debug!(msg_id = request.msg_id, "🔄 重复消息，已跳过");
                    counters.inc_duplicate();
                } else {
                    info!(from = %request.from, msg_id = request.msg_id, content = %request.content, "收到私聊");
                }
                // Always ACK — even for duplicates — so sender knows it arrived
                let _ = swarm
                    .behaviour_mut()
                    .chat
                    .send_response(channel, ChatResponse { ack: true });
            }

            // ── Chat: ACK received ──────────────────────────
            SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Chat(
                request_response::Event::Message {
                    peer,
                    message: Message::Response { request_id, .. },
                    ..
                },
            )) => {
                if let Some(entry) = pending.ack(&request_id) {
                    info!(
                        peer = %peer,
                        content = %entry.content,
                        retries = entry.retries,
                        elapsed_ms = entry.created_at.elapsed().as_millis(),
                        "✅ ACK — 消息已送达"
                    );
                    counters.inc_acked();
                }
            }

            SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Chat(
                request_response::Event::OutboundFailure {
                    peer,
                    request_id,
                    error,
                    ..
                },
            )) => {
                warn!(peer = %peer, error = ?error, "消息发送失败");
                match pending.fail(&request_id) {
                    Some(entry) if entry.retries <= MAX_RETRIES => {
                        // Within budget — retry immediately
                        info!(
                            peer = %peer,
                            retry = entry.retries,
                            max = MAX_RETRIES,
                            "🔄 重试发送"
                        );
                        let chat_req = ChatRequest {
                            from: entry.from.clone(),
                            content: entry.content.clone(),
                            ts: chrono::Utc::now().timestamp(),
                            msg_id: entry.msg_id,
                            ttl: entry.ttl,
                        };
                        let new_id = swarm.behaviour_mut().chat.send_request(&peer, chat_req);
                        pending.retrack(new_id, entry);
                        counters.inc_retried();
                    }
                    Some(entry) => {
                        // Retries exhausted — hand off to offline queue
                        info!(
                            peer = %peer,
                            retries = entry.retries,
                            "📥 重试耗尽，存入离线队列"
                        );
                        counters.inc_failed();
                        match message_queue::Queue::open(data_dir) {
                            Ok(q) => {
                                if let Err(e) = q.push_with_ttl(&peer.to_string(), &entry.content, Some(entry.ttl)) {
                                    warn!(error = %e, "离线队列入队失败");
                                } else {
                                    counters.inc_queued();
                                }
                            }
                            Err(e) => warn!(error = %e, "无法打开离线队列"),
                        }
                    }
                    None => {
                        debug!(peer = %peer, "OutboundFailure 未跟踪消息");
                    }
                }
            }

            SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Chat(_)) => {}

            // ── GossipSub: incoming group message ───────────────────
            SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Gossip(
                gossipsub::Event::Message { message, .. },
            )) => match serde_json::from_slice::<serde_json::Value>(&message.data) {
                Ok(msg) => {
                    let from = msg["from"].as_str().unwrap_or("unknown");
                    let content = msg["content"].as_str().unwrap_or("");
                    let topic_name = message.topic.to_string();
                    info!(topic = %topic_name, from = %from, content = %content, "群聊消息");
                }
                Err(_) => {
                    warn!(topic = %message.topic, "无法解析群聊消息");
                }
            },

            SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Gossip(_)) => {}

            SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Kademlia(
                kad::Event::RoutingUpdated {
                    peer, is_new_peer, ..
                },
            )) => {
                if is_new_peer {
                    debug!(peer = %peer, "DHT路由更新");
                }
            }

            SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    result: kad::QueryResult::GetRecord(Ok(kad::GetRecordOk::FoundRecord(record))),
                    ..
                },
            )) => {
                let key_str = std::str::from_utf8(record.record.key.as_ref()).unwrap_or("?");
                if key_str == RELAY_DHT_KEY {
                    let addrs = String::from_utf8_lossy(&record.record.value);
                    info!(relay_addrs = %addrs, "🔗 DHT 发现 relay 节点");
                    // Parse relay address and dial
                    for addr_str in addrs.split(',') {
                        if let Ok(addr) = addr_str.parse::<libp2p::Multiaddr>() {
                            info!(relay = %addr_str, "拨号 relay 节点");
                            let _ = swarm.dial(addr);
                        }
                    }
                }
            }
            SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Kademlia(
                kad::Event::OutboundQueryProgressed {
                    result:
                        kad::QueryResult::GetRecord(Ok(
                            kad::GetRecordOk::FinishedWithNoAdditionalRecord { .. },
                        )),
                    ..
                },
            )) => {
                info!("DHT 未发现 relay 节点（网络中尚无 relay 在线）");
            }
            SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Kademlia(_)) => {}
            SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Dcutr(event)) => {
                debug!(?event, "DCUtR");
            }
            // ── Relay: reservation request accepted ────────────────
            SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Relay(
                relay::Event::ReservationReqAccepted {
                    src_peer_id,
                    renewed,
                },
            )) => {
                info!(
                    peer_id = %src_peer_id,
                    renewed = renewed,
                    "Relay reservation"
                );
            }
            SwarmEvent::Behaviour(AgentCircleBehaviourEvent::Relay(event)) => {
                debug!(?event, "Relay");
            }
            _ => {}
        }
            } // end match → event arm
            _ = stats_timer.tick() => {
                tick_count += 1;
                let q_stats = message_queue::Queue::open(data_dir)
                    .ok()
                    .and_then(|q| q.stats().ok())
                    .unwrap_or((0, 0, 0));
                let snap = counters.snapshot(pending.len(), q_stats, started);
                info!("{}", crate::diag::format_snapshot(&snap));

                // Every 5 minutes (10 ticks × 30s): purge expired + delivered messages
                if tick_count.is_multiple_of(10) {
                    if let Ok(q) = message_queue::Queue::open(data_dir) {
                        let now = chrono::Utc::now().timestamp();
                        match q.expire_before(now) {
                            Ok(n) if n > 0 => info!(expired = n, "🧹 已清理过期离线消息"),
                            Ok(_) => {}
                            Err(e) => warn!(error = %e, "过期清理失败"),
                        }
                        match q.prune_delivered() {
                            Ok(n) if n > 0 => info!(pruned = n, "🧹 已清理已送达记录"),
                            Ok(_) => {}
                            Err(e) => warn!(error = %e, "已送达清理失败"),
                        }
                    }
                }
            }
        } // end tokio::select!

        if !bootstrapped && swarm.listeners().next().is_some() {
            if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                warn!(error = %e, "Kademlia bootstrap 失败");
            } else {
                info!("Kademlia DHT bootstrap 已启动");
                bootstrapped = true;
            }
        }

        // After bootstrap, register or discover relay nodes via DHT
        if bootstrapped && !relay_registered_or_discovered {
            let relay_key = RecordKey::new(&RELAY_DHT_KEY);
            if relay_mode {
                // Publish relay address to DHT so other nodes can discover us
                let addrs: Vec<String> = swarm
                    .listeners()
                    .chain(swarm.external_addresses())
                    .map(|a| a.to_string())
                    .collect();
                if !addrs.is_empty() {
                    let value = addrs.join(",");
                    if let Err(e) = swarm.behaviour_mut().kademlia.put_record(
                        Record::new(relay_key, value.into_bytes()),
                        libp2p::kad::Quorum::One,
                    ) {
                        warn!(error = %e, "DHT relay 注册失败");
                    } else {
                        info!(addrs = %addrs.join(" "), "🔁 DHT relay 地址已注册");
                        relay_registered_or_discovered = true;
                    }
                }
            } else {
                // Query DHT to discover relay nodes
                info!("🔍 查询 DHT 发现 relay 节点...");
                swarm.behaviour_mut().kademlia.get_record(relay_key);
                relay_registered_or_discovered = true;
            }
        }
    }
}
