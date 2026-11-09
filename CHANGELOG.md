# Changelog

## Unreleased

- Added a desktop app (`apps/desktop`): a window over the berth daemon with a project tree, docked terminals, merge request reviews, Jira tasks and remote access from a phone, which carries over Veyra settings, keychain tokens and layout on first start.
- Added `Update Berth…` and `Roll back to previous build` to the desktop app's tray menu and command palette. Windows: they rebuild a working copy installed with `scripts\install-app.ps1` from the local `main` in the background and switch to it on restart. The same dialog offers `Update daemon` whenever the daemon runs another build.
- Added `Open in session` to merge and pull requests in the desktop app: it checks the source branch out in a worktree (adding a remote for a fork), starts an agent there with a first prompt from the `sessions.mrPrompt` setting and opens it beside the request, or focuses the session already there.
- Added `Clean up worktrees` to the desktop app (tree header, command palette, `Ctrl+P`): it lists worktrees whose merge or pull request was merged or closed, whose branch is gone on the remote, that went stale (`cleanup.staleDays`, 30 by default) or that git no longer knows, and removes the ones you tick after stopping their sessions and unlinking `node_modules` junctions. A daily check puts the count on the button and in the tray.
- Added `Ctrl+Shift+N` in the desktop app to start a new session: pick the project, the worktree or main checkout or `New worktree…` with a new branch off the main checkout's, and keep its branch, pick another local branch or type a new one, which gets a worktree of its own so a checkout with running sessions never switches branches. It opens on the project of the focused tab, a conversation started outside Berth included.
- Added `Archive` to the context menu of a conversation started outside Berth in the desktop app's tree: it moves into the worktree's or folder's `Archive` group, out of `Earlier` and `Ctrl+P`, stays there across restarts and on every connected device, and comes back by itself when the conversation gets new messages or with `Restore`. The transcript file is never touched.
- Added `shift+c` diagnostics card: a floating panel with per-session CPU, RAM and Claude token use, moved by dragging its title row or with `shift+v`, and remembered across restarts.
- Added `model_prices` setting: fill in per-model rates and the diagnostics card shows a cost column.
- Added `]` and `[` to jump to the next and previous session waiting on you, in any workspace.
- Added an experimental `Key combo display` setting (`show_key_combos`) that shows each key you press bottom-left with the action it ran.
- Added a `Draft pull requests` setting under Appearance (`hide_draft_prs`) that hides draft pull requests and GitLab draft merge requests from the PRs panel and `/`.
- Added `Prewarm agents` and `Prewarm sessions` settings under Sessions (`prewarm_agents`, `prewarm_sessions`) to stop berth starting idle processes ahead of time and save memory.
- Added five color themes: `lavender`, `coral`, `slate`, `sand` and `mono`.
- Added renaming a session when you `/rename` it inside Claude Code or Codex, before its first prompt too, in the TUI and every desktop view; whichever rename came last, there or in Berth, wins.
- Added a button above the desktop app's session tree that shows only open sessions: running terminals and agents, and Claude Code or Codex conversations whose process is still running.
- macOS: Added a desktop app working copy built from your local `main` by `scripts/install-app.sh`, so `Update Berth…`, `Roll back to previous build` and `Update daemon` work there too ([details](docs/macos.md)).
- macOS: Added the traffic lights, a menu bar with `Berth`, `Edit` and `Window` menus (`Cmd+,` opens settings, `Cmd+C` and `Cmd+V` work in text fields), a monochrome menu bar icon and reopening a hidden window from the Dock to the desktop app.
- Added a `What is new in Berth` window to the desktop app: after an update and restart it lists once what the new build adds, improves and fixes.
- Windows: Added WebView2 debugging on port 9335 to the installed desktop app, so `agent-view` can attach to it.
- Fixed the selected session in the desktop app's tree drawing two highlights with dim text on top, so its name was hard to read.
- Fixed a terminal running Claude Code showing one name in the desktop app's tree and another in its tab and the TUI: a `/rename` in Claude Code now renames the terminal everywhere, and a rename in Berth shows in the tree too; whichever came last wins.
- Fixed `Continue in Berth` in the desktop app opening the terminal in a new tab beside the conversation; the terminal now takes the conversation's tab.
- Fixed the `Shift+O` session list naming almost every row after its branch, so a session now carries the title Claude Code shows it under in `/resume`.
- Windows: Fixed `Ctrl+Enter` inserting a newline in Claude Code instead of sending the message now (needs a sideloaded `conpty.dll`).
- Fixed agents' `git add` and `git commit` failing with `index.lock: File exists` while berth polls the checkout.
- Fixed a session painting in no color at all when `NO_COLOR` or `FORCE_COLOR` was set in the daemon's environment or a login profile.
- Fixed Claude Code sessions not saving their transcript (`Transcript saving is off`) when the daemon was started from inside another Claude Code session.
- Fixed scrolling a pane up past its oldest line, which then took as many wheel notches to come back down.
- Fixed desktop app tabs showing a gray dot for a terminal running Claude Code or Codex while the tree showed it waiting or working; a tab now shows the tree's status dot, and the Claude or Codex icon pulses while the agent works.
- Fixed the desktop app's terminal leaving an empty strip on the right and at the bottom; it now uses the full width, and the leftover part of a row sits above the first line instead of below the prompt.
- macOS and Linux: Fixed the desktop app not finding `gh`, `glab`, `node`, `claude` and `stratum-lint` installed by Homebrew or nvm when it was started from the Dock, Finder or an app launcher; it now takes `PATH` from your login shell.
- Changed pacer is now berth: the command is `berth`, the npm package is `@petukhovart/berth`, and the first `berth` copies your pacer projects, workspaces and settings into its own directory while pacer's data and its running daemon stay as they are.
- Changed the desktop app's web UI to work on a phone opened through remote access, in portrait and landscape: dialogs, the command palette and quick switcher open full screen, menus slide up from the bottom, settings and diffs show a list first and the details after a tap, row actions stay visible without a mouse, a long press opens the context menu and a `Commands` tab opens the command palette.
- Changed the environment variables from `PACER_*` to `BERTH_*`; the old spelling still works, except that berth refuses to start on `PACER_RUNTIME_DIR` or `PACER_DATA_DIR` set without its `BERTH_*` counterpart.
- macOS: Changed the desktop app's shortcuts to use `Cmd` where Windows and Linux use `Ctrl` (`Cmd+Shift+P`, `Cmd+P`, `Cmd+,`, `Cmd+click` to open beside), with `Cmd+C` and `Cmd+V` copying and pasting in the terminal while `Ctrl+C` always reaches the agent, and Option acting as Meta there unless `terminal.option_as_meta` is turned off.
- Changed scrollbars in the desktop app to be wider and easier to grab with the mouse, and to darken while hovered or dragged.
- Changed the desktop app's update dialog to show a progress bar and the current stage instead of build ids, commits and the build log.
- Removed the hot session chips from the desktop app's title bar; the `Ctrl+Shift+O` overview, `Alt+1`..`Alt+9`, `Alt+J` and `Ctrl+Tab` still switch between live sessions.
- Removed the refresh button above the desktop app's tree; Jira issues now refresh on their own every two minutes, as pull requests and reviews already did.
- Removed prebuilt Linux binaries from releases and npm; on Linux `install.sh` builds berth from source with `cargo install`.

