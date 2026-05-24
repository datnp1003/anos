use anyhow::Result;
use colored::*;
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::{env, process::Command};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::OwnedReadHalf;
use tokio::net::UnixStream;

const SOCKET: &str = "/tmp/anos.sock";
const COMMANDS: &[&str] = &[
    "/help",
    "/version",
    "/v",
    "/versions",
    "/providers",
    "/p",
    "/model",
    "/loop",
    "/continue",
    "/cont",
    "/tools",
    "/auto",
    "/watch",
    "/checks",
    "/alerts",
    "/memstatus",
    "/memindex",
    "/memsearch",
    "/stream",
    "/memory",
    "/audit",
    "/spawn",
    "/agents",
    "/hooks",
    "/snapshot",
    "/upgrade",
    "/ping",
    "/clear",
    "/exit",
    "/quit",
];

struct AnosHelper;
impl Helper for AnosHelper {}
impl Hinter for AnosHelper {
    type Hint = String;
}
impl Highlighter for AnosHelper {}
impl Validator for AnosHelper {}
impl Completer for AnosHelper {
    type Candidate = Pair;
    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        if !line.starts_with('/') {
            return Ok((0, Vec::new()));
        }
        let prefix = &line[..pos];
        Ok((
            0,
            COMMANDS
                .iter()
                .filter(|cmd| cmd.starts_with(prefix))
                .map(|cmd| Pair {
                    display: (*cmd).to_string(),
                    replacement: (*cmd).to_string(),
                })
                .collect(),
        ))
    }
}

