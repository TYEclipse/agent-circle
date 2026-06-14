//! Agent Circle — AI 智能体的微信
//!
//! A P2P social CLI for AI agents. Serverless. Key = identity. E2E by default.

mod chat;
mod control;
mod dedup;
mod diag;
mod errors;
mod identity;
mod keys;
mod message_queue;
mod metrics;
mod network;
mod plugin;
mod plugin_registry;
mod protocol;
mod reliability;
mod sequence;
mod service_discovery;
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
    /// 数据目录（默认 $AGENT_CIRCLE_HOME 或 ~/.agent-circle/）
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

    /// 服务发现 — 搜索和列出网络服务
    #[command(subcommand)]
    Service(ServiceCmd),

    /// 插件 — Agent Plugin 管理 (S09)
    #[command(subcommand)]
    Plugin(PluginCmd),

    /// 诊断 — 消息投递统计 & 离线队列
    Diag {
        #[command(subcommand)]
        cmd: DiagCmd,
    },

    /// 全链路诊断 — 一键检查身份/网络/存储/联系人 (S11R111)
    Doctor {
        /// 仅检查指定子系统 (identity|network|storage|contacts)
        #[arg(short, long)]
        check: Option<String>,
        /// JSON 输出
        #[arg(short, long)]
        json: bool,
    },

    /// 性能指标暴露 — OpenMetrics/Prometheus 格式 (S11R116)
    Metrics,
}

#[derive(Subcommand)]
enum ServiceCmd {
    /// 列出所有已发现的网络服务（彩色表格）
    List {
        /// 显示详细信息（描述 + 最后在线时间）
        #[arg(short, long)]
        verbose: bool,
    },
    /// 按名称或标签搜索服务
    Search {
        /// 搜索关键词
        query: String,
    },
    /// 协商服务能力（查询协议版本 + 参数格式）
    Negotiate {
        /// 目标 PeerId
        peer_id: String,
        /// 服务标识符 (如 "weather-v1")
        service_id: String,
    },
    /// 调用远程服务
    Call {
        /// 目标 PeerId
        peer_id: String,
        /// 服务标识符 (如 "weather-v1")
        service_id: String,
        /// 方法名 (如 "forecast")
        #[arg(default_value = "default")]
        method: String,
        /// JSON 参数
        #[arg(default_value = "{}")]
        params: String,
        /// 跳过能力协商，直接调用
        #[arg(long)]
        skip_negotiate: bool,
    },
    /// 订阅服务 — 关注特定服务的更新通知
    Subscribe {
        /// 服务标识符 (如 "weather-v1" 或 "weather-v1@<peer>")
        service_spec: String,
        /// 订阅标签
        #[arg(short, long, default_value = "")]
        label: String,
    },
    /// 取消订阅服务
    Unsubscribe {
        /// 服务标识符 (如 "weather-v1" 或 "weather-v1@<peer>")
        service_spec: String,
    },
    /// 列出所有已订阅的服务
    Subscriptions,
    /// 服务离线缓存 — 查看本地缓存状态/手动刷新
    Cache {
        /// 查看缓存摘要
        #[arg(short, long)]
        stats: bool,
        /// 强制刷新缓存 (清空后重新等待公告)
        #[arg(short, long)]
        flush: bool,
    },
    /// 发布服务到本地缓存 + 市场公告 (S10R109)
    Publish {
        /// 服务标识符 (如 "weather-v1")
        service_id: String,
        /// 服务名称
        #[arg(short, long)]
        name: String,
        /// 服务端点 (如 "/ac/weather/1.0.0")
        #[arg(short, long)]
        endpoint: String,
        /// 服务描述
        #[arg(short, long)]
        description: Option<String>,
        /// 标签 (逗号分隔)
        #[arg(short, long, value_delimiter = ',')]
        tags: Vec<String>,
    },
}

