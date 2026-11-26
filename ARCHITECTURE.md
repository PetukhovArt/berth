# How Berth works

Berth is a self-hosted agent platform: it runs coding agents from any vendor for you and your team, on your own machines.
Where it is heading is in `VISION.md`, vocabulary in `GLOSSARY.md`. Details live in the code: this
file is the map, not the spec.

## Process model

1. **Daemon** (`berth daemon`) — owns every PTY, SQLite, git worktrees, agent status. Started when
   nothing is listening: a `berth` command that needs one spawns its own binary (`current_exe`) in a
   new session (`setsid`), and the desktop app starts `berth daemon` detached from itself, from the
   `berth` bundled beside it in a release build or from `PATH` in a working copy or a source build.
   Either way it outlives its clients and holds no controlling terminal.
2. **Clients** on the daemon socket: an `AF_UNIX` socket in the `0700` runtime dir on unix, or —
   Windows has none — a loopback TCP listener plus a bearer token there instead. One or the other,
   never both; `berth-core/src/transport.rs` hides the difference behind one API. The clients are the
   desktop app (`apps/desktop`), which also serves its web UI to a phone over remote access, and the
   one-shot `berth` commands people, agents and scripts run (listed under Crate layout).
   Close the app and nothing dies; reopen it and scrollback is replayed.
3. **Team hub** (`berth hub serve`, crate `berth-hub`), optional: a separate server for a team. Daemons
   of its members connect to it for Channels, mentions, presence, task notes and touched modules; it
   checks Roles and Permissions on every request.

IPC is length-prefixed MessagePack: `ClientRequest` in (CRUD, attach, keystrokes, resize),
`ServerEvent` out (entity deltas, status, PTY output). Attach replays the per-PTY ring buffer, then
streams live output.

## Domain tree

**Workspace → Project → Worktree → Session** (an agent or a plain terminal). Exactly one workspace is
open per client connection. Worktrees are real `git worktree`s under `<repo>/../<repo-name>-worktrees/`.
Everything persists in SQLite under the data dir.

## Agent status (not MCP)

At spawn the daemon writes managed hooks into the agent CLI's config; the hooks curl the daemon's
loopback HTTP receiver with a per-boot bearer token. Hook events plus the PTY's OSC 9;4 progress
escapes drive a per-agent state machine that maps to the colored status dots. Claude, Codex and Grok share
one hooks dialect, each with its own endpoint (`agent/`). The same hook channel powers auto-titling.
Cursor, OpenCode and the other ACP providers have no hooks: the daemon drives them over the Agent
Client Protocol (`acp/`) and takes their status from that session.

## Side paths

In the daemon (`crates/berth-daemon/src`): the
prewarm pool, the idle reaper, restoring live sessions after a restart (`restore.rs`), the metrics
sampler (`sampler.rs`) behind the desktop app's metrics panel, and the chat bridge (`messenger/`):
notifications to Telegram or a webhook, and prompts the owner sends the Telegram bot, whose voice
notes the daemon hands to the desktop app's whisper engine over the Daemon Socket
(`OfferTranscription` / `TranscribeVoice`) so the daemon itself never links it. Also in the daemon:
the LLM Router (`router/`), scheduled prompts and webhooks (`triggers.rs`), Checkpoints
(`checkpoints.rs`), the team side (`team*.rs`, `channels.rs`, `channel_requests.rs`, `hub_stream.rs`,
`presence.rs`, `task_notes.rs`), shared setup (`shared_setup/`), usage limits (`usage_limits.rs`) and
the ACP driver (`acp/`).

In the desktop app (`apps/desktop/src-tauri/src`): remote access (`remote.rs`, `remote_devices.rs`,
`tailscale.rs`), the ssh remote daemon (`ssh_remote.rs`), Web Push (`push.rs`), connectors, the
transcript search index (`search_index/`), voice (`voice/`), ACP provider checks (`providers.rs`),
worktree cleanup (`cleanup.rs`), the working-copy updater (`working_copy.rs`) and the merge-request
watcher (`merge_watch.rs`, decisions in `berth-core/src/merge_watch.rs`) behind archive on merge,
tracker write-back and watching a merge request, which the desktop app and `berth-server` both run,
one at a time per daemon.

