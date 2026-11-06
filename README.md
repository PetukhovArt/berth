# berth

Run Claude Code, Codex and Cursor agents across your projects and git worktrees, and see at a glance which
one needs you. A daemon owns the terminals, so the agents keep working after you close the window.

[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey?style=flat-square)](docs/windows.md)

berth has two interfaces over the same daemon:

- a desktop app (`apps/desktop`) with a project tree, docked terminals, merge request reviews, Jira tasks
  and worktree cleanup;
- a terminal UI (`berth`) that also works over ssh and in a browser tab.

## Install

```sh
npm install -g @petukhovart/berth
```

Other routes (install script, `cargo install`) are in [getting started](docs/getting-started.md). You need
at least one agent CLI on your `PATH`: `claude`, `codex` or `cursor-agent`. `gh` or `glab` turn on pull
requests.

The desktop app builds from this repository and needs Node 22, pnpm and a Rust toolchain:

```sh
pnpm install
pnpm build:desktop
```

It connects to a running daemon, so start `berth daemon` first. On Windows, `scripts\install-app.ps1`
installs a copy that updates itself from your local `main` ([details](docs/windows.md#desktop-working-copy)).

## Quickstart

```sh
berth add ~/code/my-app
berth
```

`?` lists every key in the terminal UI.

## Docs

- [Getting started](docs/getting-started.md)
- [Keymap](docs/keymap.md)
- [Commands](docs/cli.md)
- [Configuration](docs/configuration.md)
- [Windows](docs/windows.md)
- [Remote access over Tailscale](docs/remote-access-tailscale.md)
- [Changelog](CHANGELOG.md)

## License

MIT
