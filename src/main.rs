//! Agent Circle — AI 智能体的微信
//!
//! A P2P social CLI for AI agents. Serverless. Key = identity. E2E by default.

mod chat;
mod dedup;
mod diag;
mod errors;
mod identity;
mod message_queue;
mod network;
mod reliability;
mod sequence;
mod storage;
mod timeline;

use clap::{Parser, Subcommand};
use futures::StreamExt;
use identity::Identity;
use std::path::PathBuf;
use std::sync::OnceLock;
use storage::{load_card, load_identity, save_card, save_identity};
use tracing::error;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{reload, EnvFilter, Registry};

/// Global data directory override, set from CLI --data-dir.
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Reload handle for dynamic log level switching (SIGUSR1).
static RELOAD_HANDLE: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();

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

    /// 诊断 — 消息投递统计 & 离线队列
    Diag {
        #[command(subcommand)]
        cmd: DiagCmd,
    },
}

#[derive(Subcommand)]
enum DiagCmd {
    /// 离线消息队列统计 (pending / delivered / failed)
    Queue,
    /// 清理过期消息和已送达记录
    Clean,
    /// 守护进程运行状态
    Status,
}

#[derive(Subcommand)]
enum ChatCmd {
    /// 发送消息给指定 PeerId
    Send {
        /// 目标 PeerId
        peer_id: String,
        /// 消息内容
        message: Vec<String>,
        /// 追踪投递状态 — 等待 ACK 或超时后打印结果
        #[arg(long)]
        track: bool,
        /// 追踪超时秒数 (默认 30s)
        #[arg(long, default_value = "30")]
        timeout: u64,
    },
    /// 压力测试 — 发送 N 条消息并统计投递率
    PressureTest {
        /// 目标 PeerId
        peer_id: String,
        /// 消息数量 (默认 100)
        #[arg(long, default_value = "100")]
        count: usize,
        /// 每条消息等待 ACK 的超时秒数 (默认 5s)
        #[arg(long, default_value = "5")]
        timeout: u64,
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
        /// 启用中继模式—作为 Relay 节点为 NAT 后节点提供兜底连接
        #[arg(long)]
        relay: bool,
    },
    /// 查看守护进程状态
    Status,
}

fn init_tracing(json: bool) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let (filter, reload_handle) = reload::Layer::new(env_filter);

    if json {
        Registry::default()
            .with(filter)
            .with(tracing_subscriber::fmt::layer().json().with_target(false))
            .init();
    } else {
        Registry::default()
            .with(filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    let _ = RELOAD_HANDLE.set(reload_handle);
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        error!("{e}");
        std::process::exit(1);
    }
}