## Crate layout

| Crate | Role |
|---|---|
| `berth` | Thin CLI: `daemon`, `socket`, `kill`, `add`, `rename`, `worktree`, `enter`, `end-session`, `spawn`, `schedule`, `webhook`, `triggers`, `channel`, `hub`, `sessions`, `attention`, `send`, `mcp`, `workspace`, `shared-setup`, `upgrade`; no args prints help. `client.rs` holds the daemon client: auto-spawn, handshake, kill |
| `berth-core` | Shared protocol, entities, IDs, paths, codec |
| `berth-daemon` | PTYs, SQLite, git, hook receiver, status engine, ACP driver |
| `berth-hub` | The team hub server: members, Roles and Permissions, Channels and their events, mentions, presence, notes, touched modules, admin and audit, optional TLS |
| `berth-transcripts` | Claude and Codex transcript parsing (messages and their keys, titles, reading a file) and the search index core (schema, per-file indexing, SQL queries); the desktop app wraps it with discovery, the daemon and texts |

## Apps

| Path | Role |
|---|---|
| `apps/desktop/src-tauri` | `berth-desktop` crate: the Tauri shell, a daemon client on `berth-core` (`berth.rs`), connectors, remote access, and the headless `berth-server` binary |
| `apps/web` | Vue UI the desktop window and `berth-server` both serve; Feature-Sliced layers under `src/` (`app` > `pages` > `widgets` > `features` > `entities` > `shared`, enforced by stratum), UI kit and icons only through `shared/ui`, Rust only through `bridge.ts` |
| `apps/model-router` | a small Node proxy that speaks the Anthropic API and sends each model to a chain of upstream providers, with fallback on errors and limits; every upstream call goes through `HTTPS_PROXY` |
| `packages/ui-kit`, `packages/ui-tokens` | components and design tokens for `apps/web` |
| `docker` | image with `berth` and `berth-server` for a remote machine |

The desktop crate is a workspace member but not a default member: a bare `cargo build` skips it, and
CI excludes it because it needs a webview toolchain. The release workflow builds it in its own
`desktop` job, with the `berth` CLI bundled beside the app.

`vendor/vt100` is a patched fork (DECSTBM scrollback fix — see its Cargo.toml); don't swap it for
crates.io vt100.

## Platform adapters

What differs between Windows, macOS and Linux goes through one adapter per layer:
`berth-core/src/platform/` for what the crates and the desktop share (openers,
program lookup, process table and memory, killing a process tree, symlinks, path case rules,
termination signals, owner-only files) and `apps/desktop/src-tauri/src/platform/` for the desktop's
own (child lifetime, self-update and the install dir of a running build, CLI search dirs, the login
console, webview profile dirs, window chrome, the tray icon, Dock reopen). Each has a facade `mod.rs` and one implementation file per OS (`windows.rs`, `macos.rs`,
`linux.rs`, with `unix.rs` shared), picked by `cfg` in that `mod.rs` and nowhere else. Callers never
write `cfg(target_os)`, `cfg(windows)` or `cfg!(…)`: they call the facade, or branch on a
`platform::CAPABILITIES` field (or `SELF_UPDATE`, `WINDOW_CHROME`) named for the consequence. An
OS without an implementation answers "unsupported" (`None`, `false`, `Err`) so the UI can hide the
action. A new concern is a facade function plus one function per OS file; a new OS is one more file.
The web UI reads the same facts from `apps/web/src/shared/lib/platform.ts` (the `platform_info` command, with a
userAgent guess for the client). Concerns that own a whole module keep their split behind that
module's API in the same way: `transport` and `paths` in core, the daemon's PTY layer, `launch` and
`lifecycle`, and the CLI's `client` (daemon auto-spawn) and `upgrade`. `node scripts/cross-clippy.mjs`
lints the other OSes' halves from any host.