#[derive(Subcommand)]
enum PluginCmd {
    /// 列出已加载的插件
    List,
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
        /// 丢包率模拟 (0.0–1.0), 发送端随机丢弃
        #[arg(long, default_value = "0.0")]
        drop_rate: f64,
        /// 结果报告输出路径 (JSON)
        #[arg(long)]
        output: Option<String>,
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
    /// 从 BIP-39 助记词恢复身份
    Restore {
        /// 12 个助记词（用引号包裹，空格分隔）
        mnemonic: String,
        /// 可选的密码短语 (BIP-39 passphrase)
        #[arg(short, long, default_value = "")]
        passphrase: String,
    },
    /// 生成新的 BIP-39 助记词（用于备份）
    Mnemonic,
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
    /// 动态切换日志级别 (Linux/macOS/Windows 通用)
    LogLevel {
        /// 目标级别: error | warn | info | debug | trace
        level: String,
    },
    /// 安装为系统服务 (systemd / launchd / WinSW)
    Install,
    /// 卸载系统服务
    Uninstall,
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
            IdentityCmd::Restore {
                mnemonic,
                passphrase,
            } => cmd_identity_restore(&mnemonic, &passphrase)?,
            IdentityCmd::Mnemonic => cmd_identity_mnemonic()?,
        },
        Commands::Daemon { cmd } => match cmd {
            DaemonCmd::Start { group, relay } => cmd_daemon_start(&group, relay).await?,
            DaemonCmd::Status => cmd_daemon_status()?,
            DaemonCmd::LogLevel { level } => cmd_daemon_log_level(&level).await?,
            DaemonCmd::Install => cmd_daemon_install()?,
            DaemonCmd::Uninstall => cmd_daemon_uninstall()?,
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
                drop_rate,
                output,
            } => {
                cmd_chat_pressure_test(&peer_id, count, timeout, drop_rate, output.as_deref())
                    .await?
            }
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
        Commands::Service(cmd) => match cmd {
            ServiceCmd::List { verbose } => cmd_service_list(verbose)?,
            ServiceCmd::Search { query } => cmd_service_search(&query)?,
            ServiceCmd::Negotiate {
                peer_id,
                service_id,
            } => cmd_service_negotiate(&peer_id, &service_id)?,
            ServiceCmd::Call {
                peer_id,
                service_id,
                method,
                params,
                skip_negotiate: _skip,
            } => cmd_service_call(&peer_id, &service_id, &method, &params)?,
            ServiceCmd::Subscribe {
                service_spec,
                label,
            } => cmd_service_subscribe(&service_spec, &label)?,
            ServiceCmd::Unsubscribe { service_spec } => cmd_service_unsubscribe(&service_spec)?,
            ServiceCmd::Subscriptions => cmd_service_subscriptions()?,
            ServiceCmd::Cache { stats, flush } => cmd_service_cache(stats, flush)?,
            ServiceCmd::Publish {
                service_id,
                name,
                endpoint,
                description,
                tags,
            } => cmd_service_publish(&service_id, &name, &endpoint, description.as_deref(), &tags)?,
        },
        Commands::Plugin(cmd) => match cmd {
            PluginCmd::List => cmd_plugin_list()?,
        },
        Commands::Diag { cmd } => match cmd {
            DiagCmd::Queue => cmd_diag_queue()?,
            DiagCmd::Clean => cmd_diag_clean()?,
            DiagCmd::Status => cmd_daemon_status()?,
        },
        Commands::Doctor { check, json } => cmd_doctor(check.as_deref(), json)?,
        Commands::Metrics => cmd_metrics()?,
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
    let card = id.create_card(name, owner, model, capabilities, vec![])?;

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

    // S07R75 — Cross-platform control socket (replaces Unix-only SIGUSR1)
    // Spawn a local TCP listener so `agent-circle daemon log-level <LEVEL>`
    // works on Linux, macOS, and Windows alike.
    if let Some(handle) = RELOAD_HANDLE.get().cloned() {
        let data_dir = storage::resolve_data_dir(data_dir_opt())?;
        match control::spawn_control_server(handle).await {
            Ok(addr) => {
                let port_path = data_dir.join("control.port");
                if let Err(e) = std::fs::write(&port_path, addr.port().to_string()) {
                    tracing::warn!(%e, "无法写入 control.port");
                } else {
                    tracing::info!(port = %addr.port(), "📡 控制端口已启动");
                }
            }
            Err(e) => tracing::warn!(%e, "控制端口启动失败 (log-level 命令不可用)"),
        }
    }

    // SIGUSR1 → cycle log level (Unix only, bonus fallback)
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
                            tracing::info!(%new_level, "📶 日志级别已切换 (SIGUSR1)");
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
    let port_file = dir.join("control.port");
    if sock.exists() || port_file.exists() {
        let running = if sock.exists() {
            format!("socket: {}", sock.display())
        } else {
            format!(
                "control port: {}",
                std::fs::read_to_string(&port_file)
                    .unwrap_or_default()
                    .trim()
            )
        };
        println!("✅ 守护进程正在运行 ({running})");
        println!("   动态日志: agent-circle daemon log-level <LEVEL>");
    } else {
        println!("⏸️  守护进程未运行。启动: agent-circle daemon start");
    }
    Ok(())
}

/// S07R75 — Connect to daemon control socket and switch log level.
async fn cmd_daemon_log_level(level: &str) -> errors::AcResult<()> {
    let levels: [&str; 5] = ["error", "warn", "info", "debug", "trace"];
    if !levels.contains(&level) {
        return Err(errors::AcError::Network(format!(
            "无效日志级别 '{level}'。可选: {}",
            levels.join(", ")
        )));
    }

    let dir = storage::resolve_data_dir(data_dir_opt())?;
    let port_path = dir.join("control.port");
    let port_str = std::fs::read_to_string(&port_path).map_err(|e| {
        errors::AcError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "守护进程未运行或 control.port 不可读: {e}\n请先启动: agent-circle daemon start"
            ),
        ))
    })?;
    let port: u16 = port_str
        .trim()
        .parse()
        .map_err(|e| errors::AcError::Network(format!("无效端口号 '{port_str}': {e}")))?;

    let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}")).await?;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let (reader, mut writer) = stream.split();
    let mut buf_reader = BufReader::new(reader);

    writer
        .write_all(format!("log-level {level}\n").as_bytes())
        .await?;

    let mut response = String::new();
    buf_reader.read_line(&mut response).await?;
    println!("📶 日志级别: {response}");
    Ok(())
}

/// S07R76-R78 — Install agent-circle as a system service.
///
/// Platform mapping:
///   Linux   → systemd user unit  (~/.config/systemd/user/agent-circle.service)
///   macOS   → launchd plist      (~/Library/LaunchAgents/com.agent-circle.daemon.plist)
///   Windows → WinSW XML          (next to agent-circle.exe)
fn cmd_daemon_install() -> errors::AcResult<()> {
    let bin_path = std::env::current_exe().map_err(|e| {
        errors::AcError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("无法获取可执行文件路径: {e}"),
        ))
    })?;

    #[cfg(target_os = "linux")]
    {
        let home = dirs::home_dir().ok_or_else(|| {
            errors::AcError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "无法确定 home 目录",
            ))
        })?;
        let unit_dir = home.join(".config/systemd/user");
        std::fs::create_dir_all(&unit_dir)?;
        let unit_path = unit_dir.join("agent-circle.service");

        let unit = format!(
            "[Unit]\nDescription=Agent Circle P2P Daemon\nAfter=network.target\n\n\
             [Service]\nType=simple\nExecStart={} daemon start\nRestart=on-failure\n\
             RestartSec=5\nEnvironment=RUST_LOG=info\n\n\
             [Install]\nWantedBy=default.target\n",
            bin_path.display()
        );

        std::fs::write(&unit_path, &unit)?;
        println!("✅ systemd user unit → {}", unit_path.display());
        println!();
        println!("启用服务:");
        println!("  systemctl --user enable --now agent-circle");
        println!("  systemctl --user status agent-circle");
    }

    #[cfg(target_os = "macos")]
    {
        let launch_dir = dirs::home_dir()
            .ok_or_else(|| {
                errors::AcError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "无法确定 home 目录",
                ))
            })?
            .join("Library/LaunchAgents");
        std::fs::create_dir_all(&launch_dir)?;
        let plist_path = launch_dir.join("com.agent-circle.daemon.plist");

        let plist = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\"\n  \
             \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n<dict>\n  \
             <key>Label</key>\n  <string>com.agent-circle.daemon</string>\n  \
             <key>ProgramArguments</key>\n  <array>\n    \
             <string>{}</string>\n    \
             <string>daemon</string>\n    \
             <string>start</string>\n  </array>\n  \
             <key>RunAtLoad</key>\n  <true/>\n  \
             <key>KeepAlive</key>\n  <true/>\n  \
             <key>StandardOutPath</key>\n  <string>/tmp/agent-circle.out</string>\n  \
             <key>StandardErrorPath</key>\n  <string>/tmp/agent-circle.err</string>\n\
             </dict>\n</plist>\n",
            bin_path.display()
        );

        std::fs::write(&plist_path, &plist)?;
        println!("✅ launchd plist → {}", plist_path.display());
        println!();
        println!("加载服务:");
        println!("  launchctl load {}", plist_path.display());
    }

    #[cfg(windows)]
    {
        let svc_dir = bin_path.parent().ok_or_else(|| {
            errors::AcError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "无法确定可执行文件所在目录",
            ))
        })?;
        let xml_path = svc_dir.join("agent-circle-service.xml");

        let xml = format!(
            "<service>\n  \
             <id>agent-circle</id>\n  \
             <name>Agent Circle P2P Daemon</name>\n  \
             <description>AI 智能体的 P2P 社交网络守护进程</description>\n  \
             <executable>{}</executable>\n  \
             <arguments>daemon start</arguments>\n  \
             <log mode=\"roll-by-size\">\n    \
             <sizeThreshold>10485760</sizeThreshold>\n    \
             <keepFiles>5</keepFiles>\n  </log>\n  \
             <onfailure action=\"restart\" delay=\"5 sec\"/>\n  \
             <env name=\"RUST_LOG\" value=\"info\"/>\n\
             </service>\n",
            bin_path.display()
        );

        std::fs::write(&xml_path, &xml)?;
        println!("✅ WinSW 配置 → {}", xml_path.display());
        println!();
        println!("将 WinSW.exe 重命名为 agent-circle-service.exe 放到同目录，然后:");
        println!("  agent-circle-service.exe install");
        println!("  agent-circle-service.exe start");
    }

    Ok(())
}

