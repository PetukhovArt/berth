# Getting started

This walk-through takes you from the installer to an agent working in a worktree of its own, with its
questions waiting for you in the Inbox. Every command is in the [command reference](cli.md) and every
setting in [configuration](configuration.md).

berth has two parts: the `berth` command, which runs the daemon that owns every agent's terminal, and
the desktop app you work in. The desktop installer brings both.

## Install the desktop app

Every GitHub release carries an installer for each platform, all of the same version:

| Platform | File | Where `berth` lands |
|---|---|---|
| Windows x64 | `berth-desktop-x86_64-windows-setup.exe` | beside the app in `%LOCALAPPDATA%\Berth`, which the installer adds to your user `PATH` |
| macOS, Apple silicon | `berth-desktop-aarch64-macos.dmg` | inside the bundle, `Berth.app/Contents/MacOS/berth` |
| macOS, Intel | `berth-desktop-x86_64-macos.dmg` | inside the bundle, `Berth.app/Contents/MacOS/berth` |
| Debian and Ubuntu x64 | `berth-desktop-x86_64-linux.deb` | `/usr/bin/berth` |

On Debian and Ubuntu:

```sh
sudo apt install ./berth-desktop-x86_64-linux.deb
```

The macOS app is not signed with an Apple developer identity, so the first launch asks you to allow it
in System Settings, Privacy & Security. Its `berth` is on the `PATH` of the sessions the app starts but
not of your own shell; link it there if you want it in a terminal:

```sh
ln -s /Applications/Berth.app/Contents/MacOS/berth ~/.local/bin/berth
```

When no daemon is running, the app starts one in the background. An app installed from a release
starts its own bundled `berth` and puts it first on the `PATH` of every session, so agents call the
same version. An app built from source or installed as a working copy has no bundled `berth` and
starts the one on your `PATH`.

### Updates

While `app.autoUpdate` is on (the default), the app checks the latest release a minute after it starts
and every 10 minutes after that. A newer version downloads in the background and its signature is
checked; then `Restart to update` appears in the title bar. Clicking it installs the new version and
restarts the app. Sessions keep running in the daemon meanwhile. `Update Berth…` checks on demand, also
with `app.autoUpdate` off, and shows the release notes. A failed download or a bad signature leaves the
installed version as it is, and the next check tries again.

On Linux the `.deb` install asks for your password. On Windows the installer window shows its progress
and starts the app again when it is done.

## Install only the berth command

On macOS and Linux the install script installs and updates, and `berth upgrade` runs it again later,
pinned to the version the latest release names in `latest.json`, so the command matches the desktop
app. Releases carry no Linux `berth` archive, so on Linux the script builds from source with
`cargo install` (the desktop `.deb` includes `berth`):

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

You need at least one agent CLI on your `PATH`: `claude`, `codex` or `grok`, or the CLI of an ACP agent such as `cursor-agent` or `opencode`.

