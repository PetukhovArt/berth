# Windows

berth runs natively on Windows 10 and 11. The whole workspace builds, lints and tests there, and the
daemon, the desktop app and agent sessions work the same way they do on macOS. This page covers what
is different.

## Install

Releases carry `berth-desktop-x86_64-windows-setup.exe`. It installs the desktop app for your user into
`%LOCALAPPDATA%\Berth` without asking for admin rights, puts `berth.exe` beside it and adds that folder to
your user `PATH`. The app updates itself from later releases
([details](getting-started.md#updates)).

For the command alone, releases carry a prebuilt `berth-x86_64-pc-windows-msvc.zip`: unzip it and put
`berth.exe` anywhere on your `PATH`.

From source:

```powershell
cargo install --git https://github.com/PetukhovArt/berth berth --locked
```

That drops `berth.exe` into `~\.cargo\bin`. From a checkout, `cargo install --path crates/berth` does the
same.

`berth upgrade` downloads the zip of the version the latest release names and swaps the running
`berth.exe` for it. The old file stays as `berth.old.exe` until nothing runs it, because Windows can
rename a running exe but not overwrite it. A daemon with no live sessions is stopped so the next launch
starts the new binary; with live sessions it keeps running until you run `berth kill`.

To build the desktop app from your own `main`, see [Desktop working copy](#desktop-working-copy).

You also need:

- A recent Rust toolchain: <https://rustup.rs>.
- At least one agent CLI on `PATH`: `claude`, `codex` or `grok`, or the CLI of an ACP agent such as `cursor-agent` or `opencode`. CLIs installed through npm
  (`.cmd` shims) are resolved and launched correctly.
- Git for Windows.
- `gh` and/or `glab`, if you want to connect GitHub or GitLab through their CLI instead of a token.

## What differs under the hood

- Transport: unix keeps an `AF_UNIX` socket in a `0700` runtime dir, where a successful connect is
  already proof of identity. Windows has no such socket, so the daemon binds a loopback TCP listener and
  a client presents a bearer token, the same model the hook receiver already uses on every platform.
  Port and token live in an endpoint file beside the pidfile, inside the per-user `%TEMP%\berth`, which
  the profile's ACL already closes to other unprivileged users.
- Daemon lifetime: the daemon runs windowless in the background and survives its clients closing. A
  second daemon won't start over a live one, and `berth kill` stops it cleanly.
- Process trees: closing a session kills the agent's whole process tree, so nothing is left running in
  the background, and memory metrics are read through the Win32 APIs rather than parsed from `ps`.
- No flashing windows: background `git` / `gh` invocations are spawned with `CREATE_NO_WINDOW`.
- Paths: `~/` resolves through `USERPROFILE` when no `HOME` is set, and canonical paths are handed
  out in ordinary `C:\…` form rather than the `\\?\C:\…` verbatim form git refuses.

## Development loop

There is no `make` on Windows, and installing one would not help much: the Makefile's dev and install
targets are unix-only throughout (`shasum`, `/tmp`, `ps`, `kill`, no `.exe` suffix). Call cargo directly
instead:

```powershell
cargo check --workspace --all-targets    # make check, the fastest feedback
cargo clippy --workspace --all-targets   # make lint
cargo fmt --all -- --check               # make ci, first step
cargo test --workspace                   # make test
```

Never run a fresh build against your real daemon. `scripts\dev.ps1` is the Windows counterpart of
`make dev`: it builds, keys a runtime dir and a database to this checkout, stops the dev daemon left
over from the previous run, starts a dev daemon from the fresh binary and runs the desktop app against
it. When the app closes, the script stops that daemon.

```powershell
powershell -ExecutionPolicy Bypass -File scripts\dev.ps1
powershell -ExecutionPolicy Bypass -File scripts\dev.ps1 -Agent cmd.exe   # stub agents out
powershell -ExecutionPolicy Bypass -File scripts\dev.ps1 -Reset           # wipe the dev database
```

Run it in a window of its own, outside berth. The loop is edit, close the dev app, run the script
again. Stopping the old daemon is the load-bearing part: it detaches and outlives the app, so a rebuild
that only relaunches the app leaves daemon-side changes running the previous code.

The script sets what you would otherwise export by hand, then runs `berth daemon` and
`pnpm dev:desktop`:

```powershell
$env:BERTH_RUNTIME_DIR = "$env:TEMP\berth-dev"
$env:BERTH_DATA_DIR    = "$env:USERPROFILE\.berth-dev\main"
$env:BERTH_HOME        = "$env:USERPROFILE\.berth-dev\main\home"
$env:BERTH_SECRETS     = 'file'
$env:BERTH_AGENT_CMD   = 'cmd.exe'   # optional: stub agents, spawn no real claude
```

Give each checkout its own pair of paths: two checkouts sharing a runtime dir means the second app
silently drives the first one's daemon. `berth kill` with the same variables set stops that dev daemon;
deleting the two directories resets the instance.

`BERTH_AGENT_CMD` is one program name, taken verbatim with no arguments, so the unix `make dev
AGENT=/bin/cat` idiom has no direct translation. Name a harmless console program instead.

To install a build for real use and cut the live daemon over to it:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\update.ps1
```

It builds `--release`, moves the live `berth.exe` aside (Windows locks a running executable, but renaming
it is allowed), copies the fresh build into `~\.cargo\bin`, and kills the daemon so the next launch runs
the new code. Run it from a terminal outside berth: the cutover stops every session. A build failure
stops before anything is touched.

## Desktop working copy

The desktop app you use all day should not be the one agents are rebuilding. Install a working copy: a
release build of the committed `main` under `%LOCALAPPDATA%\Berth`, launched from the Start Menu like any
other app, and updated only when you ask.

```powershell
powershell -ExecutionPolicy Bypass -File scripts\install-app.ps1 -Launch
```

The script reads `main` from the checkout it sits in, never the files in the working tree, and builds it
in `%LOCALAPPDATA%\Berth\build` (`src` for the exported tree, `target` for an incremental cargo cache,
`logs` for one log per build). The result goes to `app\<build-id>`, `app\current` points at it, `app\previous`
at the build before, and the Start Menu gets a `Berth` shortcut to `app\current\berth-desktop.exe`. The
working copy uses `~\.berth`, the OS keychain and your real daemon.

### Speech engine

The app builds without anything beyond what the `berth` command needs. The local speech engine (the
`voice` cargo feature, off by default in a working copy; the release installers have it) is optional:
without it, voice input uses the speech recognition of the app's webview ([Voice](configuration.md#voice)
describes what the engine does). The engine compiles whisper.cpp and generates its bindings with
bindgen, so on Windows it needs two more tools:

- CMake on `PATH` (`winget install Kitware.CMake`). The copy inside Visual Studio works too when it is on
  `PATH`, but a CMake older than 3.21 does not know the Visual Studio 2022 generator: with such a CMake
  and Build Tools 2019, set `CMAKE_GENERATOR` to `Visual Studio 16 2019`.
- LLVM for `libclang` (`winget install LLVM.LLVM`). The build finds it in `C:\Program Files\LLVM` or in
  the "C++ Clang tools" component of Visual Studio on its own; anywhere else, set `LIBCLANG_PATH` to the
  folder that holds `libclang.dll`. A bare `libclang.dll` without the clang headers (the PyPI `libclang`
  wheel, for one) also needs `BINDGEN_EXTRA_CLANG_ARGS` with `-isystem` for the MSVC and Windows SDK
  include folders.

The install script looks for both before it builds. When it finds them it builds with `--features voice`;
when it does not, it builds without the engine and prints one line saying what to install.
`install-app.ps1 -CheckVoice` prints that verdict, with any variables it would set, and exits without
building. By hand, add `--features voice` to `cargo build -p berth-desktop` or to
`pnpm --filter @berth/desktop exec tauri dev`, and set what the script would:

- `CLANG_PATH` to LLVM's `clang.exe` when LLVM's `bin` is not on `PATH` (the winget install leaves it
  off). bindgen asks `clang.exe` for its include folders; without it the bindings stop at `'stdbool.h'
  file not found`.
- When the newest Visual Studio on the machine has no C++ workload (a Visual Studio 2022 IDE beside Build
  Tools 2019, say), CMake and clang both pick it and fail with `could not find any instance of Visual
  Studio` and `'stdio.h' file not found`. Point them at the one that has it: `CMAKE_GENERATOR` to
  `Visual Studio 16 2019` and `VCToolsInstallDir` to its `VC\Tools\MSVC\<version>\` folder.

With the engine, the build downloads the sherpa-onnx libraries from their GitHub releases and puts
`sherpa-onnx-c-api.dll` and `onnxruntime.dll` next to `berth-desktop.exe`, and the install script copies
both into the working copy. The wake word needs them there, while dictation and the rest of the app
start without them. `berth-desktop.exe` then also needs `MSVCP140.dll` from the Visual C++ runtime, which
Visual Studio Build Tools installs.

### Update and rollback

This is how a working copy updates, on Windows and on macOS. Inside the app, `Update Berth…` (the tray
or menu bar icon, the command palette, and the `Berth` menu on macOS) rebuilds `main` in the background
the way the install script does: it shows the
commit it builds, the current step and the last log lines, and `Cancel` stops it. A failed build changes
nothing and shows its log path. A finished one offers `Restart now` or `Later`; sessions live in the
daemon, so a restart keeps them. `Roll back to previous build` swaps `current` and `previous`.
`app.sourceRepo` in settings points the update at another checkout.

The update never touches the daemon. When the daemon runs another build than the installed one, the
dialog says so and offers `Update daemon`: it installs that build's `berth` into `~/.cargo/bin`
(`berth.exe` into `~\.cargo\bin` on Windows, the way `scripts\update.ps1` does) and restarts the daemon,
which ends every live terminal. The daemon's build is recorded in `daemon/build.json` under the install
root, so a daemon started any other way (from `scripts\update.ps1` or by a `berth` call) counts as not
from this build. When the protocols differ, the app cannot talk to the daemon until it is updated.
`-UpdateDaemon` and `-Rollback` of the install script do the same from a terminal.

### Test locations

Every location of the Windows working copy can be moved for a test: `BERTH_APP_ROOT` (install root), `BERTH_START_MENU` (shortcut
folder), `BERTH_BIN_DIR` (where `-UpdateDaemon` puts `berth.exe`), `BERTH_HOME`,
`BERTH_RUNTIME_DIR`, `BERTH_DATA_DIR`, and `WEBVIEW2_USER_DATA_FOLDER` for the webview profile.

The dev run (`scripts\dev.ps1` or `pnpm dev:desktop`) stays apart from it: the Tauri identifier
`dev.berth.desktop.dev`, a window titled `Berth (dev)`, `BERTH_HOME` under `~\.berth-dev`, file secrets,
a dev daemon and WebView2 debugging on port 9334. The installed app keeps WebView2 debugging on port
9335, so `agent-view` can attach to it as well; `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` overrides it.

The PTY e2e suite is marked as requiring a Unix host and is skipped on Windows; the rest of the suite,
including the Windows transport, token and daemon-lock smoke tests, runs.
