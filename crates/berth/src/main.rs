mod channel;
mod client;
mod mcp;
mod sessions;
mod shared_setup;
mod triggers;
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
    berth_core::AgentKind::parse(s)
        .filter(|kind| berth_core::AgentKind::NATIVE.contains(kind))
        .ok_or_else(|| {
            format!(
                "unknown harness `{s}` — expected one of {}",
                berth_core::AgentKind::NATIVE
                    .iter()
                    .map(|k| k.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

#[derive(clap::Args)]
struct TargetArgs {
    #[arg(
        long,
        value_name = "ID",
        help = "Type into this session (default: the session this runs in)"
    )]
    agent: Option<String>,
    #[arg(
        long,
        value_name = "ID",
        help = "Start a new session in this worktree instead"
    )]
    worktree: Option<String>,
    #[arg(long, value_name = "KIND", value_parser = parse_agent_kind, help = "Harness of the new session")]
    kind: Option<berth_core::AgentKind>,
}

impl From<TargetArgs> for triggers::TriggerTargetArgs {
    fn from(t: TargetArgs) -> Self {
        Self {
            agent: t.agent,
            worktree: t.worktree,
            kind: t.kind,
        }
    }
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
        #[arg(
            long,
            value_name = "REF",
            help = "Start point for a new branch (default: worktree_base in config.json, else develop, main or master)"
        )]
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
    #[command(
        about = "End this session once the current turn ends: its process stops and it moves to Ended in the Sessions tree, where opening it resumes the conversation, and later to the Archive per the session_archive_after setting (run from inside a berth agent session; agents run it when you ask them to end or close the session)"
    )]
    EndSession,
    /// Start a new agent session beside this one — same worktree, same
    /// harness unless --kind names another — opening on the given task as
    /// its first prompt (run from inside a berth agent session; agents run
    /// it when you ask for a new berth session). The new row shows up in
    /// the sessions list on its own; this session carries on untouched.
    Spawn {
        /// The task the new session starts on; multiple words need no quotes.
        #[arg(required = true, num_args = 1..)]
        task: Vec<String>,
        /// Harness for the new session: claude, codex or grok (default:
        /// the same as this session's).
        #[arg(long, value_name = "KIND", value_parser = parse_agent_kind)]
        kind: Option<berth_core::AgentKind>,
        #[arg(
            long,
            value_name = "BRANCH",
            help = "Start the session in this branch's worktree of the project instead, creating the branch and worktree when there is none"
        )]
        worktree: Option<String>,
        #[arg(
            long,
            value_name = "REF",
            requires = "worktree",
            help = "Start point for a new --worktree branch (default: worktree_base in config.json, else develop, main or master)"
        )]
        base: Option<String>,
    },
    #[command(
        about = "Type a prompt into a session, or start a new session on it, after --in (once), repeating with --every"
    )]
    Schedule {
        #[arg(required = true, num_args = 1.., help = "The prompt; multiple words need no quotes")]
        prompt: Vec<String>,
        #[arg(long = "in", value_name = "SPAN", value_parser = triggers::parse_span, help = "Delay before the first run: 30m, 2h, 1d")]
        delay: std::time::Duration,
        #[arg(long, value_name = "SPAN", value_parser = triggers::parse_span, help = "Repeat every span, at least 60s")]
        every: Option<std::time::Duration>,
        #[arg(long)]
        name: Option<String>,
        #[command(flatten)]
        target: TargetArgs,
    },
    #[command(
        about = "Create a webhook that types a prompt into a session, or starts a new one; prints its secret once. Needs \"webhooks\": true in the config"
    )]
    Webhook {
        #[arg(required = true, num_args = 1.., help = "The prompt; multiple words need no quotes")]
        prompt: Vec<String>,
        #[arg(long)]
        name: Option<String>,
        #[command(flatten)]
        target: TargetArgs,
    },
    #[command(about = "List scheduled prompts and webhooks as JSON, or delete one with --remove")]
    Triggers {
        #[arg(long, value_name = "ID")]
        remove: Option<String>,
    },
    #[command(
        about = "Read and post in the team channels of the hub set in the config; prints JSON"
    )]
    Channel {
        #[command(subcommand)]
        command: ChannelCommand,
    },
    #[command(about = "Run the team hub, or pair and approve a device")]
    Hub {
        #[command(subcommand)]
        command: HubCommand,
    },
    #[command(about = "List sessions as JSON (live ones; --all adds ended and archived)")]
    Sessions {
        #[arg(long, help = "Include ended and archived sessions")]
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
        #[arg(
            long,
            help = "Leave the message in the input box without pressing Enter"
        )]
        no_enter: bool,
    },
    #[command(
        about = "Serve MCP over stdio: tools to list sessions, see which need you and message one"
    )]
    Mcp,
    /// Manage workspaces — named project groups.
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    #[command(
        about = "Install the shared setup named by shared_setup in the config into every project and worktree"
    )]
    SharedSetup {
        #[command(subcommand)]
        command: SharedSetupCommand,
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
enum ChannelCommand {
    #[command(about = "List the channels you are in, with their unread counts")]
    List,
    #[command(
        about = "Create the channel for a Jira issue key, a merge request branch or a Worktree, or open it when it exists; with no key, inside a session, the key of the session's branch"
    )]
    Open {
        key: Option<String>,
        #[arg(
            long,
            help = "Make it a Shared Channel: everyone approved on the hub is in it, now and later. Needs admin.manage"
        )]
        shared: bool,
    },
    #[command(
        about = "Leave a channel: it goes from your list and your Agents leave it. A Shared Channel cannot be left"
    )]
    Leave { channel: String },
    #[command(
        about = "Delete a channel with its messages, for everyone: one you made, or any with a role that holds channels.delete"
    )]
    Delete { channel: String },
    #[command(about = "Print the events of a channel after a sequence number")]
    Read {
        channel: String,
        #[arg(long, default_value_t = 0)]
        after: i64,
    },
    #[command(
        about = "Post a message in a channel; from inside a session it is posted as that Agent"
    )]
    Post {
        channel: String,
        #[arg(required = true, num_args = 1.., help = "The message; multiple words need no quotes")]
        message: Vec<String>,
    },
    #[command(
        about = "Set the note of a channel's task: every member's Agent that starts a session on the task gets it. The latest note wins; no text clears it"
    )]
    Note {
        channel: String,
        #[arg(num_args = 0.., help = "The note; multiple words need no quotes")]
        text: Vec<String>,
    },
    #[command(about = "Answer your Agent's request in a channel: allow, deny or an option number")]
    Answer {
        channel: String,
        #[arg(help = "The seq of the request event, as `berth channel read` prints it")]
        request: i64,
        answer: String,
    },
    #[command(
        about = "Deliver a colleague's message to your Agent as an instruction; without --agent, to each of your Agents it mentions"
    )]
    Promote {
        channel: String,
        #[arg(help = "The message's sequence number, as `berth channel read` prints it")]
        seq: i64,
        #[arg(long, value_name = "AGENT_ID")]
        agent: Option<String>,
    },
    #[command(
        about = "Let a colleague instruct one of your Agents in a channel: their mentions of it arrive as instructions"
    )]
    Grant {
        channel: String,
        agent: String,
        #[arg(help = "The colleague's hub member id, or their name as it shows in the channel")]
        colleague: String,
    },
    #[command(about = "Take back a grant")]
    Revoke {
        channel: String,
        agent: String,
        colleague: String,
    },
    #[command(about = "List your grants, in one channel or in all")]
    Grants { channel: Option<String> },
}

