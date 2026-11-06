---
paths:
  - "apps/**"
  - "packages/**"
  - "docker/**"
---

# Desktop app (apps/, packages/, docker/)

The Tauri desktop app, its Vue UI and the ui packages came over from Veyra; `apps/desktop/src-tauri/src/legacy.rs`
holds every old name it still reads (`~/.veyra`, the `veyra` keychain service, `VEYRA_*`, the old Tauri
identifier). Rules there differ from the Rust crates:

- No code comments and no tests in `apps/` and `packages/`. The crates under `crates/` keep their own tests.
- Check a change with `node scripts/checks.mjs <files>` (comments, raw colors, oxlint, knip, types, rustfmt,
  clippy for `berth-desktop`). Never run a full `vue-tsc` or `tsc` over the project. The husky pre-commit runs
  `checks.mjs` on staged files under `apps/`, `packages/` and `docker/` only.
- Colors come from `packages/ui-tokens` through `var(--vr-…)`; the web side reaches Rust only through
  `apps/web/src/bridge.ts`.
- Dev run on Windows, isolated from your real daemon and your real `~/.berth` and keychain:
  `powershell -ExecutionPolicy Bypass -File scripts\dev.ps1 -Desktop` (builds berth, starts a dev daemon on
  this checkout's `BERTH_RUNTIME_DIR`/`BERTH_DATA_DIR`, sets `BERTH_HOME` and `BERTH_SECRETS=file`, runs
  `pnpm dev:desktop` with WebView2 debugging on port 9334, stops the daemon after). Without the script, set the
  same variables and run `berth daemon` and `pnpm dev:desktop` yourself. `apps/desktop/scripts/dev.mjs` runs
  `tauri dev` with `tauri.dev.conf.json` (identifier `dev.berth.desktop.dev`, title `Berth (dev)`) and
  defaults each of those variables to a dev location when it is unset.
- The user's own desktop app is a working copy under `%LOCALAPPDATA%\Berth` (`scripts/install-app.ps1`,
  `apps/desktop/src-tauri/src/working_copy.rs`), built from committed `main` and updated only from its
  `Update Berth…` menu. Never launch, rebuild or kill it from a task; test the updater with `BERTH_APP_ROOT`
  and the other overrides listed in `docs/windows.md`.
