mod branch_name;
mod client;
mod mcp;
mod sessions;
mod upgrade;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use std::path::Path;

#[derive(Parser)]
#[command(
    name = "berth",
    version,
    about = "The berth daemon, and the commands agents and scripts run against it"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Directory to add as a project — shorthand for `berth add <dir>`.
    /// (A directory whose name collides with a subcommand needs the long
    /// form or a `./` prefix.)
    dir: Option<String>,
}

/// `--kind` for `berth spawn`: one of the agent CLIs berth runs.
fn parse_agent_kind(s: &str) -> Result<berth_core::AgentKind, String> {
    berth_core::AgentKind::parse(s).ok_or_else(|| {
        format!(
            "unknown harness `{s}` — expected one of {}",
            berth_core::AgentKind::ALL
                .iter()
                .map(|k| k.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

#[derive(Subcommand)]
enum Command {
    /// Add a directory as a project, named after the repo's root directory
    /// (`berth add .` for the current one; bare `berth <dir>` works too).
    Add {
        /// Path to a git repository (default: the current directory).
        #[arg(default_value = ".")]
        path: String,
    },
    /// Run the daemon process (normally auto-spawned by the desktop app or a
    /// command that needs it).
    Daemon {
        /// Stay attached to the terminal instead of logging to file.
        #[arg(long)]
        foreground: bool,
    },
    /// Ask a running daemon to shut down cleanly.
    Kill,
    #[command(
        about = "Start the daemon when none runs and print its socket path (the desktop app runs this over ssh)"
    )]
    Socket,
    /// Title this session (run from inside a berth agent session; agents
    /// use it to auto-title on the first prompt).
    Rename {
        /// The new title; multiple words need no quotes.
        #[arg(required = true, num_args = 1..)]
        title: Vec<String>,
        /// Replace an existing title instead of only filling in a missing one.
        #[arg(long)]
        force: bool,
    },
    /// Move this session into a worktree of its project (run from inside a
    /// berth agent session; agents run it when you ask them to work in a
    /// worktree). Creates the checkout when the branch has none, re-homes
    /// the session at once, and restarts it resumed inside the worktree as
    /// soon as the current turn ends.
    Worktree {
        /// Branch name; several words are joined with hyphens, none at all
        /// gets a random `<adj>-<noun>-<verb>` one.
        name: Vec<String>,
        /// Start point for a new branch (default: the checkout's HEAD).
        #[arg(long, value_name = "REF")]
        base: Option<String>,
    },
    /// Move this session into another project (run from inside a berth
    /// agent session; agents run it when you ask them to go to another
    /// repo). Adds the repo as a project when berth does not have it yet,
    /// re-homes the session at once, and restarts it resumed there as soon
    /// as the current turn ends.
    Enter {
        /// The repo's directory, or one of its worktrees.
        dir: String,
    },
    /// Start a new agent session beside this one — same worktree, same
    /// harness unless --kind names another — opening on the given task as
    /// its first prompt (run from inside a berth agent session; agents run
    /// it when you ask for a new berth session). The new row shows up in
    /// the sessions list on its own; this session carries on untouched.
    Spawn {
        /// The task the new session starts on; multiple words need no quotes.
        #[arg(required = true, num_args = 1..)]
        task: Vec<String>,
        /// Harness for the new session: claude, codex, cursor, opencode or grok (default:
        /// the same as this session's).
        #[arg(long, value_name = "KIND", value_parser = parse_agent_kind)]
        kind: Option<berth_core::AgentKind>,
    },
    #[command(about = "List sessions as JSON (live ones; --all adds archived)")]
    Sessions {
        #[arg(long, help = "Include archived sessions")]
        all: bool,
    },
    #[command(
        about = "List the sessions that need you as JSON: waiting on a permission or question, or finished a turn nobody has read"
    )]
    Attention,
    #[command(
        about = "Type a message into a running session and press Enter; prints JSON. The session is an id, an id prefix or an exact name"
    )]
    Send {
        session: String,
        #[arg(required = true, num_args = 1.., help = "The message; multiple words need no quotes")]
        text: Vec<String>,
        #[arg(long, help = "Leave the message in the input box without pressing Enter")]
        no_enter: bool,
    },
    #[command(about = "Serve MCP over stdio: tools to list sessions, see which need you and message one")]
    Mcp,
    /// Manage workspaces — named project groups.
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    /// Install the latest published berth over this one.
    Upgrade {
        /// Upgrade even when running from a local cargo build.
        #[arg(long)]
        force: bool,
    },
    /// Installer hook: print the cutover note only when a live daemon is on
    /// a different build than this binary (see `make install` / install.sh).
    #[command(hide = true, name = "_stale-daemon-note")]
    StaleDaemonNote,
}

