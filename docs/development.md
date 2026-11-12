# Development

Everything below is the unix workflow. On Windows there is no `make`; the equivalents are in
[windows.md](windows.md#development-loop).

```sh
make help                 # every target
make check                # typecheck, the fastest feedback
make ci                   # the whole gate: fmt check, clippy, tests
cargo build --release     # → target/release/berth
cargo test                # unit + end-to-end suite (spawns real daemons/PTYs)
```

Never run freshly built code against your real daemon. `make dev` builds `berth`, starts a daemon on an
isolated instance (its own socket, database and app settings, one per checkout) and runs the desktop
app against it. `make dev AGENT=/bin/cat` stubs agent spawns. `make dev-seed` copies your real
projects and settings into the instance, `make dev-reset` wipes it, `make dev-ls` lists every
checkout's instance.

## Layout

| Crate | What it owns |
|---|---|
| `berth-core` | shared protocol, entities, paths, transport |
| `berth-daemon` | PTYs, SQLite, hook receiver, status engine |
| `berth` | the binary: the daemon and its subcommands |

The desktop app is `apps/desktop` (the Tauri shell) and `apps/web` (its Vue UI), with shared UI in
`packages/`.

`vendor/vt100` is a patched copy of the terminal parser, wired in through `[patch.crates-io]`; the
daemon's ConPTY unicode probe test rebuilds captured output with it. Don't bump or replace it.

[ARCHITECTURE.md](../ARCHITECTURE.md) is the map of how the pieces fit; [GLOSSARY.md](../GLOSSARY.md) is
the domain vocabulary. Read both before touching IPC, hooks or the daemon.

## Releases

Bump `version` in the root `Cargo.toml` and land it on `main`. CI builds macOS (arm/intel) and Windows
(x64) binaries, tags the commit and attaches them to a GitHub release, which is what `install.sh`
downloads. The version bump is the release, so leave `version` alone in every other change.

The whole runbook is in [releasing.md](releasing.md).
