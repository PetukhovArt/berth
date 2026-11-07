# macOS

The terminal UI and the daemon install on macOS the usual ways (npm, `install.sh`, `cargo install`),
see [getting started](getting-started.md). This page covers the desktop app.

## Desktop working copy

The desktop app you use all day should not be the one agents are rebuilding. Install a working copy: a
release build of the committed `main` under `~/Library/Application Support/Berth`, launched like any
other app and updated only when you ask.

You need Xcode Command Line Tools (`xcode-select --install`), Node 22, pnpm and a Rust toolchain. From
the checkout:

```sh
bash scripts/install-app.sh -Launch
```

The script reads `main` from the checkout it sits in, never the files in the working tree, and builds it
in `~/Library/Application Support/Berth/build` (`src` for the exported tree, `target` for an incremental
cargo cache, `logs` for one log per build). It builds the web UI and `Berth.app`, signs the bundle, and
puts it with the matching `berth` binary in `app/<build-id>`. `app/current` points at it, `app/previous` at
the build before, and `~/Applications/Berth.app` is a link to `app/current/Berth.app`, so opening it from
Finder starts the current build. The working copy uses `~/.berth`, the macOS keychain and your real daemon.

The app connects to a running daemon and does not start one. Run `berth` once (it starts the daemon in
the background), or `berth daemon` in a terminal you keep open.

## Window

The window keeps the macOS traffic lights over Berth's own title bar, and the menu bar has the usual
`Berth`, `Edit` and `Window` menus: `Cmd+,` opens settings, `Cmd+C` and `Cmd+V` work in every text field,
and `Cmd+Q` quits. With `window.closeToTray` on, closing the window hides it; the menu bar icon or a click
on the Dock icon brings it back.

## Update and rollback

`Update Berth…` (menu bar icon, the `Berth` menu and the command palette) rebuilds in the background: it
shows the commit it builds, the current step and the last log lines, and `Cancel build` stops it. A
failed build changes nothing and shows its log path. A finished one offers `Restart now` or `Later`;
sessions live in the daemon, so a restart keeps them. `Roll back to previous build` swaps `current` and
`previous`. `app.sourceRepo` in settings points the update at another checkout.

The update never touches the daemon. When the daemon runs another build than the installed one, the
dialog offers `Update daemon`: it puts that build's `berth` into `~/.cargo/bin`, stops the daemon and
starts the new one, which ends every live terminal. The daemon's build is recorded in `daemon/build.json`
under the install root.

The same from a terminal:

```sh
bash ~/Library/Application\ Support/Berth/app/current/install-app.sh -Rollback -Launch
bash ~/Library/Application\ Support/Berth/app/current/install-app.sh -UpdateDaemon
```

Every location can be moved for a test: `BERTH_APP_ROOT` (install root), `BERTH_APPLICATIONS_DIR`
(where the `Berth.app` link goes), `BERTH_BIN_DIR` (where `-UpdateDaemon` puts `berth`), `BERTH_HOME`,
`BERTH_RUNTIME_DIR` and `BERTH_DATA_DIR`.

## Signing

The app is signed ad hoc (`codesign --sign -`), with no Developer ID and no notarization. A build you
make yourself carries no quarantine flag, so Gatekeeper lets it start. An ad hoc signature changes with
every build, though, so the keychain and the firewall can ask again after an update. To stop that,
create a code signing certificate in Keychain Access (Certificate Assistant, Create a Certificate, type
Code Signing) and write its name to `signing-identity` in the install root:

```sh
echo "Berth Local" > ~/Library/Application\ Support/Berth/signing-identity
```

Every build after that, from the terminal or from `Update Berth…`, signs with it.
`BERTH_CODESIGN_IDENTITY` overrides the file for one run.

A pinned Dock icon can keep pointing at the build it was pinned from, and that build is deleted two
updates later. If the icon stops opening Berth, pin it again.

## Development

The dev app (`pnpm dev:desktop`) runs apart from the working copy: the Tauri identifier
`dev.berth.desktop.dev`, a window titled `Berth (dev)`, `BERTH_HOME` under `~/.berth-dev`, file secrets
and a dev daemon. `make dev` gives the TUI its own instance the same way.