## 0.20.0 (2026-09-18)

- Changed `Shift+O` from listing only orphaned sessions to a project-wide session picker: it now shows every conversation from every worktree of the project, so a session started in one checkout can be resumed in any other.
- Windows: Fixed `Esc` doing nothing in a Claude Code session on Windows 10, so a running turn could not be cancelled and the next letter typed landed as `Alt`+letter. The built-in console host drops the key encoding Claude Code asks for, so pacer keeps to the plain bytes there.
- Windows: Fixed `Shift+Enter` in a Claude Code or cursor-agent session sending the message instead of adding a line.
- Fixed sessions never getting their status, title or resume id on a machine with a system-wide `HTTP_PROXY`: the hooks sent their loopback call through the proxy, which answered 502. The hooks now bypass the proxy for the daemon.
- Fixed a Claude Code session staying on the running status forever after `Esc` cancelled its turn. Claude Code 2.1.270 stopped emitting the progress sequence pacer read for the end of a cancelled turn, so pacer now reads the window title as well.

## 0.19.0 (2026-09-10)

- Added mermaid diagrams to the `b` file preview: `.mmd` files and fenced `mermaid` blocks in markdown render as text art through the `mermaid_renderer` command (`mermaid-ascii -f -` by default), with one install hint when it is not on the path.
- Added a `Bold names` setting under Appearance: off draws the panel headers and project names at normal weight, for terminals whose bold face is a heavier font.
- Added `Ctrl+Shift+R` and `F5` to repaint the whole screen, and an automatic full repaint when the window regains focus, clearing stale glyphs left after the emulator repaints behind pacer's back. `Ctrl+Shift+R` needs a terminal that speaks the kitty keyboard protocol; `F5` always arrives.
- Added a `Row timestamps` setting under Appearance: turn it off and the dim `2h ago` label leaves the project, worktree and session rows, which gives the columns back to the names.
- Added the main checkout's gitignored files and symlinks to every new worktree, so `.env`, local settings and linked skills are in place before an agent moves in. Ignored directories such as `node_modules` and `target` are left for you to build.
- Fixed a session that had already answered sweeping yellow for another half hour, because a background subagent cancelled earlier in the turn never reported that it had stopped.
- Fixed `⇧←`/`⇧→`/`⇧↑`/`⇧↓` on the splash screen wrecking the saved layout: with no projects yet there is no body to move a panel across, so instead of swapping the panel with its neighbour the keys tore it out into a minimum-width strip and saved that. They now do nothing until the panels are on screen.
- Fixed `h`/`l` and `←`/`→` walking the fixed Projects/Worktrees/PRs/Sessions order regardless of the layout, which skipped past the panel actually next door. They now follow the panels' places on screen, so a panel moved with `⇧←`/`⇧→`/`⇧↑`/`⇧↓` is reached from where it sits, and `h` steps over the terminal pane to a panel moved past it.
- Fixed a session cancelled with `Esc` keeping its red waiting-on-you dot indefinitely, which carried up to the worktree and project rows. The turn is now closed out from the agent CLI's own idle state within a minute.
- Fixed the Projects panel wearing the open workspace's name as its header while the Workspaces bar is off. The header reads `PROJECTS` either way, and the footer nameplate still names the workspace.
- Changed session rows to drop the trailing CLI badge when every session in the tree runs the same CLI. The badge is back as soon as a second kind appears.
- Windows: Fixed the Hotkeys tab warning about `^⇧`, `^→` and `^⌫` chords, which the console delivers fine. It now warns about `^Esc` (the Start menu) and Windows Terminal's own defaults.

