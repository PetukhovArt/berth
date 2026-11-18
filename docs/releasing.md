# Releasing

Two channels:

| Channel | Who it serves | Built by |
|---|---|---|
| GitHub release | `install.sh`, `berth upgrade`, manual downloads | `.github/workflows/release.yml`, on a version bump |
| `cargo install --git` | source builds, Windows, fallback when no asset matches | nothing to do |

crates.io is not a channel: the workspace pulls a patched `vt100` through `[patch.crates-io]`, and
`cargo publish` refuses a patched dependency. `cargo install --git` is unaffected: it builds the
workspace as it stands, vendored fork and all.

The desktop app is released for Linux only, as a `.deb`. On macOS and Windows it is built from a checkout,
see [getting started](getting-started.md).

## Cutting a release

A release is a `version` bump in the root `Cargo.toml` (`[workspace.package]`) landing on `main`.
The `Weekly release` workflow (`.github/workflows/release-weekly.yml`) makes that bump every Monday
at 06:00 UTC:

- With nothing under `## Unreleased` in `CHANGELOG.md` there is no release that week.
- With CI on the head of `main` not green the week is skipped with a failed run.
- Otherwise it picks the version: the minor goes up when the section has an `Added`, `Changed` or
  `Removed` line, the patch when it holds fixes only. It bumps `Cargo.toml` and `Cargo.lock`, turns
  `## Unreleased` into `## <version> (<date>)` under a new empty `## Unreleased`, opens the pull
  request `chore: release <version>` with the section as its body, and fast-forwards `main` to it.

`main` takes changes only through a pull request its owner approved. The workflow gets past that
rule with the `RELEASE_DEPLOY_KEY` secret, a deploy key with write access that the `protect main`
ruleset lets through. A push made with it starts the `Release` workflow, which a push made with
`GITHUB_TOKEN` would not.

`gh workflow run release-weekly.yml` releases now. A bump by hand still works the same way, so do not
bump `version` as part of unrelated work.

The `Release` workflow does the rest in one run:

1. `gate` reads the version and asks whether `v<version>` is already tagged. On every other push to
   `main` it is, and the run stops there: a push that changes no version costs one 30-second job.
2. `build` compiles three targets, macOS arm64/x64 and Windows x64, and uploads the archive
   `install.sh` downloads for each. `desktop-linux` builds the desktop app on Ubuntu 24.04 and packs it with
   `berth` into `berth-desktop-x86_64-linux.deb` (`apps/desktop/src-tauri/tauri.linux.conf.json`).
3. `release` tags the commit and attaches `berth-<target>.tar.gz` (and `.zip` for Windows, and the `.deb`) to a GitHub
   release whose notes are this version's `CHANGELOG.md` section.

The tag is the record of what shipped, not the trigger. It has to be, because a tag pushed with
`GITHUB_TOKEN` starts no workflow (GitHub's guard against a workflow triggering itself). Tagging by
hand still works and re-releases that exact commit; a tag whose version disagrees with `Cargo.toml`
fails the gate before anything is built.

To check a cross-compile without releasing, run the workflow from the Actions tab
(`workflow_dispatch`); it builds the matrix and skips the release job.

`install.sh` downloads those assets, so the repository has to be public for the one-liner to work. While
it is private, the script falls back to `cargo install --git`, which needs the user's git credentials.

## After a release

Upgrading while a daemon is running is safe: sessions keep running on the old binary until someone runs
`berth kill`. `berth upgrade` says so, and refuses to replace a local `cargo build` unless passed
`--force`.