#[derive(Subcommand)]
enum HubCommand {
    #[command(about = "Serve the hub; binds loopback unless --bind says otherwise")]
    Serve {
        #[arg(long, default_value = "127.0.0.1:7843")]
        bind: std::net::SocketAddr,
        #[arg(long, value_name = "FILE")]
        db: Option<std::path::PathBuf>,
        #[arg(long)]
        admin_token: Option<String>,
        #[arg(
            long,
            value_name = "PEM",
            requires = "key",
            help = "Serve HTTPS with this certificate chain"
        )]
        cert: Option<std::path::PathBuf>,
        #[arg(
            long,
            value_name = "PEM",
            requires = "cert",
            help = "The private key of --cert"
        )]
        key: Option<std::path::PathBuf>,
    },
    #[command(about = "Step one: admit this device; prints its token, which opens nothing yet")]
    Pair { url: String, name: String },
    #[command(
        about = "Step two: approve an admitted device with a role, or change the role of an approved one"
    )]
    Approve {
        url: String,
        #[arg(
            long,
            help = "The admin token of `berth hub serve`, or the device token of someone whose role holds admin.manage"
        )]
        admin_token: String,
        id: String,
        #[arg(
            long,
            default_value = "member",
            help = "admin, member, guest, or the id of a custom role"
        )]
        role: String,
    },
    #[command(about = "List the devices of the hub with their status and role, and the roles")]
    Devices {
        url: String,
        #[arg(long)]
        admin_token: String,
    },
    #[command(about = "Revoke a device: its token and its Agents stop working at once")]
    Revoke {
        url: String,
        #[arg(long)]
        admin_token: String,
        id: String,
    },
}

