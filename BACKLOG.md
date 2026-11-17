# Features backlog

Ideas not yet built, with where they come from. Name things with the terms in `GLOSSARY.md`.

## From t3code

Source paths are in [`pingdotgg/t3code`](https://github.com/pingdotgg/t3code).

- Remote connect over the best route: the client already ranks LAN, Tailscale and a public URL and
  moves to a better one by itself (`apps/web/src/remoteRoutes.ts`). Still missing: ssh as one of
  those routes, since it is a separate mode chosen by `BERTH_REMOTE`. Source:
  `docs/user/remote-access.md:110-137`.
- Resume after the usage limit resets: an Agent at its limit shows the reset time in the Inbox, but
  nothing continues it when the window resets. The Return chip of the router only offers to switch
  back to the earlier Agent or model. Source: `docs/user/thread-sidebar.md:133-269`.
- Prompt queue under a terminal Session: a follow-up typed while the Agent is busy is sent when it
  turns Finished. Built for the Chat view of ACP Agents (`apps/web/src/entities/session/model/promptQueue.ts`).
  Still missing: the queue for PTY Agents, editing a queued prompt, and keeping the queue across a
  reload. Source: `docs/user/composer.md:63-91`.
- Subscription limits for Cursor: the 5-hour and weekly windows are read for Claude and Codex
  (`crates/berth-daemon/src/usage_limits.rs`). Source: `docs/user/usage.md:113-129`.
- Dictation engine in released builds: whisper.cpp and sherpa-onnx are behind the `voice` Cargo
  feature, which only the local install scripts turn on; `release.yml` builds without it, and Linux
  never builds it. Source: `docs/user/voice-input.md`.
- A screen in the desktop app for scheduled prompts and webhooks, which `berth schedule`,
  `berth webhook` and `berth triggers` cover from the command line today.
- Suggested next prompt: the Chat view shows Claude's own predicted prompt and `Tab` fills it in.
  Still missing: `Escape` to dismiss it, and an opt-in setting. Source: `docs/user/composer.md:161-169`.

## From openclaw

Source paths are in [`openclaw/openclaw`](https://github.com/openclaw/openclaw).

- Voice loop: the phone Chat view reads the reply aloud with code stripped and stops when you speak
  (`apps/web/src/widgets/conversation/model/useVoiceLoop.ts`). Still missing: a short summary that
  points to the screen instead of the whole reply, speaking on status changes rather than only when
  the reply settles, and the loop on the desktop. Source: `docs/nodes/talk.md`, `docs/tools/tts.md`.
- Voice notes over messengers other than Telegram. Source: `docs/nodes/audio.md`.

## From buzz

- Overlaps for Agents: the overlap warning runs in the desktop web client only
  (`apps/web/src/features/overlaps`). An MCP tool and a `berth` command that list overlaps need it
  in the Daemon.

## From the TUI

- A berth on another machine from the UI: the desktop app attaches to a Daemon over ssh and installs
  berth there (`apps/desktop/src-tauri/src/ssh_remote.rs`), but only when `BERTH_REMOTE` is set. Still
  missing: choosing the machine in Settings, and the web client doing the same.
- Token cost per Session: token counts are shown per Session. Still missing: prices from rates the
  user enters, never a built-in table. Source: `git show d4ee7a4^:crates/berth-tui/src/price.rs`.
- Agent presets for scheduled prompts: presets start Sessions from New Session
  (`apps/web/src/features/new-session/model/presets.ts`) but live in the browser and are unknown to
  the Daemon, so a scheduled prompt cannot launch from one.

## Shared agent setup

Built in `crates/berth-daemon/src/shared_setup`. Still missing: hooks for OpenCode and ACP Agents,
plugins for Agent kinds other than Claude, and a Worktree made outside berth getting the setup before
berth discovers it.
