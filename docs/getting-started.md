# Getting started

berth has two parts: the `berth` command, which runs the daemon that owns every agent's terminal, and
the desktop app you work in. Install the command first, then build the app.

## Install the berth command

On macOS and Linux the install script installs and updates, and `berth upgrade` runs it again later.
Releases carry no Linux binaries, so on Linux it builds from source with `cargo install`:

```sh
curl -fsSL https://raw.githubusercontent.com/PetukhovArt/berth/main/install.sh | sh
```

It puts the prebuilt binary from the latest GitHub release into `~/.local/bin` (`BERTH_INSTALL_DIR`
overrides it) and falls back to `cargo install --git` when no release matches. From source, on any
platform, with a Rust toolchain:

```sh
cargo install --git https://github.com/PetukhovArt/berth berth --locked
```

Windows has its own notes in [windows.md](windows.md).

You need at least one agent CLI on your `PATH`: `claude`, `codex`, `cursor-agent`, `opencode` or `grok`.

berth was called pacer up to 0.20. Its first start copies your pacer data over and leaves pacer running;
see [Moving from pacer](configuration.md#moving-from-pacer).

## Build the desktop app

The app builds from this repository and needs Node 22, pnpm and a Rust toolchain:

```sh
pnpm install
pnpm build:desktop
```

For everyday use, install a working copy that you update from inside the app: `scripts\install-app.ps1`
on Windows ([details](windows.md#desktop-working-copy)) and `scripts/install-app.sh` on macOS
([details](macos.md)).

The app talks to the daemon and needs `berth` on your `PATH`: on Windows and macOS it starts
`berth daemon` from there when none is running. On Linux, start it yourself with `berth daemon`.

## 1. Add a project

berth is project-first, and a project is just a git checkout. Use `Add Project` in the app, or the
command line:

```sh
berth add ~/code/my-app       # or, from inside the repo: berth add .
```

Projects sit in workspaces, named groups of projects shown in the tree. Every install has a
`default` workspace.

## 2. Start a session

`Ctrl+Shift+N` (`Cmd+Shift+N` on macOS) opens `New Session`. Pick the project, then where to run: the
main checkout, an existing worktree, or `New worktree…` with a new branch. Pick the type (Claude, Codex,
Cursor, OpenCode, Grok or a plain terminal), and for an agent the model, the effort and an optional first prompt.

A new worktree is a real `git worktree`, created under `<repo>/../<repo-name>-worktrees/<branch>`.
That is the point of it: two agents in two worktrees edit two directories and never collide.
`sessions.worktreeBase` in settings picks the branch a new worktree starts from.

You can also just ask: tell an agent "do this in a worktree" and it runs `berth worktree`, which
creates one and moves the session into it. Asked to work in another repo, it runs `berth enter`.

A new worktree also gets what git leaves behind: the gitignored files sitting in the main checkout
(`.env`, `.claude/settings.local.json`) are copied in, and gitignored symlinks (a linked skill, a
shared config) are recreated pointing where they pointed. Ignored directories are not: `node_modules`,
`target` and the like are yours to build, and a link into one is followed only if you linked it
yourself. A file the branch tracks is never overwritten by the main checkout's ignored copy.

## 3. Leave, and it keeps running

The daemon owns every terminal, so closing the app stops nothing. Open it again and each session is
where you left it, scrollback replayed. When the daemon itself restarts, it starts the sessions that
were running again (`restore_sessions` in [configuration](configuration.md)) and agents resume their
conversation.

Idle terminals nobody is looking at are released after `session_idle_timeout` (5m by default) and
reconnect when you open the tab again.

## 4. Read the status instead of the screens

Once you run more than one agent you stop reading terminals and start reading the tree. Each agent has
a status:

| Status | Meaning |
|---|---|
| Fresh | the agent has not run a turn yet |
| Running | a turn is in progress (a turn with live subagents still counts as running) |
| Finished | the turn is complete; until you look at it, it is also marked unseen |
| Needs Feedback | a permission prompt or a question is waiting on you |
| Terminated | the process died mid-run |
| Disconnected | the daemon restarted while the agent was live |

Worktree and project rows roll up their children, so a project shows when something under it needs
you. `ui.statusAnimation` picks how the tree and the tabs animate a status.

Status comes from agent CLI hooks, not MCP: berth merges managed hooks into each agent's config and they
report to the daemon's loopback endpoint. Codex hooks live in `~/.codex/hooks.json`; approve them once
at codex's "Hooks need review" prompt.

## 5. Let them name themselves

Leave a new session on its default name and the agent retitles it after your first prompt. A name you
type yourself is never touched. `/rename` inside Claude Code or Codex renames the row too.

## Moving around

| Key | Action |
|---|---|
| `Ctrl+P` | quick open: jump to a row of the tree or a Markdown file of the current worktree |
| `Ctrl+Shift+P` or `F1` | command palette |
| `Ctrl+Shift+O` | overview of live sessions |
| `Ctrl+Space` then `T`, `M` or `D` | switch a session tab between its terminal, merge request and diff |
| `Ctrl+B` | show or hide the sidebar |
| `Ctrl+,` | settings |

On macOS `Cmd` takes the place of `Ctrl`. Every key is listed and can be changed in Settings > Keys.

## Merge requests, Jira and Confluence

Connect GitHub or GitLab in Settings > Connectors, through `gh` or `glab` or with a token. The app then shows
merge and pull requests in the tree and on the Reviews page (`Ctrl+Shift+R`), with their changes,
pipeline and discussions. You can reply to threads and resolve them, open a request in the browser, or
use `Open in session` to check its branch out in a worktree and start an agent on it.

Jira (`Ctrl+Shift+J`) and Confluence connect the same way.

## Conversations and worktrees

Claude Code and Codex conversations started outside berth show up in the tree beside the sessions of
the worktree they ran in. `Archive` hides one until it gets new messages. A project's
`Old Conversations` lists the conversations of deleted worktrees and resumes one in a worktree that
still exists.

`Clean up worktrees` lists worktrees whose request was merged or closed, whose branch is gone on the
remote or that went stale, and removes the ones you tick.

## On a phone

The desktop app can serve its web UI to a phone. See
[remote access over Tailscale](remote-access-tailscale.md).
