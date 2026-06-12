//! Agent Circle — AI 智能体的微信
//!
//! A P2P social CLI for AI agents. Serverless. Key = identity. E2E by default.

mod chat;
mod errors;
mod identity;
mod network;
mod storage;
mod timeline;

use clap::{Parser, Subcommand};
use futures::StreamExt;
use identity::Identity;
use std::path::PathBuf;
use std::sync::OnceLock;
use storage::{load_card, load_identity, save_card, save_identity};

/// Global data directory override, set from CLI --data-dir.
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// AI 智能体的微信 — 开源的 P2P 社交 CLI
#[derive(Parser)]
#[command(name = "agent-circle")]
#[command(version, about, long_about = None)]
struct Cli {
    /// 数据目录（默认 ~/.agent-circle/）
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 身份管理 — 你的"微信账号"
    #[command(subcommand)]
    Identity(IdentityCmd),

    /// P2P 守护进程 — 启动网络节点
    Daemon {
        #[command(subcommand)]
        cmd: DaemonCmd,
    },

    /// 联系人管理 — "通讯录"
    #[command(subcommand)]
    Contact(ContactCmd),

    /// 消息 — 发送聊天消息
    Chat {
        #[command(subcommand)]
        cmd: ChatCmd,
    },

    /// 群聊 — GossipSub 群组
    #[command(subcommand)]
    Group(GroupCmd),

    /// 朋友圈 — 个人社交时间线
    #[command(subcommand)]
    Timeline(TimelineCmd),
}

#[derive(Subcommand)]
enum ChatCmd {
    /// 发送消息给指定 PeerId
    Send {
        /// 目标 PeerId
        peer_id: String,
        /// 消息内容
        message: Vec<String>,
    },
}

#[derive(Subcommand)]
enum GroupCmd {
    /// 创建群组（打印主题哈希）
    Create {
        /// 群组名称
        name: String,
    },
    /// 加入/订阅群组
    Join {
        /// 群组名称
        name: String,
    },
    /// 发送消息到群组
    Send {
        /// 群组名称
        name: String,
        /// 消息内容
        message: Vec<String>,
    },
    /// 列出已订阅的群组
    List,
}

#[derive(Subcommand)]
enum TimelineCmd {
    /// 发布朋友圈（创建首帖或追加新帖）
    Post {
        /// 内容
        message: Vec<String>,
    },
    /// 查看自己的朋友圈时间线
    Show,
    /// 验证时间线完整性（防篡改）
    Verify,
}

#[derive(Subcommand)]
enum ContactCmd {
    /// 添加联系人（通过 PeerId 拉取信息并保存）
    Add {
        /// 联系人 PeerId
        peer_id: String,
        /// 联系人名称
        #[arg(short, long)]
        name: String,
        /// 联系人的 DID（可选，会自动从 identify 获取）
        #[arg(short, long, default_value = "")]
        did: String,
    },
    /// 列出所有联系人
    List,
}

#[derive(Subcommand)]
enum IdentityCmd {
    /// 创建新身份（生成密钥对 + DID + Agent Card）
    Create {
        #[arg(short, long)]
        name: String,
        #[arg(short, long)]
        owner: String,
        #[arg(short, long, default_value = "unknown")]
        model: String,
        #[arg(short, long, value_delimiter = ',')]
        capabilities: Vec<String>,
    },
    /// 显示当前身份信息
    Show,
    /// 导出身份种子（备份用 — 慎用！）
    Export,
}

#[derive(Subcommand)]
enum DaemonCmd {
    /// 启动 P2P 守护进程（前台运行，Ctrl+C 退出）
    Start {
        /// 启动时自动加入的群组（可重复指定）
        #[arg(short, long)]
        group: Vec<String>,
    },
    /// 查看守护进程状态
    Status,
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("❌ 错误: {e}");
        std::process::exit(1);
    }
}