#[derive(Subcommand)]
enum SharedSetupCommand {
    #[command(
        about = "Fetch the shared setup when it is a repository, then update every checkout from it"
    )]
    Sync,
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
        Some(Command::SharedSetup {
            command: SharedSetupCommand::Sync,
        }) => shared_setup::run_sync(),
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
        Some(Command::EndSession) => client::run_end_session(),
        Some(Command::Spawn {
            task,
            kind,
            worktree,
            base,
        }) => client::run_spawn(task.join(" "), kind, worktree, base),
        Some(Command::Schedule {
            prompt,
            delay,
            every,
            name,
            target,
        }) => triggers::run_schedule(prompt.join(" "), name, delay, every, target.into()),
        Some(Command::Webhook {
            prompt,
            name,
            target,
        }) => triggers::run_webhook(prompt.join(" "), name, target.into()),
        Some(Command::Triggers { remove }) => triggers::run_triggers(remove),
        Some(Command::Channel { command }) => match command {
            ChannelCommand::List => channel::run_list(),
            ChannelCommand::Open { key, shared } => channel::run_open(key, shared),
            ChannelCommand::Leave { channel } => channel::run_leave(channel),
            ChannelCommand::Delete { channel } => channel::run_delete(channel),
            ChannelCommand::Read { channel, after } => channel::run_read(channel, after),
            ChannelCommand::Post { channel, message } => {
                channel::run_post(channel, message.join(" "))
            }
            ChannelCommand::Note { channel, text } => channel::run_note(channel, text.join(" ")),
            ChannelCommand::Answer {
                channel,
                request,
                answer,
            } => channel::run_answer(channel, request, answer),
            ChannelCommand::Promote {
                channel,
                seq,
                agent,
            } => channel::run_promote(channel, seq, agent),
            ChannelCommand::Grant {
                channel,
                agent,
                colleague,
            } => channel::run_grant(channel, agent, colleague, true),
            ChannelCommand::Revoke {
                channel,
                agent,
                colleague,
            } => channel::run_grant(channel, agent, colleague, false),
            ChannelCommand::Grants { channel } => channel::run_grants(channel),
        },
        Some(Command::Hub { command }) => match command {
            HubCommand::Serve {
                bind,
                db,
                admin_token,
                cert,
                key,
            } => {
                let tls = cert
                    .zip(key)
                    .map(|(cert, key)| berth_hub::TlsFiles { cert, key });
                channel::run_hub_serve(bind, db, admin_token, tls)
            }
            HubCommand::Pair { url, name } => channel::run_hub_pair(url, name),
            HubCommand::Approve {
                url,
                admin_token,
                id,
                role,
            } => channel::run_hub_approve(url, admin_token, id, role),
            HubCommand::Devices { url, admin_token } => channel::run_hub_devices(url, admin_token),
            HubCommand::Revoke {
                url,
                admin_token,
                id,
            } => channel::run_hub_revoke(url, admin_token, id),
        },
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
