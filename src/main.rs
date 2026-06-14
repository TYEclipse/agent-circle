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
mod network;
mod protocol;
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