#[derive(Debug, Clone)]
struct Paths {
    sock: PathBuf,
    anos_dir: PathBuf,
    anosd: PathBuf,
    cli: PathBuf,
    service_file: PathBuf,
    policy_file: PathBuf,
    providers_file: PathBuf,
}
impl Paths {
    fn load() -> Self {
        let home = PathBuf::from(env::var("HOME").unwrap_or_else(|_| ".".into()));
        let anos_dir = env::var("ANOS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".anos"));
        let bin_dir = env::var("ANOS_BIN_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".local/bin"));
        let user_systemd = home.join(".config/systemd/user");
        Self {
            sock: env::var("ANOS_SOCKET")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(SOCKET)),
            anosd: env::var("ANOSD_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|_| bin_dir.join("anosd")),
            cli: env::var("ANOS_CLI_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|_| bin_dir.join("anos-cli")),
            service_file: user_systemd.join("anosd.service"),
            policy_file: anos_dir.join("policy.yaml"),
            providers_file: anos_dir.join("config/providers.yaml"),
            anos_dir,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let paths = Paths::load();
    if args.len() > 1 {
        match args[1].as_str() {
            "status" => return cmd_status(&paths).await,
            "doctor" => return cmd_doctor(&paths).await,
            "setup" => return cmd_setup(&paths),
            "install-service" => return cmd_install_service(&paths),
            "policy" => return cmd_policy(&paths, &args[2..]),
            "--version" | "-V" => {
                println!("anos-cli {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => {
                let msg = args[1..].join(" ");
                return one_shot(&paths.sock, &msg).await;
            }
        }
    }
    interactive(&paths.sock).await
}

async fn one_shot(sock: &PathBuf, msg: &str) -> Result<()> {
    let stream = UnixStream::connect(sock).await?;
    let (reader, mut writer) = stream.into_split();
    let mut buf = BufReader::new(reader);
    let mut g = String::new();
    buf.read_line(&mut g).await?;
    writer
        .write_all(format!("{}\n/exit\n", msg).as_bytes())
        .await?;
    read_response(&mut buf).await?;
    Ok(())
}

async fn interactive(sock: &PathBuf) -> Result<()> {
    println!(
        "\n{}",
        "╭──────────────────────────────────╮".bright_black()
    );
    println!(
        "{}",
        "│  🦾  Anos — AI Native OS CLI      │"
            .to_string()
            .bright_yellow()
            .bold()
    );
    println!("{}", "│  /version /help /providers /exit │".dimmed());
    println!("{}", "│  status doctor setup policy      │".dimmed());
    println!("{}", "╰──────────────────────────────────╯".bright_black());

    let stream = UnixStream::connect(sock).await?;
    let (reader, mut writer) = stream.into_split();
    let mut buf = BufReader::new(reader);
    let mut g = String::new();
    buf.read_line(&mut g).await?;
    println!("{}", g.trim().dimmed());
    writer.write_all(b"/version\n").await?;
    let _ = read_response(&mut buf).await;

    let mut rl = Editor::<AnosHelper, DefaultHistory>::new()?;
    rl.set_helper(Some(AnosHelper));
    let _ = rl.load_history("/tmp/anos-history.txt");

    loop {
        let line = match rl.readline("\x1b[1;33manos>\x1b[0m ") {
            Ok(l) => l,
            Err(
                rustyline::error::ReadlineError::Interrupted | rustyline::error::ReadlineError::Eof,
            ) => {
                println!("\nBye!");
                break;
            }
            Err(_) => break,
        };
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let should_exit = matches!(t, "/exit" | "/quit");
        match t {
            "/help" | "/h" => {
                let _ = rl.add_history_entry(t);
                show_help();
                continue;
            }
            "/clear" | "/c" => {
                let _ = rl.add_history_entry(t);
                print!("\x1b[2J\x1b[H");
                continue;
            }
            _ => {
                let _ = rl.add_history_entry(t);
            }
        }
        if writer
            .write_all(format!("{}\n", t).as_bytes())
            .await
            .is_err()
        {
            eprintln!("{}", "Connection lost".red());
            break;
        }
        if read_response(&mut buf).await.is_err() {
            eprintln!("{}", "Connection lost".red());
            break;
        }
        if should_exit {
            break;
        }
    }
    let _ = rl.save_history("/tmp/anos-history.txt");
    Ok(())
}

async fn read_response(reader: &mut BufReader<OwnedReadHalf>) -> Result<()> {
    let mut line = String::new();
    let mut thinking = false;
    loop {
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            break;
        }
        let t = line.trim();
        if t == "[END]" {
            println!();
            break;
        }
        if t == "[THINKING]" {
            thinking = true;
            eprint!("{} ", "🤔".dimmed());
            continue;
        }
        if let Some(c) = t.strip_prefix(">> ") {
            if c.starts_with("🔧") || c.starts_with("⚠️") || c.starts_with("❌") {
                println!("{}", c);
            } else if thinking {
                eprint!("\r\x1b[K\r");
                print!("{}", "anos> ".bright_yellow());
                print!("{}", c);
                thinking = false;
            } else {
                print!("{}", c);
            }
        } else if !t.is_empty() {
            println!("{}", t);
        }
    }
    Ok(())
}

async fn cmd_status(paths: &Paths) -> Result<()> {
    println!("{}", "🦾 Anos Status".bright_yellow().bold());
    println!("Version:      {}", env!("CARGO_PKG_VERSION"));
    println!("ANOS_DIR:     {}", paths.anos_dir.display());
    println!("Socket:       {}", paths.sock.display());
    println!(
        "anosd:        {} {}",
        exists_icon(&paths.anosd),
        paths.anosd.display()
    );
    println!(
        "anos-cli:     {} {}",
        exists_icon(&paths.cli),
        paths.cli.display()
    );
    println!(
        "Policy:       {} {}",
        exists_icon(&paths.policy_file),
        paths.policy_file.display()
    );
    println!(
        "Providers:    {} {}",
        exists_icon(&paths.providers_file),
        paths.providers_file.display()
    );
    println!(
        "Service:      {} {}",
        exists_icon(&paths.service_file),
        paths.service_file.display()
    );

    if socket_alive(&paths.sock).await {
        println!("Daemon:       {} running", "✅".green());
        if let Ok(out) = ask_daemon(&paths.sock, "/version").await {
            println!("Daemon info:  {}", out.lines().next().unwrap_or("-"));
        }
    } else {
        println!("Daemon:       {} not reachable", "⚠️".yellow());
    }

    let service = Command::new("systemctl")
        .args(["--user", "is-active", "anosd.service"])
        .output();
    if let Ok(out) = service {
        println!(
            "systemd:      {}",
            String::from_utf8_lossy(&out.stdout).trim()
        );
    }
    Ok(())
}

async fn cmd_doctor(paths: &Paths) -> Result<()> {
    println!("{}", "🩺 Anos Doctor".bright_yellow().bold());
    check("Linux", cfg!(target_os = "linux"));
    check_path("ANOS_DIR", &paths.anos_dir);
    check_path("anosd binary", &paths.anosd);
    check_path("anos-cli binary", &paths.cli);
    check_path("providers.yaml", &paths.providers_file);
    check_path("policy.yaml", &paths.policy_file);
    check("socket reachable", socket_alive(&paths.sock).await);
    check("systemctl available", command_exists("systemctl"));
    check("git available", command_exists("git"));
    check("curl available", command_exists("curl"));
    check("write ~/.anos", can_write(&paths.anos_dir));
    check("write /etc (needs root)", can_write(Path::new("/etc")));
    check(
        "write /usr/local/bin (needs root)",
        can_write(Path::new("/usr/local/bin")),
    );
    if !paths.providers_file.exists() {
        println!("{} run: anos setup", "→".cyan());
    }
    if !paths.service_file.exists() {
        println!("{} optional: anos install-service", "→".cyan());
    }
    Ok(())
}

fn cmd_setup(paths: &Paths) -> Result<()> {
    fs::create_dir_all(paths.anos_dir.join("config"))?;
    ensure_policy(paths)?;
    println!("{}", "🦾 Anos Setup".bright_yellow().bold());
    println!("Press Enter to keep defaults. API key input is visible for now; paste carefully.");

    let provider = prompt("Provider id", "9router")?;
    let name = prompt("Provider name", "9Router")?;
    let base_url = prompt("Base URL", "https://9router.datnp.com/v1")?;
    let model = prompt("Model", "cmc/deepseek/deepseek-v4-pro")?;
    let api_key = prompt("API key (or env var name)", "")?;
    let active = prompt("Set active provider", &provider)?;

    let key_line = if api_key.is_empty() {
        "    apiKey: \"\"\n".to_string()
    } else if api_key
        .chars()
        .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
    {
        format!("    apiKeyEnv: {}\n", api_key)
    } else {
        format!("    apiKey: {}\n", api_key)
    };

    let yaml = format!(
        "active: {}\nproviders:\n  {}:\n    name: {}\n    baseUrl: {}\n{}    model: {}\n",
        active, provider, name, base_url, key_line, model
    );
    fs::write(&paths.providers_file, yaml)?;
    println!("{} wrote {}", "✅".green(), paths.providers_file.display());
    println!("Next: anos doctor && anos /providers");
    Ok(())
}

fn cmd_install_service(paths: &Paths) -> Result<()> {
    fs::create_dir_all(paths.service_file.parent().unwrap())?;
    fs::create_dir_all(&paths.anos_dir)?;
    let service = format!(
        "[Unit]\nDescription=Anos AI Native OS Daemon\nAfter=network-online.target\n\n[Service]\nType=simple\nEnvironment=ANOS_DIR={}\nEnvironment=ANOS_SOCKET={}\nExecStart={}\nRestart=on-failure\nRestartSec=2\n\n[Install]\nWantedBy=default.target\n",
        paths.anos_dir.display(),
        paths.sock.display(),
        paths.anosd.display()
    );
    fs::write(&paths.service_file, service)?;
    println!("{} wrote {}", "✅".green(), paths.service_file.display());
    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();
    println!("Run to enable now:");
    println!("  systemctl --user enable --now anosd.service");
    println!("  loginctl enable-linger $USER   # optional, keeps service after logout");
    Ok(())
}

fn cmd_policy(paths: &Paths, args: &[String]) -> Result<()> {
    fs::create_dir_all(&paths.anos_dir)?;
    if args.first().map(String::as_str) == Some("init") || !paths.policy_file.exists() {
        ensure_policy(paths)?;
        println!("{} wrote {}", "✅".green(), paths.policy_file.display());
        return Ok(());
    }
    println!("{}", paths.policy_file.display());
    println!("{}", fs::read_to_string(&paths.policy_file)?);
    Ok(())
}

fn ensure_policy(paths: &Paths) -> Result<()> {
    if paths.policy_file.exists() {
        return Ok(());
    }
    let home = env::var("HOME").unwrap_or_else(|_| "/home/user".into());
    let policy = format!(
        "# Anos permission policy skeleton\nmode: user\nfilesystem:\n  allow_write:\n    - {home}\n    - /tmp\n  deny_write:\n    - /etc\n    - /root\n    - /usr\ncommands:\n  require_confirm:\n    - apt install\n    - apt remove\n    - systemctl restart\n    - systemctl stop\n  deny:\n    - rm -rf /\n    - mkfs\n    - dd if=\n",
    );
    fs::write(&paths.policy_file, policy)?;
    Ok(())
}

async fn ask_daemon(sock: &PathBuf, msg: &str) -> Result<String> {
    let stream = UnixStream::connect(sock).await?;
    let (reader, mut writer) = stream.into_split();
    let mut buf = BufReader::new(reader);
    let mut greeting = String::new();
    buf.read_line(&mut greeting).await?;
    writer
        .write_all(format!("{}\n/exit\n", msg).as_bytes())
        .await?;
    let mut out = String::new();
    let mut line = String::new();
    loop {
        line.clear();
        if buf.read_line(&mut line).await? == 0 {
            break;
        }
        let t = line.trim();
        if t == "[END]" || t == "Bye!" {
            break;
        }
        if !t.is_empty() {
            out.push_str(t);
            out.push('\n');
        }
    }
    Ok(out)
}

async fn socket_alive(sock: &PathBuf) -> bool {
    ask_daemon(sock, "/ping")
        .await
        .map(|o| o.contains("pong"))
        .unwrap_or(false)
}

fn prompt(label: &str, default: &str) -> Result<String> {
    print!("{} [{}]: ", label, default);
    io::stdout().flush()?;
    let mut s = String::new();
    io::stdin().read_line(&mut s)?;
    let s = s.trim();
    Ok(if s.is_empty() {
        default.into()
    } else {
        s.into()
    })
}

fn exists_icon(path: &Path) -> ColoredString {
    if path.exists() {
        "✅".green()
    } else {
        "❌".red()
    }
}
fn check_path(label: &str, path: &Path) {
    check(&format!("{} ({})", label, path.display()), path.exists());
}
fn check(label: &str, ok: bool) {
    println!(
        "{} {}",
        if ok { "✅".green() } else { "⚠️".yellow() },
        label
    );
}
fn command_exists(cmd: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {} >/dev/null 2>&1", cmd))
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
fn can_write(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    let test = path.join(format!(".anos-write-test-{}", std::process::id()));
    match fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&test)
    {
        Ok(_) => {
            let _ = fs::remove_file(test);
            true
        }
        Err(_) => false,
    }
}

fn show_help() {
    println!(
        "\n{}",
        "┌─ Commands ──────────────────────────┐".bright_black()
    );
    for line in [
        "│  /help          — Show this          │",
        "│  /exit, /quit   — Quit               │",
        "│  /clear         — Clear screen       │",
        "│  /version, /v   — Show Anos version  │",
        "│  /providers, /p — List AI providers  │",
        "│  /model         — Show current       │",
        "│  /model <id>    — Switch provider    │",
        "│  /loop [n]      — Tool loop limit    │",
        "│  /continue      — Continue max loop  │",
        "│  /tools         — List tools         │",
        "│                                    │",
        "│  Shell commands:                   │",
        "│  anos status                       │",
        "│  anos doctor                       │",
        "│  anos setup                        │",
        "│  anos install-service              │",
        "│  anos policy [init]                │",
        "└──────────────────────────────────────┘",
    ] {
        println!("{}", line.bright_black());
    }
    println!();
}