## 0.18.0 (2026-09-03)

- Added native Windows 10 and 11 support: pacer runs there the same way it does on macOS and Linux.
- Added movable panels: `Shift+←/→/↑/↓` moves the focused Projects, Worktrees or Sessions panel through the body like a tiling window manager, so you can swap it with a neighbour, stack it above or below the terminal, or park it on any edge of the screen. Horizontal rules drag like the vertical ones, and the arrangement is remembered.
- Added a PRs panel: the open pull requests are a panel of their own, stacked under Worktrees by default, with its own cursor, filter and `Shift+R` toggle, and movable like the other three.
- Added GitLab alongside GitHub, with merge requests, comments, approvals and diffs in the same interface, self-hosted instances included.
- Added review and pipeline status to OPEN PRS rows, as icons to the left of the number, each in its own column.
- Added an `Open PRs filter` setting: the group lists all open pull requests, only yours, or ones you took part in. The login is asked of the forge inside that checkout, so self-hosted GitLab and GHE answer for themselves, and if it can't be determined the list is hidden rather than silently showing everything. Changing the filter refreshes immediately, and a reply to an already-sent request with the old filter is discarded.
- Added pins (`p`) for any number of workspaces, projects, worktrees and sessions. Pinned rows carry a ★, float to the top of their list and survive a restart.
- Added per-column sorting (`Projects sort`, `Worktrees sort`, `Sessions sort`); `⇧S` cycles the sort of the column the cursor is in, between recent, name and creation order. A config predating the split carries one `list_sort`, which all three columns adopt.
- Added an inline list filter (`Ctrl+F`): a fuzzy query narrows the focused panel, `Enter` parks it, `Esc` clears and closes.
- Added orphaned sessions (`⇧O`), so deleting a worktree no longer takes the conversations with it. pacer saves the agent CLIs' session ids first and lists them per project, with branch, date and transcript size. `Enter` resumes one in whatever worktree the cursor is on, and Claude is told the old directory is gone. The list draws on two sources: pacer's own table (all three CLIs, from this version on) and Claude Code's transcripts on disk, which also turns up sessions lost earlier.
- Added a thread tree for PR conversations: replies sit under the root comment with `├` / `└` branches instead of interleaved by time, and the root shows the file and diff line it hangs on and a `✓ resolved` mark. GitLab threads come from `/discussions`, and "requested changes" shows as a verdict alongside approval. If the endpoint doesn't answer, the previous flat list remains.
- Added an npm install route: `npm install -g @petukhovart/pacer` on macOS, Linux and Windows alike. npm fetches the binary for your platform only, with no build and no download at install time.
- Added prebuilt binaries for five targets (macOS arm64/x64, Linux x64/arm64 on static musl, and Windows x64) to each GitHub release. `install.sh` and `pacer upgrade` take them from there.
- Changed lists to default to `created` order, a stable creation order, so they no longer re-sort themselves as you work. `recent` and `name` are opt-in through `⇧S` or the settings, and cursors stay on the same row across a sort change rather than the same index.
- Changed `n` to go straight to naming the session when only one harness is enabled, instead of offering a menu of one.
- Changed the documentation layout: the README is now the pitch and the install, and the rest moved into `docs/` (getting started, keymap, CLI, configuration, Windows, releasing), plus a guide to reaching the TUI from a phone over Tailscale.
- Windows: Added loopback TCP with a bearer token between the daemon and the TUI, in place of a unix socket. Port and token live in an endpoint file in the per-user runtime dir.
- Windows: Added the daemon guard, so a second daemon refuses to start over a live one; `pacer kill` and the version check work.
- Windows: Added windowless background operation for the daemon, which survives the client closing.
- Windows: Added end-to-end session support: agent launch, output, scrollback and turn status.
- Windows: Added whole-process-tree kill when a session closes, so nothing is left in the background.
- Windows: Added session memory metrics, read through the Win32 APIs.
- Windows: Fixed agent CLIs installed through npm failing to launch.
- Windows: Fixed `~/` not resolving when `HOME` is unset; it now resolves through `USERPROFILE`.
- Windows: Fixed background `git` and `gh` calls flashing console windows.
- Windows: Fixed links not opening in the browser, for `Enter` on a pull-request row and `⌥`-click on a URL.
- Windows: Fixed the editor modal failing when `vim` and `nano` are not on `PATH`; it now finds Git for Windows' own copy.
- Windows: Fixed UTF-8 files opening in the editor as mojibake.
- Windows: Fixed `Shift+Enter` and other Ctrl/Alt chords not reaching console programs.
- Windows: Fixed a multi-line paste (`Ctrl+V`) reaching the agent line by line instead of as one message.
- Windows: Fixed `pacer ssh` mishandling the console, the exit code and `Ctrl+C`.
- Windows: Fixed `pacer upgrade` failing obscurely; it now declines and points at `npm update` or `cargo install`.