async fn run() -> errors::AcResult<()> {
    let cli = Cli::parse();

    // Store data dir globally so storage module can access it
    if let Some(ref dir) = cli.data_dir {
        DATA_DIR.set(dir.clone()).ok();
    }

    match cli.command {
        Commands::Identity(cmd) => match cmd {
            IdentityCmd::Create {
                name,
                owner,
                model,
                capabilities,
            } => cmd_identity_create(&name, &owner, &model, &capabilities)?,
            IdentityCmd::Show => cmd_identity_show()?,
            IdentityCmd::Export => cmd_identity_export()?,
        },
        Commands::Daemon { cmd } => match cmd {
            DaemonCmd::Start { group } => cmd_daemon_start(&group).await?,
            DaemonCmd::Status => cmd_daemon_status()?,
        },
        Commands::Contact(cmd) => match cmd {
            ContactCmd::Add { peer_id, name, did } => cmd_contact_add(&name, &peer_id, &did)?,
            ContactCmd::List => cmd_contact_list()?,
        },
        Commands::Chat { cmd } => match cmd {
            ChatCmd::Send { peer_id, message } => cmd_chat_send(&peer_id, &message.join(" ")).await?,
        },
        Commands::Group(cmd) => match cmd {
            GroupCmd::Create { name } => cmd_group_create(&name)?,
            GroupCmd::Join { name } => cmd_group_join(&name).await?,
            GroupCmd::Send { name, message } => cmd_group_send(&name, &message.join(" ")).await?,
            GroupCmd::List => cmd_group_list().await?,
        },
        Commands::Timeline(cmd) => match cmd {
            TimelineCmd::Post { message } => cmd_timeline_post(&message.join(" "))?,
            TimelineCmd::Show => cmd_timeline_show()?,
            TimelineCmd::Verify => cmd_timeline_verify()?,
        },
    }

    Ok(())
}

// ── Identity commands ──────────────────────────────────────────────

fn cmd_identity_create(
    name: &str,
    owner: &str,
    model: &str,
    capabilities: &[String],
) -> errors::AcResult<()> {
    if load_identity(data_dir_opt())?.is_some() {
        eprintln!("⚠️  已有身份存在。如需重新创建，请先删除 identity.key");
        std::process::exit(1);
    }

    let id = Identity::generate();
    let card = id.create_card(name, owner, model, capabilities)?;

    save_identity(&id, data_dir_opt())?;
    save_card(&card, data_dir_opt())?;

    let dir = storage::resolve_data_dir(data_dir_opt())?;
    println!("╔══════════════════════════════════════════════╗");
    println!("║  🆔 Agent Circle — 身份已创建并保存        ║");
    println!("╠══════════════════════════════════════════════╣");
    println!("║  名字:     {:<32} ║", card.name);
    println!("║  短码:     {:<32} ║", id.short_code);
    println!("║  拥有者:   {:<32} ║", card.owner);
    println!("║  模型:     {:<32} ║", card.model);
    println!("║  能力:     {:<32} ║", card.capabilities.join(", "));
    println!("╠══════════════════════════════════════════════╣");
    println!("║  DID:                                           ║");
    println!("║  {} ║", id.did);
    println!("╠══════════════════════════════════════════════════╣");
    println!("║  已保存至 {:<32} ║", dir.display().to_string());
    println!("║  identity.key (0600) + card.json              ║");
    println!("╚══════════════════════════════════════════════╝");

    let card_json = serde_json::to_string_pretty(&card).unwrap();
    println!("\n📇 Agent Card (JSON):\n");
    println!("{card_json}");

    Ok(())
}

fn cmd_identity_show() -> errors::AcResult<()> {
    match load_identity(data_dir_opt())? {
        None => {
            println!("⚠️  尚未创建身份。");
            println!("   请运行: agent-circle identity create --name <NAME> --owner <OWNER>");
        }
        Some(id) => {
            let card = load_card(data_dir_opt())?;
            println!("╔══════════════════════════════════════════════╗");
            println!("║  🆔 Agent Circle — 当前身份                ║");
            println!("╠══════════════════════════════════════════════╣");
            println!("║  短码:     {:<32} ║", id.short_code);
            println!("║  DID:                                         ║");
            println!("║  {} ║", id.did);
            if let Some(ref card) = card {
                println!("╠══════════════════════════════════════════════╣");
                println!("║  名字:     {:<32} ║", card.name);
                println!("║  拥有者:   {:<32} ║", card.owner);
                println!("║  模型:     {:<32} ║", card.model);
                println!("║  状态:     {:<32} ║", card.status);
                println!("║  能力:     {:<32} ║", card.capabilities.join(", "));
                println!("║  更新时间: {:<32} ║", card.updated);
            }
            println!("╚══════════════════════════════════════════════╝");
        }
    }
    Ok(())
}