/// Uninstall the system service (remove the config file).
fn cmd_daemon_uninstall() -> errors::AcResult<()> {
    #[cfg(target_os = "linux")]
    {
        let home = dirs::home_dir().ok_or_else(|| {
            errors::AcError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "无法确定 home 目录",
            ))
        })?;
        let unit_path = home.join(".config/systemd/user/agent-circle.service");
        if unit_path.exists() {
            // Try to stop + disable first (best-effort)
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "stop", "agent-circle"])
                .output();
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "disable", "agent-circle"])
                .output();
            std::fs::remove_file(&unit_path)?;
            println!("✅ 已移除: {}", unit_path.display());
        } else {
            println!("ℹ️  未找到已安装的服务");
        }
    }

    #[cfg(target_os = "macos")]
    {
        let plist_path = dirs::home_dir()
            .ok_or_else(|| {
                errors::AcError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "无法确定 home 目录",
                ))
            })?
            .join("Library/LaunchAgents/com.agent-circle.daemon.plist");
        if plist_path.exists() {
            let _ = std::process::Command::new("launchctl")
                .args(["unload", plist_path.to_str().unwrap_or("")])
                .output();
            std::fs::remove_file(&plist_path)?;
            println!("✅ 已移除: {}", plist_path.display());
        } else {
            println!("ℹ️  未找到已安装的服务");
        }
    }

    #[cfg(windows)]
    {
        let svc_dir = std::env::current_exe()
            .map_err(|e| {
                errors::AcError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("无法获取 exe 路径: {e}"),
                ))
            })?
            .parent()
            .map(|p| p.to_path_buf())
            .ok_or_else(|| {
                errors::AcError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "无法确定 exe 目录",
                ))
            })?;
        let xml_path = svc_dir.join("agent-circle-service.xml");
        if xml_path.exists() {
            // Try to stop + uninstall first (best-effort)
            let _ = std::process::Command::new(svc_dir.join("agent-circle-service.exe"))
                .arg("stop")
                .output();
            let _ = std::process::Command::new(svc_dir.join("agent-circle-service.exe"))
                .arg("uninstall")
                .output();
            std::fs::remove_file(&xml_path)?;
            println!("✅ 已移除: {}", xml_path.display());
        } else {
            println!("ℹ️  未找到已安装的服务");
        }
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
    drop_rate: f64,
    output_path: Option<&str>,
) -> errors::AcResult<()> {
    use libp2p::request_response::{self, Message};
    use libp2p::swarm::SwarmEvent;
    use libp2p::PeerId;
    use rand::Rng;
    use std::str::FromStr;
    use std::time::Instant;

    let drop_rate = drop_rate.clamp(0.0, 1.0);

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

    if drop_rate > 0.0 {
        println!(
            "🔫 压力测试 — {count} 条消息, 超时 {timeout_secs}s, 丢包率 {:.0}%",
            drop_rate * 100.0
        );
    } else {
        println!("🔫 压力测试 — {count} 条消息, 超时 {timeout_secs}s");
    }

    let mut delivered: usize = 0;
    let mut failed: usize = 0;
    let mut timeout_expired: usize = 0;
    let mut dropped: usize = 0;
    let mut latencies_ms: Vec<u64> = Vec::with_capacity(count);
    let total_start = Instant::now();
    let mut rng = rand::thread_rng();

    for i in 1..=count {
        // Drop? (simulated packet loss)
        if drop_rate > 0.0 && rng.r#gen::<f64>() < drop_rate {
            dropped += 1;
            if count <= 20 || i % 10 == 0 {
                print!("✂");
            }
            if count > 20 && i % 50 == 0 {
                print!("[{dropped}]");
            }
            continue;
        }

        let msg = format!("bench-{i:05}: P2P 消息投递可靠性验证 (S02R26)");
        let request_id = network::send_chat(&mut swarm, peer_id, &my_did, &msg);
        let sent_at = Instant::now();
        let deadline = Instant::now() + std::time::Duration::from_secs(timeout_secs);

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
                            let ms = sent_at.elapsed().as_millis() as u64;
                            latencies_ms.push(ms);
                            delivered += 1;
                            acked = true;
                            if count <= 20 || i % 10 == 0 {
                                print!(" #{i}:{ms}ms");
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

        if count > 20 && i % 10 == 0 && i % 50 == 0 {
            let pct = (i as f64 / count as f64) * 100.0;
            println!(" {pct:.0}%");
        }
    }
    println!();

    // ── Stats ────────────────────────────────────────────────────
    let elapsed = total_start.elapsed();
    let sent = count - dropped;
    let total = delivered + failed + timeout_expired;
    let rate = if sent > 0 {
        (delivered as f64 / sent as f64) * 100.0
    } else {
        0.0
    };

    let (lat_min, lat_max, lat_avg, lat_p50, lat_p95, lat_p99) = if !latencies_ms.is_empty() {
        let mut sorted = latencies_ms.clone();
        sorted.sort_unstable();
        let min = sorted[0];
        let max = sorted[sorted.len() - 1];
        let avg = sorted.iter().sum::<u64>() as f64 / sorted.len() as f64;
        let p50 = sorted[(sorted.len() as f64 * 0.50) as usize];
        let p95 = sorted[(sorted.len() as f64 * 0.95).min((sorted.len() - 1) as f64) as usize];
        let p99 = sorted[(sorted.len() as f64 * 0.99).min((sorted.len() - 1) as f64) as usize];
        (min, max, avg, p50, p95, p99)
    } else {
        (0, 0, 0.0, 0, 0, 0)
    };

    println!();
    println!("══════════════════════════════════════════");
    println!("  S02R26/R27 投递可靠性验证报告");
    println!("══════════════════════════════════════════");
    println!(
        "  测试时间:    {}",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
    );
    println!("  目标 Peer:   {peer_id}");
    println!("  计划发送:    {count}");
    if dropped > 0 {
        println!(
            "  ✂ 模拟丢包:  {dropped} ({:.1}%)",
            (dropped as f64 / count as f64) * 100.0
        );
    }
    println!("  实际发送:    {sent}");
    println!("  ✅ 已送达:    {delivered} ({:.1}%)", rate);
    println!("  ❌ 失败:      {failed}");
    println!("  ⏰ 待确认:    {timeout_expired}");
    println!("──────────────────────────────────────────");
    println!("  延迟 (ms):   min={lat_min}  avg={lat_avg:.0}  max={lat_max}");
    println!("               p50={lat_p50}  p95={lat_p95}  p99={lat_p99}");
    println!("──────────────────────────────────────────");
    println!("  总耗时:       {:.1}s", elapsed.as_secs_f64());
    println!(
        "  吞吐量:       {:.1} msg/s",
        total as f64 / elapsed.as_secs_f64().max(0.001)
    );
    println!("══════════════════════════════════════════");

    // Grade
    if drop_rate <= 0.0 && rate >= 99.9 {
        println!("🎯 R26 PASS — 稳定网络 99.9% 投递率达成！");
    } else if drop_rate > 0.0 && rate >= 99.0 {
        println!(
            "✅ R27 PASS — {:.0}% 丢包环境 ≥99.0% 投递率达成！",
            drop_rate * 100.0
        );
    } else if rate >= 99.0 {
        println!("⚠️  未达 99.9%，可增加 --count 或调整网络");
    } else {
        println!("❌ 投递率较低，检查网络和对方 daemon 是否运行");
    }

    // ── Write report ─────────────────────────────────────────────
    let report = serde_json::json!({
        "test": "S02R26/R27",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "target_peer": peer_id_str,
        "parameters": {
            "count": count,
            "timeout_secs": timeout_secs,
            "drop_rate": drop_rate,
        },
        "results": {
            "sent": sent,
            "delivered": delivered,
            "failed": failed,
            "timeout": timeout_expired,
            "dropped_simulated": dropped,
            "delivery_rate_pct": rate,
            "latency_ms": {
                "min": lat_min,
                "max": lat_max,
                "avg": lat_avg,
                "p50": lat_p50,
                "p95": lat_p95,
                "p99": lat_p99,
            },
            "elapsed_secs": elapsed.as_secs_f64(),
            "throughput_msg_per_sec": total as f64 / elapsed.as_secs_f64().max(0.001),
        },
        "verdict": if drop_rate <= 0.0 && rate >= 99.9 {
            "PASS_R26"
        } else if drop_rate > 0.0 && rate >= 99.0 {
            "PASS_R27"
        } else if rate >= 99.0 {
            "WARN_BELOW_99_9"
        } else {
            "FAIL"
        }
    });

    let output_path = output_path.unwrap_or_else(|| {
        let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        &*Box::leak(format!("S02R26-report-{ts}.json").into_boxed_str())
    });
    match std::fs::write(output_path, serde_json::to_string_pretty(&report)?) {
        Ok(()) => println!("\n📄 报告已保存: {output_path}"),
        Err(e) => eprintln!("\n⚠️  报告保存失败: {e}"),
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

// ── Service discovery commands ────────────────────────────────────────
// S10R103

// S10R105 — 彩色表格展示层
fn cmd_service_list(verbose: bool) -> errors::AcResult<()> {
    let data_dir = storage::resolve_data_dir(data_dir_opt())?;
    let registry = service_discovery::load_registry(&data_dir)?;
    let services = registry.all_services_with_meta();

    if services.is_empty() {
        println!("🔍 暂无已发现的服务 (等待 daemon 发现或手动公告)");
        return Ok(());
    }

    // ── ANSI color constants ──────────────────────────────────────
    let (bold, reset) = ("\x1b[1m", "\x1b[0m");
    let (cyan, green, yellow, dim, magenta) =
        ("\x1b[36m", "\x1b[32m", "\x1b[33m", "\x1b[2m", "\x1b[35m");

    // Column widths (dynamic, using char count for multibyte safety)
    let max_svc_id = services
        .iter()
        .map(|(_, s, _)| s.id.chars().count())
        .max()
        .unwrap_or(10)
        .max(10);
    let max_name = services
        .iter()
        .map(|(_, s, _)| s.name.chars().count())
        .max()
        .unwrap_or(4)
        .max(4);
    let max_ep = services
        .iter()
        .map(|(_, s, _)| s.endpoint.chars().count())
        .max()
        .unwrap_or(8)
        .max(8);

    let w_peer = 14; // 12D3KooW… + 2 pad
    let w_svc = max_svc_id + 4; // generous padding for truncation margin
    let w_name = max_name + 4;
    let w_ep = max_ep + 4;
    let tag_w = 24;
    let desc_w = if verbose { 36 } else { 0 };
    let seen_w = if verbose { 18 } else { 0 };

    // Total table width
    let total_w = 1
        + w_peer
        + 1
        + w_svc
        + 1
        + w_name
        + 1
        + w_ep
        + 1
        + tag_w
        + 1
        + if verbose { desc_w + 1 + seen_w + 1 } else { 0 };

    // ── Top border + header ──────────────────────────────────────
    let bar = "━".repeat(total_w);

    println!("{}╔{}╗{}", dim, bar, reset);
    let header_line = format!(
        "{bold}🔍  Service Discovery — {} 服务 / {} 节点{reset}",
        services.len(),
        registry.peer_count()
    );
    let header_pad = total_w.saturating_sub(header_line.chars().count() + 2);
    println!(
        "{}║{} {}{}{} ║{}",
        dim,
        reset,
        header_line,
        " ".repeat(header_pad),
        dim,
        reset
    );

    // Column headers
    println!("{}╟{}╢{}", dim, "─".repeat(total_w - 2), reset);
    let col_hdr = format!(
        "{bold}{:w_peer$}{reset}│{bold}{:w_svc$}{reset}│{bold}{:w_name$}{reset}│{bold}{:w_ep$}{reset}│{bold}{:tag_w$}{reset}",
        "Peer", "Service", "Name", "Endpoint", "Tags"
    );
    let col_hdr = if verbose {
        format!(
            "{}│{bold}{:desc_w$}{reset}│{bold}{:seen_w$}{reset}",
            col_hdr, "Description", "Last Seen"
        )
    } else {
        col_hdr
    };
    println!("{}║{} {}║{}", dim, reset, col_hdr, reset);
    println!("{}╟{}╢{}", dim, "─".repeat(total_w - 2), reset);

    // ── Data rows ────────────────────────────────────────────────
    let now = chrono::Utc::now().timestamp();
    // small utility: truncate &str to max chars (char boundary safe)
    fn trunc_str(s: &str, max_chars: usize) -> &str {
        let mut char_count = 0;
        for (i, _c) in s.char_indices() {
            char_count += 1;
            if char_count > max_chars {
                return &s[..i];
            }
        }
        s
    }
    for (peer, svc, last_seen) in &services {
        let short_peer = trunc_str(peer, 12);
        let tags_str = if svc.tags.is_empty() {
            format!("{:tag_w$}", "—")
        } else {
            let joined = svc
                .tags
                .iter()
                .map(|t| format!("{dim}[{reset}{magenta}{t}{reset}{dim}]{reset}"))
                .collect::<Vec<_>>()
                .join(" ");
            format!(
                "{joined}{:w$}",
                "",
                w = tag_w.saturating_sub(svc.tags.join(" ").len() + svc.tags.len() * 3)
            )
        };
        let name_trunc = trunc_str(&svc.name, w_name - 3);
        let svc_id_trunc = trunc_str(&svc.id, w_svc - 3);

        let row_base = format!(
            "{cyan}{short_peer:w_peer$}{reset}│{green}{svc_id_trunc:w_svc$}{reset}│{yellow}{name_trunc:w_name$}{reset}│{dim}{:w_ep$}{reset}│{tags_str}",
            svc.endpoint
        );

        let row = if verbose {
            let desc = svc.description.as_deref().unwrap_or("—");
            let desc_trunc = trunc_str(desc, desc_w - 3);
            let age = now - *last_seen;
            let seen = if *last_seen == 0 {
                "从未".to_string()
            } else if age < 60 {
                format!("{}s 前", age)
            } else if age < 3600 {
                format!("{}m 前", age / 60)
            } else if age < 86400 {
                format!("{}h 前", age / 3600)
            } else {
                format!("{}d 前", age / 86400)
            };
            format!("{row_base}│{dim}{desc_trunc:desc_w$}{reset}│{dim}⏱ {seen:seen_w$}{reset}")
        } else {
            row_base
        };

        println!("{}║{} {}║{}", dim, reset, row, reset);
    }

    // ── Bottom border ────────────────────────────────────────────
    println!("{}╚{}╝{}", dim, bar, reset);
    Ok(())
}

fn cmd_service_search(query: &str) -> errors::AcResult<()> {
    let data_dir = storage::resolve_data_dir(data_dir_opt())?;
    let registry = service_discovery::load_registry(&data_dir)?;
    let results = registry.search(query);
    if results.is_empty() {
        println!("🔍 未找到匹配 \"{}\" 的服务", query);
    } else {
        println!("🔍 \"{}\" 找到 {} 个服务:", query, results.len());
        for (peer, svc) in &results {
            let short_peer = &peer[..std::cmp::min(12, peer.len())];
            println!(
                "  {:<12}  {:20}  {}  {:?}",
                short_peer, svc.id, svc.name, svc.tags
            );
        }
    }
    Ok(())
}

fn cmd_service_call(
    peer_id: &str,
    service_id: &str,
    method: &str,
    params_json: &str,
) -> errors::AcResult<()> {
    let params: serde_json::Value =
        serde_json::from_str(params_json).map_err(errors::AcError::Serialization)?;
    // Format as a "service call" message via the chat protocol
    let msg = serde_json::json!({
        "type": "service-call",
        "service_id": service_id,
        "method": method,
        "params": params,
    });
    let content = serde_json::to_string(&msg)?;
    println!("📡 调用 {}::{} → Peer {}", service_id, method, peer_id);
    println!("   参数: {}", content);
    println!("   提示: 先用 `service negotiate` 查询可用能力，或用 `--skip-negotiate` 跳过协商");
    Ok(())
}

fn cmd_service_negotiate(peer_id: &str, service_id: &str) -> errors::AcResult<()> {
    use agent_circle_core::identity::{CapabilityStatement, ProtocolVersion};

    let data_dir = storage::resolve_data_dir(data_dir_opt())?;
    let registry = service_discovery::load_registry(&data_dir)?;

    // Look up the peer's services in local cache
    let peer_services: Vec<_> = registry
        .all_services_with_meta()
        .into_iter()
        .filter(|(p, _, _)| p.starts_with(peer_id))
        .collect();

    if peer_services.is_empty() {
        println!("⚠️  本地缓存中未找到 Peer {} 的服务", peer_id);
        println!("   请先运行 daemon 发现服务, 或确认 PeerId 正确");
        return Ok(());
    }

    // Find the specific service
    let svc = peer_services.iter().find(|(_, s, _)| s.id == service_id);

    // Build a synthetic CapabilityStatement from cached ServiceInfo
    let statement = if let Some((_peer, svc, _last_seen)) = svc {
        let versions = if svc.protocol_versions.is_empty() {
            vec![ProtocolVersion {
                version: "1.0.0".into(),
                endpoint: svc.endpoint.clone(),
                input_schema: svc.input_schema.clone().unwrap_or_default(),
            }]
        } else {
            svc.protocol_versions
                .iter()
                .map(|v| ProtocolVersion {
                    version: v.clone(),
                    endpoint: svc.endpoint.clone(),
                    input_schema: svc.input_schema.clone().unwrap_or_default(),
                })
                .collect()
        };

        CapabilityStatement {
            service_id: svc.id.clone(),
            versions,
            accepted_formats: vec!["json".into()],
            service_found: true,
        }
    } else {
        CapabilityStatement {
            service_id: service_id.into(),
            versions: vec![],
            accepted_formats: vec![],
            service_found: false,
        }
    };

    // ── Display negotiation result ──────────────────────────────────
    println!();
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  🔧 服务能力协商                                       ║");
    println!("╠══════════════════════════════════════════════════════════╣");
    println!("║  目标 Peer:  {:<37} ║", peer_id);
    println!("║  服务 ID:    {:<37} ║", service_id);
    println!("╠══════════════════════════════════════════════════════════╣");

    if !statement.service_found {
        println!("║  ⚠️  该 Peer 未提供此服务                              ║");
        println!("║      可用的服务:");
        for (_, svc, _) in &peer_services {
            println!("║        • {} ({})", svc.id, svc.name);
        }
    } else {
        println!(
            "║  ✅ 服务可用 — {} 个协议版本:                          ║",
            statement.versions.len()
        );
        for v in &statement.versions {
            println!("║     {}  →  {:<39} ║", v.version, v.endpoint);
        }
        println!("╠══════════════════════════════════════════════════════════╣");
        println!(
            "║  接受格式:  {:<37} ║",
            statement.accepted_formats.join(", ")
        );
        if let Some(first) = statement.versions.first() {
            if !first.input_schema.is_empty() && first.input_schema != "{}" {
                println!("║  参数 Schema:                                         ║");
                for line in first.input_schema.lines().take(5) {
                    println!("║    {}", line);
                }
            }
        }
    }
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    if statement.service_found {
        println!("💡 协商成功！使用以下命令调用:");
        println!(
            "   agent-circle service call {} {} <METHOD> '{{\"key\":\"value\"}}'",
            peer_id, service_id
        );
    }
    Ok(())
}

// ── Service cache command (S10R108) ──────────────────────────────

fn cmd_service_cache(stats: bool, flush: bool) -> errors::AcResult<()> {
    let data_dir = storage::resolve_data_dir(data_dir_opt())?;
    let registry = service_discovery::load_registry(&data_dir)?;

    if flush {
        // Remove the services.json file to force a fresh start
        let path = data_dir.join("services.json");
        if path.exists() {
            std::fs::remove_file(&path).map_err(errors::AcError::Io)?;
            println!("🧹 服务缓存已清空。daemon 重启后将重新发现服务。");
        } else {
            println!("💡 缓存文件不存在，无需清空。");
        }
        return Ok(());
    }

    if stats {
        let peers = registry.peer_count();
        let services = registry.service_count();
        let has_cache = registry.has_cached_data();
        let freshness_secs = 600; // 10 min

        println!("╔══════════════════════════════════════════════════════╗");
        println!("║  💾 服务缓存摘要                                    ║");
        println!("╠══════════════════════════════════════════════════════╣");
        println!("║  缓存节点:  {:<38} ║", peers);
        println!("║  缓存服务:  {:<38} ║", services);
        println!(
            "║  状态:      {:<38} ║",
            if has_cache {
                "🟢 有缓存数据"
            } else {
                "⚪ 缓存为空"
            }
        );

        if has_cache {
            let stale_threshold = 600; // 10 min
            let now = chrono::Utc::now().timestamp();
            let all_services = registry.all_services_with_meta();
            let stale_count = all_services
                .iter()
                .filter(|(_, _, ts)| now - ts > stale_threshold)
                .count();
            let fresh_count = all_services.len() - stale_count;
            println!(
                "║  新鲜 ({freshness_secs}s):  {:<31} ║",
                format!("{} 个服务", fresh_count)
            );
            println!(
                "║  过期 (>{}s):  {:<33} ║",
                stale_threshold,
                format!("{} 个服务", stale_count)
            );
        }
        println!("╠══════════════════════════════════════════════════════╣");
        println!(
            "║  缓存文件:  {:<38} ║",
            data_dir.join("services.json").display().to_string()
        );
        println!("║  刷新命令:  agent-circle service cache --flush      ║");
        println!("╚══════════════════════════════════════════════════════╝");
    } else {
        // Default: show brief status
        let peers = registry.peer_count();
        let services = registry.service_count();
        println!(
            "💾 服务缓存: {} 节点 / {} 服务  {}",
            peers,
            services,
            if registry.has_cached_data() {
                "🟢"
            } else {
                "⚪"
            }
        );
        println!(
            "   services.json → {}",
            data_dir.join("services.json").display()
        );
        println!("   使用 `service cache --stats` 查看详情");
        println!("   使用 `service cache --flush` 清空缓存");
    }
    Ok(())
}

// ── Service publish command (S10R109) ─────────────────────────────

fn cmd_service_publish(
    service_id: &str,
    name: &str,
    endpoint: &str,
    description: Option<&str>,
    tags: &[String],
) -> errors::AcResult<()> {
    use agent_circle_core::identity::ServiceInfo;

    let data_dir = storage::resolve_data_dir(data_dir_opt())?;
    let mut registry = service_discovery::load_registry(&data_dir)?;

    // Create a new ServiceInfo entry
    let svc = ServiceInfo {
        id: service_id.to_string(),
        name: name.to_string(),
        endpoint: endpoint.to_string(),
        description: description.map(|s| s.to_string()),
        tags: tags.to_vec(),
        protocol_versions: vec!["1.0.0".to_string()],
        input_schema: Some("{}".to_string()),
    };

    // Publish as a local announcement
    let self_peer = "local".to_string();
    let ann = service_discovery::ServiceAnnouncement {
        peer_id: self_peer,
        services: vec![svc],
        ts: chrono::Utc::now().timestamp(),
    };
    registry.ingest(ann);
    service_discovery::save_registry(&registry, &data_dir)?;

    println!("📡 服务已发布到本地缓存:");
    println!("   ID:      {}", service_id);
    println!("   名称:    {}", name);
    println!("   端点:    {}", endpoint);
    if let Some(desc) = description {
        println!("   描述:    {}", desc);
    }
    if !tags.is_empty() {
        println!("   标签:    [{}]", tags.join(", "));
    }
    println!();
    println!("💡 提示: 在 daemon 模式下，此服务将自动通过 GossipSub 广播到网络。");
    Ok(())
}

// ── Service subscription commands (S10R107) ──────────────────────

/// Parse "service_id" or "service_id@peer_id" format.
fn parse_service_spec(spec: &str) -> (&str, Option<&str>) {
    if let Some((service_id, peer_id)) = spec.split_once('@') {
        (service_id, Some(peer_id))
    } else {
        (spec, None)
    }
}

fn cmd_service_subscribe(service_spec: &str, label: &str) -> errors::AcResult<()> {
    let data_dir = storage::resolve_data_dir(data_dir_opt())?;
    let mut subs = service_discovery::load_subscriptions(&data_dir)?;
    let (svc_id, peer_id) = parse_service_spec(service_spec);

    subs.subscribe(svc_id, peer_id, label);
    service_discovery::save_subscriptions(&subs, &data_dir)?;

    let target = if let Some(pid) = peer_id {
        format!("{}@{}", svc_id, pid)
    } else {
        svc_id.to_string()
    };
    let label_str = if label.is_empty() {
        String::new()
    } else {
        format!(" (\"{}\")", label)
    };
    println!("✅ 已订阅服务: {}{}", target, label_str);
    println!("   当该服务有更新时，daemon 将自动通知你。");
    Ok(())
}

fn cmd_service_unsubscribe(service_spec: &str) -> errors::AcResult<()> {
    let data_dir = storage::resolve_data_dir(data_dir_opt())?;
    let mut subs = service_discovery::load_subscriptions(&data_dir)?;
    let (svc_id, peer_id) = parse_service_spec(service_spec);

    let was_present = subs.unsubscribe(svc_id, peer_id);
    service_discovery::save_subscriptions(&subs, &data_dir)?;

    let target = if let Some(pid) = peer_id {
        format!("{}@{}", svc_id, pid)
    } else {
        svc_id.to_string()
    };

    if was_present {
        println!("✅ 已取消订阅: {}", target);
    } else {
        println!("⚠️  未找到订阅: {}", target);
    }
    Ok(())
}

fn cmd_service_subscriptions() -> errors::AcResult<()> {
    let data_dir = storage::resolve_data_dir(data_dir_opt())?;
    let subs = service_discovery::load_subscriptions(&data_dir)?;
    let items = subs.list();

    if items.is_empty() {
        println!("📭 暂无服务订阅");
        return Ok(());
    }

    println!("📋 已订阅 {} 个服务:", items.len());
    for s in items {
        let target = if let Some(ref pid) = s.peer_id {
            format!("{}@{}", s.service_id, pid)
        } else {
            s.service_id.clone()
        };
        let label = if s.label.is_empty() { "—" } else { &s.label };
        let age = chrono::Utc::now().timestamp() - s.created_at;
        let ago = if age < 60 {
            format!("{}s 前", age)
        } else if age < 3600 {
            format!("{}m 前", age / 60)
        } else {
            format!("{}h 前", age / 3600)
        };
        println!("   • {:<30}  [{}]  {}", target, label, ago);
    }
    Ok(())
}

// ── Plugin commands ────────────────────────────────────────────────

fn cmd_plugin_list() -> errors::AcResult<()> {
    let data_dir = storage::resolve_data_dir(data_dir_opt())?;
    let plugin_dir = data_dir.join("plugins");
    let mut registry = plugin_registry::PluginRegistry::new();
    registry.discover_and_load(&plugin_dir);
    let loaded = registry.loaded();
    if loaded.is_empty() {
        println!("📦 未加载插件 (目录: {})", plugin_dir.display());
        println!("   将 .so/.dylib/.dll 放入该目录后重启 daemon 即可加载");
    } else {
        println!("📦 已加载 {} 个插件:", loaded.len());
        for m in &loaded {
            println!("   {} v{} — {}  {}", m.id, m.version, m.name, m.description);
        }
    }
    Ok(())
}

// ── Doctor command (S11R111) ────────────────────────────────────────

/// Run a single check, returning (label, status_icon, detail).
type DoctorCheck = (&'static str, &'static str, String);

fn cmd_doctor(check_filter: Option<&str>, json: bool) -> errors::AcResult<()> {
    let data_dir = storage::resolve_data_dir(data_dir_opt())?;
    let mut checks: Vec<DoctorCheck> = Vec::new();

    let should_run = |name: &str| -> bool { check_filter.is_none() || check_filter == Some(name) };

    // ── Identity check ──────────────────────────────────────────
    if should_run("identity") {
        let _identity_path = data_dir.join("identity.key");
        match storage::load_identity(data_dir_opt()) {
            Ok(Some(id)) => {
                let short = &id.did[..std::cmp::min(48, id.did.len())];
                checks.push((
                    "identity",
                    "✅",
                    format!("DID: {} · 短码: {}", short, id.short_code),
                ));
            }
            Ok(None) => checks.push((
                "identity",
                "❌",
                "未创建身份 - 运行 `agent-circle identity create` 创建".into(),
            )),
            Err(e) => checks.push(("identity", "❌", format!("加载失败: {e}"))),
        }
    }

    // ── Storage check (S11R113 — integrity validation) ──────────
    if should_run("storage") {
        if data_dir.exists() {
            let mut parts: Vec<String> = Vec::new();

            // card.json
            let card_path = data_dir.join("card.json");
            let card_ok = card_path.exists();
            parts.push(format!("card.json {}", if card_ok { "✓" } else { "✗" }));

            // contacts.json — load + validate entries
            let contacts_path = data_dir.join("contacts.json");
            if contacts_path.exists() {
                match storage::load_contacts(data_dir_opt()) {
                    Ok(contacts) => {
                        let with_name = contacts.iter().filter(|c| !c.name.is_empty()).count();
                        parts.push(format!(
                            "contacts.json ✓ ({} entries, {} named)",
                            contacts.len(),
                            with_name
                        ));
                    }
                    Err(_) => parts.push("contacts.json ✗ (parse error)".into()),
                }
            } else {
                parts.push("contacts.json ✗".into());
            }

            // timeline.json — load + verify
            let tl_path = data_dir.join("timeline.json");
            if tl_path.exists() {
                match storage::load_timeline(data_dir_opt()) {
                    Ok(tl) => match tl.verify() {
                        Ok(()) => {
                            parts.push(format!("timeline.json ✓ ({} posts, verified)", tl.len()))
                        }
                        Err(e) => parts.push(format!(
                            "timeline.json ⚠ ({} posts, verify fail: {})",
                            tl.len(),
                            e
                        )),
                    },
                    Err(_) => parts.push("timeline.json ✗ (parse error)".into()),
                }
            } else {
                parts.push("timeline.json —".into());
            }

            // services.json
            let svc_path = data_dir.join("services.json");
            if svc_path.exists() {
                match service_discovery::load_registry(&data_dir) {
                    Ok(r) => parts.push(format!(
                        "services.json ✓ ({} peers / {} svc)",
                        r.peer_count(),
                        r.service_count()
                    )),
                    Err(_) => parts.push("services.json ✗ (parse error)".into()),
                }
            } else {
                parts.push("services.json ✗".into());
            }

            let status = if parts
                .iter()
                .any(|p| p.contains('✗') || p.contains("parse error"))
            {
                "❌"
            } else if parts.iter().any(|p| p.contains("⚠")) {
                "⚠️"
            } else {
                "✅"
            };

            checks.push(("storage", status, parts.join(" · ")));
        } else {
            checks.push((
                "storage",
                "❌",
                format!("数据目录不存在: {}", data_dir.display()),
            ));
        }
    }

    // ── Network check ───────────────────────────────────────────
    if should_run("network") {
        let sock = data_dir.join("control.sock");
        let sock_exists = sock.exists();
        let registry = service_discovery::load_registry(&data_dir).unwrap_or_default();
        let peer_count = registry.peer_count();
        let svc_count = registry.service_count();

        if sock_exists {
            let detail = if peer_count > 0 {
                let peers = registry.all_services_with_meta();
                let mut peer_set: Vec<String> = Vec::new();
                for (p, _, ts) in &peers {
                    let age = chrono::Utc::now().timestamp() - ts;
                    let freshness = if age < 120 {
                        "🟢"
                    } else if age < 600 {
                        "🟡"
                    } else {
                        "🔴"
                    };
                    let short = &p[..std::cmp::min(12, p.len())];
                    peer_set.push(format!("{}{}", freshness, short));
                }
                peer_set.sort();
                peer_set.dedup();
                format!(
                    "daemon 在线 · {} peers: {}",
                    peer_count,
                    peer_set.join(", ")
                )
            } else {
                "daemon 在线 · 无已发现 peers".into()
            };
            checks.push(("network", "✅", detail));
        } else {
            let detail = format!(
                "daemon 离线 · 缓存 {} peers / {} services",
                peer_count, svc_count
            );
            checks.push(("network", "⚠️", detail));
        }
    }

    // ── Contacts check ─────────────────────────────────────────
    if should_run("contacts") {
        match storage::load_contacts(data_dir_opt()) {
            Ok(contacts) => {
                if contacts.is_empty() {
                    checks.push(("contacts", "⚠️", "联系人列表为空".into()));
                } else {
                    let names: Vec<_> = contacts.iter().map(|c| c.name.clone()).collect();
                    checks.push((
                        "contacts",
                        "✅",
                        format!("{} 个联系人: {}", contacts.len(), names.join(", ")),
                    ));
                }
            }
            Err(e) => checks.push(("contacts", "❌", format!("加载失败: {e}"))),
        }
    }

    // ── Errors reference (S11R115) ───────────────────────────────
    if should_run("errors") {
        let codes = [
            (
                "E0001",
                "IO error — file, directory, or stream access failure",
            ),
            (
                "E0002",
                "Identity error — key missing/malformed/DID verification failed",
            ),
            (
                "E0003",
                "Serialization error — JSON/serde encode/decode failure",
            ),
            (
                "E0004",
                "Key error — cryptographic key derivation/import/signing failure",
            ),
            (
                "E0005",
                "Network error — P2P transport/dial/listen/swarm failure",
            ),
        ];
        for (code, desc) in &codes {
            checks.push(("errors", "📖", format!("{}: {}", code, desc)));
        }
    }

    // ── Display ─────────────────────────────────────────────────
    if json {
        let items: Vec<serde_json::Value> = checks
            .into_iter()
            .map(|(name, status, detail)| {
                serde_json::json!({"check": name, "status": status, "detail": detail})
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&items)?);
    } else {
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║  🩺 Agent Circle 全链路诊断                             ║");
        println!("╠══════════════════════════════════════════════════════════╣");
        for (name, icon, detail) in &checks {
            println!("║  {}  {:<8}  {:<38} ║", icon, name, detail);
        }
        println!("╠══════════════════════════════════════════════════════════╣");
        let pass = checks.iter().filter(|(_, i, _)| i == &"✅").count();
        let warn = checks.iter().filter(|(_, i, _)| i == &"⚠️").count();
        let fail = checks.iter().filter(|(_, i, _)| i == &"❌").count();
        println!(
            "║  总计: {} 项  通过: {}  警告: {}  失败: {}",
            checks.len(),
            pass,
            warn,
            fail
        );
        let overall = if fail > 0 {
            "❌ 有失败项"
        } else if warn > 0 {
            "⚠️ 有警告"
        } else {
            "✅ 全部通过"
        };
        println!("║  状态: {:<46} ║", overall);
        println!("╚══════════════════════════════════════════════════════════╝");
    }
    Ok(())
}

// ── Metrics command (S11R116) ──────────────────────────────────────

fn cmd_metrics() -> errors::AcResult<()> {
    let output = metrics::collect()?;
    print!("{output}");
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

// ── BIP-39 Mnemonic commands ──────────────────────────────────────

fn cmd_identity_mnemonic() -> errors::AcResult<()> {
    let mnemonic = keys::generate_mnemonic()?;
    println!("🔑 BIP-39 助记词（请安全保存！）\n");
    println!("   {}\n", mnemonic);
    println!("⚠️  用以下命令恢复身份：");
    println!("   agent-circle identity restore \"{}\"\n", mnemonic);
    println!("   丢失助记词 = 永久失去身份控制权。请离线保存。");
    Ok(())
}

fn cmd_identity_restore(mnemonic: &str, passphrase: &str) -> errors::AcResult<()> {
    // Validate mnemonic before deriving
    keys::validate_mnemonic(mnemonic).map_err(errors::AcError::Identity)?;
    let id = keys::derive_from_mnemonic(mnemonic, passphrase)?;
    save_identity(&id, data_dir_opt())?;
    println!("✅ 身份已从助记词恢复");
    println!("   DID:        {}", id.did);
    println!("   短码:       {}", id.short_code);
    Ok(())
}