async fn run() -> errors::AcResult<()> {
    let cli = Cli::parse();

    // Init tracing: JSON for daemon, human-readable for CLI
    let is_daemon = matches!(cli.command, Commands::Daemon { .. });
    init_tracing(is_daemon);

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
            DaemonCmd::Start { group, relay } => cmd_daemon_start(&group, relay).await?,
            DaemonCmd::Status => cmd_daemon_status()?,
        },
        Commands::Contact(cmd) => match cmd {
            ContactCmd::Add { peer_id, name, did } => cmd_contact_add(&name, &peer_id, &did)?,
            ContactCmd::List => cmd_contact_list()?,
        },
        Commands::Chat { cmd } => match cmd {
            ChatCmd::Send {
                peer_id,
                message,
                track,
                timeout,
            } => {
                if track {
                    cmd_chat_send_track(&peer_id, &message.join(" "), timeout).await?
                } else {
                    cmd_chat_send(&peer_id, &message.join(" ")).await?
                }
            }
            ChatCmd::PressureTest {
                peer_id,
                count,
                timeout,
            } => cmd_chat_pressure_test(&peer_id, count, timeout).await?,
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
        Commands::Diag { cmd } => match cmd {
            DiagCmd::Queue => cmd_diag_queue()?,
            DiagCmd::Clean => cmd_diag_clean()?,
            DiagCmd::Status => cmd_daemon_status()?,
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

async fn cmd_daemon_start(groups: &[String], relay_mode: bool) -> errors::AcResult<()> {
    let id = match load_identity(data_dir_opt())? {
        Some(id) => id,
        None => {
            eprintln!("⚠️  尚未创建身份。请先运行: agent-circle identity create --name <NAME> --owner <OWNER>");
            std::process::exit(1);
        }
    };

    // Spawn SIGUSR1 handler for dynamic log level switching
    // NOTE: WSL2 has signal delivery quirks; tested on native Linux/macOS
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        if let Some(handle) = RELOAD_HANDLE.get().cloned() {
            match signal(SignalKind::user_defined1()) {
                Ok(mut sig) => {
                    tokio::spawn(async move {
                        let levels: [&str; 5] = ["error", "warn", "info", "debug", "trace"];
                        let mut idx: usize = 2;
                        loop {
                            sig.recv().await;
                            idx = (idx + 1) % levels.len();
                            let new_level = levels[idx];
                            let _ = handle.reload(EnvFilter::new(new_level));
                            tracing::info!(%new_level, "📶 日志级别已切换");
                        }
                    });
                }
                Err(e) => tracing::warn!("SIGUSR1 注册失败: {e}"),
            }
        }
    }

    if relay_mode {
        tracing::info!("🔁 中继模式已启用 — 本节点将作为 Relay 为 NAT 后节点提供兜底连接");
    }

    let data_dir = storage::resolve_data_dir(data_dir_opt())?;
    network::run_daemon(&id, groups, relay_mode, &data_dir).await
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

    swarm
        .dial(peer_id)
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

async fn cmd_chat_send_track(
    peer_id_str: &str,
    message: &str,
    timeout_secs: u64,
) -> errors::AcResult<()> {
    use libp2p::request_response::{self, Message};
    use libp2p::swarm::SwarmEvent;
    use libp2p::PeerId;
    use std::str::FromStr;
    use std::time::Instant;

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

    // ── Connect ──────────────────────────────────────────────────
    println!("📡 正在发现并连接 {peer_id}...");
    for _ in 0..5 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        tokio::select! {
            _ = swarm.select_next_some() => {}
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
        }
        if swarm.is_connected(&peer_id) {
            break;
        }
    }
    swarm
        .dial(peer_id)
        .map_err(|e| errors::AcError::Network(format!("dial 失败: {e}")))?;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // ── Send & track ─────────────────────────────────────────────
    let request_id = network::send_chat(&mut swarm, peer_id, &my_did, message);
    let sent_at = Instant::now();
    println!("💬 已发送 → {peer_id}: {message}");
    println!("⏳ 等待送达确认 (超时: {timeout_secs}s)...");

    let deadline = Instant::now() + std::time::Duration::from_secs(timeout_secs);

    loop {
        let now = Instant::now();
        if now >= deadline {
            println!("⏰ 待确认 ({}s 内未收到 ACK)", timeout_secs);
            return Ok(());
        }

        let remaining = deadline - now;
        tokio::select! {
            event = swarm.select_next_some() => {
                match event {
                    // ACK received → delivered!
                    SwarmEvent::Behaviour(network::AgentCircleBehaviourEvent::Chat(
                        request_response::Event::Message {
                            message: Message::Response { request_id: rid, .. },
                            ..
                        }
                    )) if rid == request_id => {
                        let elapsed = sent_at.elapsed();
                        println!("✅ 已送达 → {peer_id} ({:.0}ms)", elapsed.as_secs_f64() * 1000.0);
                        return Ok(());
                    }
                    // Outbound failure → report
                    SwarmEvent::Behaviour(network::AgentCircleBehaviourEvent::Chat(
                        request_response::Event::OutboundFailure {
                            request_id: rid,
                            error,
                            ..
                        }
                    )) if rid == request_id => {
                        println!("❌ 发送失败 → {peer_id}: {error:?}");
                        return Ok(());
                    }
                    _ => {}
                }
            }
            _ = tokio::time::sleep(remaining) => {
                println!("⏰ 待确认 ({}s 内未收到 ACK)", timeout_secs);
                return Ok(());
            }
        }
    }
}

async fn cmd_chat_pressure_test(
    peer_id_str: &str,
    count: usize,
    timeout_secs: u64,
) -> errors::AcResult<()> {
    use libp2p::request_response::{self, Message};
    use libp2p::swarm::SwarmEvent;
    use libp2p::PeerId;
    use std::str::FromStr;
    use std::time::Instant;

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

    // ── Connect ──────────────────────────────────────────────────
    println!("📡 正在连接 {peer_id}...");
    for _ in 0..8 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        tokio::select! {
            _ = swarm.select_next_some() => {}
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
        }
        if swarm.is_connected(&peer_id) {
            break;
        }
    }
    swarm
        .dial(peer_id)
        .map_err(|e| errors::AcError::Network(format!("dial 失败: {e}")))?;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    println!("🔫 压力测试 — {count} 条消息, 每条约 {timeout_secs}s 超时");

    let mut delivered: usize = 0;
    let mut failed: usize = 0;
    let mut timeout_expired: usize = 0;
    let total_start = Instant::now();

    for i in 1..=count {
        // Send
        let msg =
            format!("pressure-test #{i}: 这是一条压力测试消息，用于验证 P2P 消息投递可靠性。");
        let request_id = network::send_chat(&mut swarm, peer_id, &my_did, &msg);
        let sent_at = Instant::now();
        let deadline = Instant::now() + std::time::Duration::from_secs(timeout_secs);

        // Wait for ACK or failure
        let mut acked = false;
        loop {
            let now = Instant::now();
            if now >= deadline {
                timeout_expired += 1;
                print!("⏰");
                break;
            }
            let remaining = deadline - now;
            tokio::select! {
                event = swarm.select_next_some() => {
                    match event {
                        SwarmEvent::Behaviour(network::AgentCircleBehaviourEvent::Chat(
                            request_response::Event::Message {
                                message: Message::Response { request_id: rid, .. },
                                ..
                            }
                        )) if rid == request_id => {
                            delivered += 1;
                            acked = true;
                            let ms = sent_at.elapsed().as_millis();
                            if count <= 20 || i % 10 == 0 {
                                print!(" #{i}:{ms}ms ✓");
                            }
                            break;
                        }
                        SwarmEvent::Behaviour(network::AgentCircleBehaviourEvent::Chat(
                            request_response::Event::OutboundFailure {
                                request_id: rid,
                                ..
                            }
                        )) if rid == request_id => {
                            failed += 1;
                            acked = true;
                            print!("✗");
                            break;
                        }
                        _ => {}
                    }
                }
                _ = tokio::time::sleep(remaining) => {
                    timeout_expired += 1;
                    print!("⏰");
                    acked = true;
                    break;
                }
            }
        }
        if !acked {
            timeout_expired += 1;
        }

        // Progress dot for large counts
        if count > 20 && i % 10 == 0 {
            println!();
        }
    }
    println!();

    let elapsed = total_start.elapsed();
    let total = delivered + failed + timeout_expired;
    let rate = if total > 0 {
        (delivered as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    println!();
    println!("══════════════════════════════════════");
    println!("  压力测试结果");
    println!("══════════════════════════════════════");
    println!("  消息总数:    {count}");
    println!("  ✅ 已送达:    {delivered} ({:.1}%)", rate);
    println!("  ❌ 失败:      {failed}");
    println!("  ⏰ 待确认:    {timeout_expired}");
    println!("  总耗时:       {:.1}s", elapsed.as_secs_f64());
    println!(
        "  吞吐量:       {:.1} msg/s",
        total as f64 / elapsed.as_secs_f64().max(0.001)
    );
    println!("══════════════════════════════════════");

    if rate >= 99.9 {
        println!("🎯 99.9% 投递率达成！");
    } else if rate >= 99.0 {
        println!("⚠️  未达 99.9%，可尝试增加 --count 或调整网络");
    } else {
        println!("❌ 投递率较低，检查网络和对方 daemon 是否运行");
    }

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
    println!(
        "   {:<12} {}",
        "时间:",
        chrono::DateTime::from_timestamp(node.ts, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default()
    );
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

// ── Diag command ────────────────────────────────────────────────────

fn cmd_diag_queue() -> errors::AcResult<()> {
    let data_dir = storage::resolve_data_dir(data_dir_opt())?;
    match message_queue::Queue::open(&data_dir) {
        Ok(q) => match q.stats() {
            Ok((pending, delivered, failed)) => {
                println!(
                    "📬 离线消息队列\n  待发送: {}  已送达: {}  失败(>3次重试): {}",
                    pending, delivered, failed
                );
                let total = pending + delivered + failed;
                if total > 0 {
                    let rate = (delivered as f64 / total as f64) * 100.0;
                    println!("  总计: {}  送达率: {:.1}%", total, rate);
                }
            }
            Err(e) => eprintln!("❌ 读取队列统计失败: {e}"),
        },
        Err(e) => eprintln!("❌ 打开离线队列失败: {e}"),
    }
    Ok(())
}

fn cmd_diag_clean() -> errors::AcResult<()> {
    let data_dir = storage::resolve_data_dir(data_dir_opt())?;
    let q = match message_queue::Queue::open(&data_dir) {
        Ok(q) => q,
        Err(e) => {
            eprintln!("❌ 打开离线队列失败: {e}");
            return Ok(());
        }
    };
    let now = chrono::Utc::now().timestamp();
    match q.expire_before(now) {
        Ok(n) => println!("🧹 已清理 {} 条过期离线消息", n),
        Err(e) => eprintln!("❌ 过期清理失败: {e}"),
    }
    match q.prune_delivered() {
        Ok(n) => println!("🧹 已清理 {} 条已送达记录", n),
        Err(e) => eprintln!("❌ 已送达清理失败: {e}"),
    }
    Ok(())
}