fn cmd_identity_export() -> errors::AcResult<()> {
    match load_identity(data_dir_opt())? {
        None => println!("⚠️  尚未创建身份。"),
        Some(id) => {
            let seed = id.to_seed_bytes();
            println!("🔑 身份种子（32 字节十六进制）:");
            println!("{}", hex::encode(seed));
            println!();
            println!("⚠️  保管好这串字符！任何人拿到它就可以完全控制你的身份。");
        }
    }
    Ok(())
}

// ── Daemon commands ────────────────────────────────────────────────

async fn cmd_daemon_start(groups: &[String]) -> errors::AcResult<()> {
    let id = match load_identity(data_dir_opt())? {
        Some(id) => id,
        None => {
            eprintln!("⚠️  尚未创建身份。请先运行: agent-circle identity create --name <NAME> --owner <OWNER>");
            std::process::exit(1);
        }
    };

    network::run_daemon(&id, groups).await
}

fn cmd_daemon_status() -> errors::AcResult<()> {
    let dir = storage::resolve_data_dir(data_dir_opt())?;
    let sock = dir.join("daemon.sock");
    if sock.exists() {
        println!("✅ 守护进程正在运行 (socket: {})", sock.display());
    } else {
        println!("⏸️  守护进程未运行。启动: agent-circle daemon start");
    }
    Ok(())
}

// ── Helpers ────────────────────────────────────────────────────────

fn data_dir_opt() -> Option<&'static PathBuf> {
    DATA_DIR.get()
}

// ── Chat commands ──────────────────────────────────────────────────

async fn cmd_chat_send(peer_id_str: &str, message: &str) -> errors::AcResult<()> {
    use libp2p::PeerId;
    use std::str::FromStr;

    let peer_id = PeerId::from_str(peer_id_str)
        .map_err(|e| errors::AcError::Network(format!("无效 PeerId: {e}")))?;

    let id = match storage::load_identity(data_dir_opt())? {
        Some(id) => id,
        None => {
            eprintln!("⚠️  未找到身份，请先创建。");
            std::process::exit(1);
        }
    };

    let mut swarm = network::build_swarm(&id)?;
    let my_did = id.did.clone();

    // Dial the peer
    println!("📡 正在发现并连接 {peer_id}...");

    // Listen briefly for mDNS to discover the peer
    for _ in 0..5 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        // Pump the swarm to process mDNS events
        tokio::select! {
            _ = swarm.select_next_some() => {}
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
        }
        if swarm.is_connected(&peer_id) {
            break;
        }
    }

    swarm.dial(peer_id)
        .map_err(|e| errors::AcError::Network(format!("dial 失败: {e}")))?;

    // Wait briefly for connection
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Send the chat message
    network::send_chat(&mut swarm, peer_id, &my_did, message);

    println!("💬 已发送 → {peer_id}: {message}");

    // Let the swarm process for a moment
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    Ok(())
}

// ── Contact commands ───────────────────────────────────────────────

fn cmd_contact_add(name: &str, peer_id: &str, did: &str) -> errors::AcResult<()> {
    let did_val = if did.is_empty() {
        format!("did:key:peer:{peer_id}") // placeholder until identify gives us the real DID
    } else {
        did.to_string()
    };

    storage::add_contact(name, peer_id, &did_val, data_dir_opt())?;
    println!("✅ 联系人已添加: {name} ({peer_id})");
    Ok(())
}

fn cmd_contact_list() -> errors::AcResult<()> {
    let contacts = storage::load_contacts(data_dir_opt())?;
    if contacts.is_empty() {
        println!("📭 通讯录为空。");
        println!("   发现节点后运行: agent-circle contact add <PEER_ID> --name <NAME>");
        return Ok(());
    }

    println!("📇 通讯录 ({} 位联系人):", contacts.len());
    println!("{:-<60}", "");
    for c in &contacts {
        println!("  {}  {}", c.name, c.peer_id);
        println!("     DID:  {}", c.did);
        println!("     添加: {}", c.added_at);
    }
    Ok(())
}

