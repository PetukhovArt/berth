# Releasing

Two channels:

| Channel | Who it serves | Built by |
|---|---|---|
| GitHub release | `install.sh`, `berth upgrade`, manual downloads | `.github/workflows/release.yml`, on a version bump |
| `cargo install --git` | source builds, Windows, fallback when no asset matches | nothing to do |

crates.io is not a channel: the workspace pulls a patched `vt100` through `[patch.crates-io]`, and
`cargo publish` refuses a patched dependency. `cargo install --git` is unaffected: it builds the
workspace as it stands, vendored fork and all.

The desktop app is not released. It is built from a checkout, see [getting started](getting-started.md).

## Cutting a release

Bump `version` in the root `Cargo.toml` (`[workspace.package]`), run `cargo check` so `Cargo.lock`
follows, and land it on `main`. That is the whole procedure. The version bump is the release, so do
not bump it as part of unrelated work.

The `Release` workflow does the rest in one run:

1. `gate` reads the version and asks whether `v<version>` is already tagged. On every other push to
   `main` it is, and the run stops there: a push that changes no version costs one 30-second job.
2. `build` compiles three targets, macOS arm64/x64 and Windows x64, and uploads the archive
   `install.sh` downloads for each.
3. `release` tags the commit and attaches `berth-<target>.tar.gz` (and `.zip` for Windows) to a GitHub
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