berth was called pacer up to 0.20. Its first start copies your pacer data over and leaves pacer running;
see [Moving from pacer](configuration.md#moving-from-pacer).

## Build from source

The app builds from this repository with Node 22, pnpm and a Rust toolchain (on Linux also the
webkit2gtk 4.1 and GTK 3 development packages):

```sh
pnpm install
pnpm build:desktop
```

To run your own `main` every day, install a working copy that rebuilds itself from inside the app:
`scripts\install-app.ps1` on Windows ([details](windows.md#desktop-working-copy)) and
`scripts/install-app.sh` on macOS ([details](macos.md)). A working copy does not read releases.

## 1. Check the Welcome page

The first start opens the Welcome page. It looks at this machine and lists what it found: projects from
your Claude Code and Codex history, the agent CLIs that are installed and signed in, the GitHub and
GitLab hosts you are signed in to, Jira and Confluence, and remote access for a phone. Tick what you
want and press `Start`. If you skip it, the empty Projects tree has `Open the Welcome page` to bring it
back.

## 2. Add a project

berth is project-first, and a project is just a git checkout. Use `Add Project` in the app, or the
command line:

```sh
berth add ~/code/my-app       # or, from inside the repo: berth add .
```

Projects sit in workspaces, named groups of projects shown in the tree. Every install has a
`default` workspace.

## 3. Start a session in a worktree

`Ctrl+Shift+N` (`Cmd+Shift+N` on macOS) opens `New Session`. Pick the project, then where to run: the
main checkout, an existing worktree, or `New worktree…` with a new branch. Pick the type, and for an
agent the model, the effort and an optional first prompt:

- Claude and Grok open in the Chat view. `Claude (terminal)` and `Grok (terminal)` open in a terminal.
- Codex opens in a terminal.
- An ACP agent such as Cursor, OpenCode, Gemini or Kiro runs in the Chat view once you turn it on in
  Settings → Providers. It has no terminal.
- A plain terminal runs a shell in the worktree.

`Ctrl+Space` then `C` or `T` switches a session between its Chat view and its terminal.

A new worktree is a real `git worktree`, created under `<repo>/../<repo-name>-worktrees/<branch>`.
That is the point of it: two agents in two worktrees edit two directories and never collide.
`sessions.worktreeBase` in settings picks the branch a new worktree starts from, here and for `berth worktree`.

You can also just ask: tell an agent "do this in a worktree" and it runs `berth worktree`, which
creates one and moves the session into it. Asked to work in another repo, it runs `berth enter`.

A new worktree also gets what git leaves behind: the gitignored files sitting in the main checkout
(`.env`, `.claude/settings.local.json`) are copied in, and gitignored symlinks (a linked skill, a
shared config) are recreated pointing where they pointed. Ignored directories are not: `node_modules`,
`target` and the like are yours to build, and a link into one is followed only if you linked it
yourself. A file the branch tracks is never overwritten by the main checkout's ignored copy.

## 4. Answer from the Inbox

Give the agent a task that needs a permission, then switch to something else. The Inbox at the top of
the sidebar lists every session that needs you:

- waiting for you: a permission prompt or a question, with `Allow` and `Deny` (and `Always` when the
  agent offers it) on the entry itself;
- finished and not seen yet;
- crashed, with `Resume` to start the agent again and continue its conversation;
- stopped at a usage limit, with the time it resets when the agent reports one.

`Alt+J` opens the next session that needs you. An entry's menu snoozes it for an hour or until
tomorrow, or settles it: a settled entry stays hidden until the session changes state again. The Inbox
also lists the team channels with unread messages when you use a [team hub](team-hub.md).

Outside the Inbox, each agent's row in the tree shows its status (the statuses are defined in the
[glossary](../GLOSSARY.md#statuses)), and worktree and project rows roll up their children, so a
project shows when something under it needs you. `ui.statusAnimation` picks how the tree and the tabs
animate a status.

Status comes from agent CLI hooks, not MCP: berth merges managed hooks into each agent's config and they
report to the daemon's loopback endpoint. Codex hooks live in `~/.codex/hooks.json`; approve them once
at codex's "Hooks need review" prompt.

## 5. Leave, and it keeps running

The daemon owns every terminal, so closing the app stops nothing. Open it again and each session is
where you left it, scrollback replayed. When the daemon itself restarts, it starts the sessions that
were running again (`restore_sessions` in [configuration](configuration.md)) and agents resume their
conversation.

Idle terminals nobody is looking at are released after `session_idle_timeout` and reconnect when you
open the tab again.

## 6. Let them name themselves

Leave a new session on its default name and the agent retitles it after your first prompt. A name you
type yourself is never touched. `/rename` inside Claude Code or Codex renames the row too.

## 7. Go back with a checkpoint

berth saves the files of an agent's worktree at each prompt, on hidden git refs that never touch the
branch. The `Checkpoints` button of a session lists them by prompt, and `Restore files` puts the
worktree back as it was at that prompt after asking; what is there now is saved as a new checkpoint
first. Every agent takes checkpoints except Claude in a terminal, which does not by default
because Claude Code has its own `/rewind`; `daemon.checkpointsClaude` turns them on for it.

## Find things

`Ctrl+P` opens Quick Open: it searches sessions, merge requests, tasks, Markdown files of the current
worktree, actions, channel messages and the text of conversations, and `Ctrl+Enter` on its
`All results` row opens the Search view on the same query. `Ctrl+Shift+F` opens the Search view
directly: it also finds archived sessions and the full text of Claude, Codex and Grok conversations,
and opening a match scrolls the Chat view to that message. `Ctrl+K` focuses the filter above the
sidebar, which filters the Inbox and the tree.

## Moving around

| Key | Action |
|---|---|
| `Ctrl+P` | Quick Open, outside a terminal; `Ctrl+Alt+P` works everywhere |
| `Ctrl+Shift+F` | Search view |
| `Ctrl+Shift+P` or `F1` | command palette |
| `Alt+J` | next session that needs you |
| `Alt+1` to `Alt+9` | open the Nth live session |
| `Ctrl+Tab` | switch between recent live sessions |
| `Ctrl+Space` then `C`, `T`, `M` or `D` | switch a session between its Chat view, terminal, merge request and diff |
| `Ctrl+K` | focus the sidebar filter |
| `Ctrl+B` | show or hide the sidebar |
| `Ctrl+,` | settings |

`Alt+1` to `Alt+9` and `Ctrl+Tab` work while `workbench.hot` is not `off`.

On macOS `Cmd` takes the place of `Ctrl` in the bindings Settings > Keys writes as `mod`, such as
`Cmd+P` and `Cmd+Shift+N`. `Ctrl+Tab`, the `Ctrl+Space` chord and `Alt+1` to `Alt+9` (`Option` on a Mac
keyboard) keep their keys. Every key is listed and can be changed in Settings > Keys.

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

## With a team

A team hub lets people and their agents share channels, mention each other's agents and answer an
agent's request from the channel. See [team hub](team-hub.md).

## On a phone

The desktop app can serve its web UI to a phone. See
[remote access over Tailscale](remote-access-tailscale.md).