#[derive(Subcommand)]
enum WorkspaceCommand {
    /// Create a workspace (does not open it).
    Add { name: String },
    /// Make a workspace the one berth opens into.
    Open { name: String },
    /// List workspaces; `*` marks the one new instances open into.
    List,
    /// Delete an empty workspace.
    Delete { name: String },
    /// Rename a workspace.
    Rename { name: String, new_name: String },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    berth_core::env::check_isolation_vars().map_err(anyhow::Error::msg)?;
    if let Some(note) = berth_daemon::pacer_migration::run()? {
        eprintln!("berth: {note}");
    }
    match cli.command {
        Some(Command::Daemon { foreground }) => {
            init_daemon_logging(foreground)?;
            log_fatal(
                berth_daemon::run_daemon(),
                &berth_core::paths::daemon_log_path(),
            )
        }
        Some(Command::Add { path }) => client::run_add_project(path),
        Some(Command::Workspace { command }) => {
            use client::WorkspaceOp;
            let op = match command {
                WorkspaceCommand::Add { name } => WorkspaceOp::Add { name },
                WorkspaceCommand::Open { name } => WorkspaceOp::Open { name },
                WorkspaceCommand::List => WorkspaceOp::List,
                WorkspaceCommand::Delete { name } => WorkspaceOp::Delete { name },
                WorkspaceCommand::Rename { name, new_name } => {
                    WorkspaceOp::Rename { name, new_name }
                }
            };
            client::run_workspace(op)
        }
        Some(Command::Sessions { all }) => sessions::run_sessions(all),
        Some(Command::Attention) => sessions::run_attention(),
        Some(Command::Send {
            session,
            text,
            no_enter,
        }) => sessions::run_send(session, text.join(" "), !no_enter),
        Some(Command::Mcp) => mcp::run_mcp(),
        Some(Command::Kill) => client::run_kill(),
        Some(Command::Socket) => client::run_socket(),
        Some(Command::Rename { title, force }) => {
            let mode = if force {
                client::RenameMode::Force
            } else {
                client::RenameMode::Auto
            };
            client::run_rename(title.join(" "), mode)
        }
        Some(Command::Worktree { name, base }) => client::run_worktree(name.join(" "), base),
        Some(Command::Enter { dir }) => client::run_enter(dir),
        Some(Command::Spawn { task, kind }) => client::run_spawn(task.join(" "), kind),
        Some(Command::Upgrade { force }) => upgrade::run_upgrade(force),
        Some(Command::StaleDaemonNote) => {
            if berth_daemon::lifecycle::daemon_is_stale() {
                println!("note: the running daemon was built from older code.");
                println!("{}", upgrade::KILL_HINT);
            }
            Ok(())
        }
        None => match cli.dir {
            Some(dir) => client::run_add_project(dir),
            None => {
                Cli::command().print_help()?;
                Ok(())
            }
        },
    }
}

/// Record a fatal top-level error in the log file before it goes to stderr —
/// the daemon's stderr is /dev/null.
fn log_fatal<T>(result: Result<T>, log_path: &Path) -> Result<T> {
    if let Err(err) = &result {
        berth_core::crashlog::append(log_path, &format!("FATAL {err:#}"));
    }
    result
}

fn log_filter() -> tracing_subscriber::EnvFilter {
    berth_core::env::var(berth_core::env::LOG)
        .and_then(|filter| tracing_subscriber::EnvFilter::try_new(filter).ok())
        .unwrap_or_else(|| tracing_subscriber::EnvFilter::new("info"))
}

/// Route tracing to `log_path` (created on demand, appended, no ANSI) — the
/// daemon has no stderr.
fn init_file_logging(log_path: &Path) -> Result<()> {
    std::fs::create_dir_all(berth_core::paths::log_dir())?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    tracing_subscriber::fmt()
        .with_env_filter(log_filter())
        .with_writer(file)
        .with_ansi(false)
        .init();
    Ok(())
}

fn init_daemon_logging(foreground: bool) -> Result<()> {
    // The daemon runs detached with stderr on /dev/null — without this hook a
    // panic (on any thread, tokio workers included) leaves no trace.
    let log_path = berth_core::paths::daemon_log_path();
    berth_core::crashlog::install_panic_hook(log_path.clone());
    if foreground {
        tracing_subscriber::fmt()
            .with_env_filter(log_filter())
            .init();
        return Ok(());
    }
    init_file_logging(&log_path)
}
