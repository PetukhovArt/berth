# How Berth works

Berth is a tmux-style terminal multiplexer for AI coding agents: run Claude / Codex / Cursor CLI
sessions across git repos and worktrees; they keep running after you close the UI. Vocabulary lives
in `GLOSSARY.md`. Details live in the code — this file is the map, not the spec.

## Process model

1. **Daemon** (`berth daemon`) — owns every PTY, SQLite, git worktrees, agent status. Started when
   nothing is listening: a `berth` command that needs one spawns its own binary (`current_exe`) in a
   new session (`setsid`), and the desktop app starts `berth daemon` from `PATH` detached from itself.
   Either way it outlives its clients and holds no controlling terminal.
2. **Clients** on the daemon socket: an `AF_UNIX` socket in the `0700` runtime dir on unix, or —
   Windows has none — a loopback TCP listener plus a bearer token there instead. One or the other,
   never both; `berth-core/src/transport.rs` hides the difference behind one API. The clients are the
   desktop app (`apps/desktop`), which also serves its web UI to a phone over remote access, and the
   one-shot `berth` commands (`add`, `rename`, `worktree`, `enter`, `spawn`, `workspace`, `kill`).
   Close the app and nothing dies; reopen it and scrollback is replayed.

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
escapes drive a per-agent state machine that maps to the colored status dots. Claude and Codex share
one hooks dialect; Cursor speaks its own. The same hook channel powers auto-titling.

## Side paths

In the daemon (`crates/berth-daemon/src`): cloud sessions (`claude --cloud` / `--teleport`), the
prewarm pool, the idle reaper, restoring live sessions after a restart (`restore.rs`), and the metrics
sampler (`sampler.rs`) behind the desktop app's metrics panel. In the desktop app
(`apps/desktop/src-tauri/src`): remote access (`remote.rs`), connectors, worktree cleanup
(`cleanup.rs`) and the working-copy updater (`working_copy.rs`).

## Crate layout

| Crate | Role |
|---|---|
| `berth` | Thin CLI: `daemon`, `kill`, `add`, `rename`, `worktree`, `enter`, `spawn`, `workspace`, `upgrade`; no args prints help. `client.rs` holds the daemon client: auto-spawn, handshake, kill |
| `berth-core` | Shared protocol, entities, IDs, paths, codec |
| `berth-daemon` | PTYs, SQLite, git, hook receiver, status engine |

## Apps

| Path | Role |
|---|---|
| `apps/desktop/src-tauri` | `berth-desktop` crate: the Tauri shell, a daemon client on `berth-core` (`berth.rs`), connectors, remote access, and the headless `berth-server` binary |
| `apps/web` | Vue UI the desktop window and `berth-server` both serve; Feature-Sliced layers under `src/` (`app` > `pages` > `widgets` > `features` > `entities` > `shared`, enforced by stratum), UI kit and icons only through `shared/ui`, Rust only through `bridge.ts` |
| `packages/ui-kit`, `packages/ui-tokens` | components and design tokens for `apps/web` |
| `docker` | image with `berth` and `berth-server` for a remote machine |

The desktop crate is a workspace member but not a default member: a bare `cargo build` and the
release workflow skip it, and CI excludes it because it needs a webview toolchain.

`vendor/vt100` is a patched fork (DECSTBM scrollback fix — see its Cargo.toml); don't swap it for
crates.io vt100.

## Platform adapters

What differs between Windows, macOS and Linux goes through one adapter per layer:
`berth-core/src/platform/` for what the crates and the desktop share (clipboard, sounds, openers,
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