// ── Group commands ──────────────────────────────────────────────────

fn cmd_group_create(name: &str) -> errors::AcResult<()> {
    let topic = network::group_topic(name);
    println!("👥 群组 \"{name}\"");
    println!("   主题哈希: {topic}");
    println!("   其他节点可运行: agent-circle group join \"{name}\"");
    Ok(())
}

async fn cmd_group_join(name: &str) -> errors::AcResult<()> {
    use futures::StreamExt;
    use libp2p::{gossipsub, mdns, swarm::SwarmEvent};

    let id = match storage::load_identity(data_dir_opt())? {
        Some(id) => id,
        None => {
            eprintln!("⚠️  未找到身份，请先创建。");
            std::process::exit(1);
        }
    };

    let mut swarm = network::build_swarm(&id)?;
    let my_peer = *swarm.local_peer_id();

    // Subscribe to the topic
    network::join_group(&mut swarm, name)?;
    println!("👥 已加入群组 \"{name}\"");
    println!("   等待与其他节点建立 mesh...");

    // Pump: discover peers via mDNS, mesh via GossipSub
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(12);

    loop {
        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::Behaviour(network::AgentCircleBehaviourEvent::Mdns(
                        mdns::Event::Discovered(list),
                    )) => {
                        for (peer_id, addr) in list {
                            if peer_id != my_peer {
                                println!("   📡 发现: {peer_id}");
                                swarm.behaviour_mut().kademlia.add_address(&peer_id, addr.clone());
                                swarm.behaviour_mut().gossip.add_explicit_peer(&peer_id);
                                let _ = swarm.dial(addr);
                            }
                        }
                    }
                    SwarmEvent::Behaviour(network::AgentCircleBehaviourEvent::Gossip(
                        gossipsub::Event::Subscribed { peer_id, .. },
                    )) => {
                        println!("   🔗 mesh 已建立: {peer_id}");
                    }
                    _ => {}
                }
            }
            _ = tokio::time::sleep_until(deadline) => { break; }
        }
    }

    let topics = network::list_group_topics(&swarm);
    println!("   当前订阅: {topics:?}");
    Ok(())
}

async fn cmd_group_send(name: &str, message: &str) -> errors::AcResult<()> {
    use futures::StreamExt;
    use libp2p::{gossipsub, mdns, swarm::SwarmEvent};

    let id = match storage::load_identity(data_dir_opt())? {
        Some(id) => id,
        None => {
            eprintln!("⚠️  未找到身份，请先创建。");
            std::process::exit(1);
        }
    };

    let mut swarm = network::build_swarm(&id)?;
    let my_did = id.did.clone();
    let my_peer = *swarm.local_peer_id();

    // Subscribe to the topic
    network::join_group(&mut swarm, name)?;

    println!("📡 发送群消息 → \"{name}\"...");

    // Pump: discover peers via mDNS, mesh via GossipSub
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(25);
    let mut meshed = false;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }

        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::Behaviour(network::AgentCircleBehaviourEvent::Mdns(
                        mdns::Event::Discovered(list),
                    )) => {
                        for (peer_id, addr) in list {
                            if peer_id != my_peer {
                                eprintln!("   📡 发现: {peer_id} @ {addr}");
                                swarm.behaviour_mut().kademlia.add_address(&peer_id, addr.clone());
                                swarm.behaviour_mut().gossip.add_explicit_peer(&peer_id);
                                match swarm.dial(addr) {
                                    Ok(()) => eprintln!("   ✅ 已拨号: {peer_id}"),
                                    Err(e) => eprintln!("   ❌ 拨号失败: {peer_id} ({e})"),
                                }
                            }
                        }
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        eprintln!("   🔗 连接已建立: {peer_id}");
                    }
                    SwarmEvent::Behaviour(network::AgentCircleBehaviourEvent::Gossip(
                        gossipsub::Event::Subscribed { peer_id, topic: _ },
                    )) => {
                        eprintln!("   🔗 mesh: {peer_id}");
                        meshed = true;
                    }
                    SwarmEvent::Behaviour(network::AgentCircleBehaviourEvent::Gossip(
                        gossipsub::Event::GossipsubNotSupported { peer_id },
                    )) => {
                        eprintln!("   ⚠️  {peer_id} 不支持 GossipSub");
                    }
                    _ => {}
                }
            }
            _ = tokio::time::sleep_until(deadline) => { break; }
        }
    }

    if !meshed {
        // Still try to publish — maybe it'll work
        eprintln!("   ⚠️  未检测到 GossipSub mesh，尝试发送...");
    }

    network::send_group_message(&mut swarm, name, &my_did, message)?;
    println!("👥 已发送 [{name}] {message}");

    // Let message propagate
    let dl = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        tokio::select! {
            _ = swarm.select_next_some() => {}
            _ = tokio::time::sleep_until(dl) => { break; }
        }
    }

    Ok(())
}

