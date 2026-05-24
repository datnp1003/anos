use anyhow::Result;
use colored::*;
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};
use rustyline::history::DefaultHistory;
use std::{env, path::PathBuf};
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
        let matches = COMMANDS
            .iter()
            .filter(|cmd| cmd.starts_with(prefix))
            .map(|cmd| Pair {
                display: (*cmd).to_string(),
                replacement: (*cmd).to_string(),
            })
            .collect();
        Ok((0, matches))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let sock = env::var("ANOS_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(SOCKET));
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        let msg = args[1..].join(" ");
        one_shot(&sock, &msg).await
    } else {
        interactive(&sock).await
    }
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
    println!("{}", "╰──────────────────────────────────╯".bright_black());

    let stream = UnixStream::connect(sock).await?;
    let (reader, mut writer) = stream.into_split();
    let mut buf = BufReader::new(reader);
    let mut g = String::new();
    buf.read_line(&mut g).await?;
    println!("{}", g.trim().dimmed());
    // Print daemon version on interactive startup without requiring a manual command.
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
            // Slash commands such as /providers, /model, /checks return plain text
            // (not prefixed with ">> "). Print them instead of swallowing output.
            println!("{}", t);
        }
    }
    Ok(())
}

fn show_help() {
    println!(
        "\n{}",
        "┌─ Commands ──────────────────────────┐".bright_black()
    );
    println!(
        "{}",
        "│  /help          — Show this          │".bright_black()
    );
    println!(
        "{}",
        "│  /exit, /quit   — Quit               │".bright_black()
    );
    println!(
        "{}",
        "│  /clear         — Clear screen       │".bright_black()
    );
    println!(
        "{}",
        "│  /version, /v   — Show Anos version  │".bright_black()
    );
    println!(
        "{}",
        "│  /providers, /p — List AI providers  │".bright_black()
    );
    println!(
        "{}",
        "│  /model         — Show current       │".bright_black()
    );
    println!(
        "{}",
        "│  /model <id>    — Switch provider    │".bright_black()
    );
    println!(
        "{}",
        "│  /memory        — Show memory       │".bright_black()
    );
    println!(
        "{}",
        "│  /audit         — Show audit log    │".bright_black()
    );
    println!(
        "{}",
        "│  /loop [n]      — Tool loop limit    │".bright_black()
    );
    println!(
        "{}",
        "│  /continue      — Continue max loop  │".bright_black()
    );
    println!(
        "{}",
        "│  /tools         — List tools         │".bright_black()
    );
    println!(
        "{}",
        "│  • Còn bao nhiêu disk trống?        │".bright_black()
    );
    println!(
        "{}",
        "│  • Process nào tốn CPU?             │".bright_black()
    );
    println!(
        "{}",
        "│  • Cài Neovim                       │".bright_black()
    );
    println!(
        "{}",
        "└──────────────────────────────────────┘\n".bright_black()
    );
}
