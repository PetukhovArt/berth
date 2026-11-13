# berth

Run Claude Code, Codex, Cursor, OpenCode and Grok agents across your projects and git worktrees, and see at a glance which
one needs you. A daemon owns the terminals, so the agents keep working after you close the window.

[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey?style=flat-square)](docs/windows.md)

You work in the desktop app (`apps/desktop`): a project tree with each agent's status, docked
terminals, merge request reviews, Jira tasks and worktree cleanup. With remote access turned on, a
phone opens the same UI in its browser. The `berth` command runs the daemon and the commands agents
use to title themselves, move into a worktree or start another session.

## Install

Install the `berth` command. On macOS and Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/PetukhovArt/berth/main/install.sh | sh
```

On any platform, with a Rust toolchain:

```sh
cargo install --git https://github.com/PetukhovArt/berth berth --locked
```

You need at least one agent CLI on your `PATH`: `claude`, `codex`, `cursor-agent`, `opencode` or `grok`.

Then build the desktop app from this repository, with Node 22, pnpm and a Rust toolchain:

```sh
pnpm install
pnpm build:desktop
```

On Windows, `scripts\install-app.ps1` installs a copy that updates itself from your local `main`
([details](docs/windows.md#desktop-working-copy)), and on macOS `scripts/install-app.sh` does the same
([details](docs/macos.md)). On both, the app starts `berth daemon` from your `PATH` when none is
running; on Linux, run `berth daemon` yourself.

## Quickstart

```sh
berth add ~/code/my-app
```

Open the desktop app and press `Ctrl+Shift+N` (`Cmd+Shift+N` on macOS) to start a session in that
project, in its main checkout or a new worktree.

## Key features

- Every agent's status in the tree: running, finished and not yet read, or waiting on you.
- Sessions in git worktrees, created for you or by the agent itself when you ask it to work in one.
- Sessions survive closing the app, and come back after a daemon restart with the conversation resumed.
- Merge and pull requests from GitHub and GitLab, with changes, pipelines and threads you can answer.
- Remote access from a phone, best over Tailscale.

## Docs

- [Getting started](docs/getting-started.md)
- [Commands](docs/cli.md)
- [Configuration](docs/configuration.md)
- [Windows](docs/windows.md)
- [macOS desktop app](docs/macos.md)
- [Remote access over Tailscale](docs/remote-access-tailscale.md)
- [Issues](docs/issues.md)
- [Changelog](CHANGELOG.md)

## License

MIT