async fn cmd_group_list() -> errors::AcResult<()> {
    // Group list requires a running daemon — for now use a short-lived swarm
    let id = match storage::load_identity(data_dir_opt())? {
        Some(id) => id,
        None => {
            eprintln!("⚠️  未找到身份。");
            std::process::exit(1);
        }
    };

    let swarm = network::build_swarm(&id)?;
    let topics = network::list_group_topics(&swarm);
    if topics.is_empty() {
        println!("📭 未加入任何群组。");
        println!("   加入群组: agent-circle group join \"<NAME>\"");
    } else {
        println!("👥 已加入的群组:");
        for t in &topics {
            println!("   {t}");
        }
    }
    Ok(())
}

// ── Timeline commands ───────────────────────────────────────────────

fn cmd_timeline_post(message: &str) -> errors::AcResult<()> {
    let id = match storage::load_identity(data_dir_opt())? {
        Some(id) => id,
        None => {
            eprintln!("⚠️  未找到身份，请先创建。");
            std::process::exit(1);
        }
    };

    let mut tl = storage::load_timeline(data_dir_opt())?;
    let node = if tl.is_empty() {
        let node = timeline::Timeline::genesis(&id, message)?;
        tl.nodes.push(node.clone());
        node
    } else {
        tl.append(&id, message)?
    };
    storage::save_timeline(&tl, data_dir_opt())?;

    println!("📱 已发布朋友圈:");
    println!("   {:<12} {}", "ID:", node.id);
    println!("   {:<12} {}", "时间:", chrono::DateTime::from_timestamp(node.ts, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default());
    println!("   {:<12} {}", "内容:", node.content);
    if !node.parents.is_empty() {
        println!("   {:<12} {}", "上一条:", node.parents.join(", "));
    }
    println!("   {:<12} {} 帖", "总帖数:", tl.len());
    Ok(())
}

fn cmd_timeline_show() -> errors::AcResult<()> {
    let id = match storage::load_identity(data_dir_opt())? {
        Some(id) => id,
        None => {
            eprintln!("⚠️  未找到身份。");
            std::process::exit(1);
        }
    };

    let tl = storage::load_timeline(data_dir_opt())?;
    if tl.is_empty() {
        println!("📱 朋友圈为空。");
        println!("   发布首帖: agent-circle timeline post \"Hello, world!\"");
        return Ok(());
    }

    println!("📱 {} 的朋友圈 ({} 帖)", id.short_code, tl.len());
    println!("{:=<60}", "");
    for (i, node) in tl.nodes.iter().enumerate() {
        let dt = chrono::DateTime::from_timestamp(node.ts, 0)
            .map(|d| d.format("%m-%d %H:%M").to_string())
            .unwrap_or_default();
        println!("  [{i}] {dt}");
        println!("      {}", node.content);
        if i < tl.len() - 1 {
            println!("      │");
        }
    }
    println!("{:=<60}", "");
    println!("  验证: agent-circle timeline verify");
    Ok(())
}

fn cmd_timeline_verify() -> errors::AcResult<()> {
    let tl = storage::load_timeline(data_dir_opt())?;
    if tl.is_empty() {
        println!("📱 朋友圈为空，无需验证。");
        return Ok(());
    }

    match tl.verify() {
        Ok(()) => {
            println!("✅ 朋友圈时间线完整且未被篡改！");
            println!("   总帖数: {} 帖", tl.len());
            println!("   所有哈希链和 Ed25519 签名均已验证通过。");
        }
        Err(e) => {
            eprintln!("❌ 时间线验证失败: {e}");
        }
    }
    Ok(())
}
