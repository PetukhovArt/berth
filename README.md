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

Download the desktop app from the [latest release](https://github.com/PetukhovArt/berth/releases/latest):

- Windows: `berth-desktop-x86_64-windows-setup.exe`
- macOS: `berth-desktop-aarch64-macos.dmg` (Apple silicon) or `berth-desktop-x86_64-macos.dmg` (Intel)
- Debian and Ubuntu: `berth-desktop-x86_64-linux.deb`

The app brings the `berth` command with it, starts the daemon when none is running and updates itself
from new releases ([details](docs/getting-started.md#install-the-desktop-app)).

You need at least one agent CLI on your `PATH`: `claude`, `codex`, `cursor-agent`, `opencode` or `grok`.

To install only the `berth` command, on macOS and Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/PetukhovArt/berth/main/install.sh | sh
```

On any platform, with a Rust toolchain:

```sh
cargo install --git https://github.com/PetukhovArt/berth berth --locked
```

To build the app from this repository, or to run a copy that rebuilds your local `main`, see
[Build from source](docs/getting-started.md#build-from-source).

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
