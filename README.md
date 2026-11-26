# berth

Berth is a self-hosted agent platform: it runs coding agents from any vendor for you and your team, on your own machines.
It works for a solo developer on one machine too.

[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey?style=flat-square)](docs/windows.md)

Claude Code, Codex and Grok run natively; Cursor, OpenCode, Gemini and other ACP agents run in the
chat view. A daemon owns them, so they keep working after you close the window. You work in the
desktop app: a project tree with each agent's status, an Inbox of what waits on you, merge request
reviews and Jira tasks. A team adds a hub it runs itself, where people and their agents share
channels and roles. A phone opens the same UI in its browser.

Where Berth is going is in [VISION.md](VISION.md).

## Install

Download the desktop app from the [latest release](https://github.com/PetukhovArt/berth/releases/latest):

- Windows: `berth-desktop-x86_64-windows-setup.exe`
- macOS: `berth-desktop-aarch64-macos.dmg` (Apple silicon) or `berth-desktop-x86_64-macos.dmg` (Intel)
- Debian and Ubuntu: `berth-desktop-x86_64-linux.deb`

The app brings the `berth` command with it, starts the daemon when none is running and updates itself
from new releases ([details](docs/getting-started.md#install-the-desktop-app)).

You need at least one agent CLI on your `PATH`: `claude`, `codex` or `grok`, or the CLI of an ACP agent such as `cursor-agent` or `opencode`.

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

- An Inbox of the agents that wait on you, with Allow, Deny and the agent's options on the entry itself.
- Sessions in git worktrees, created for you or by the agent itself when you ask it to work in one.
- Sessions survive closing the app, and come back after a daemon restart with the conversation resumed.
- A team hub you host: channels per task, `@agent` mentions, an agent's questions answered from the channel, roles and an audit log.
- Merge and pull requests from GitHub and GitLab, with changes, pipelines and threads you can answer, and agents that react when a watched pipeline fails.
- Remote access from a phone, best over Tailscale.

## Docs

- [Getting started](docs/getting-started.md)
- [Team hub](docs/team-hub.md)
- [Remote access and the phone](docs/remote-access-tailscale.md)
- [Commands](docs/cli.md)
- [Configuration](docs/configuration.md)
- [Windows](docs/windows.md)
- [macOS desktop app](docs/macos.md)
- [Development](docs/development.md)
- [Releasing](docs/releasing.md)
- [Issues](docs/issues.md)
- [Vision](VISION.md) and [architecture decisions](docs/adr/README.md)
- [Changelog](CHANGELOG.md)

## License

MIT
