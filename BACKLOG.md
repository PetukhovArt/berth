# Features backlog

Ideas not yet built, with where they come from. Name things with the terms in `GLOSSARY.md`.

## From t3code

Source paths are in [`pingdotgg/t3code`](https://github.com/pingdotgg/t3code).

- Different routes to remote connect with priority switch for best (secure): LAN, Tailscale, SSH
  and a public URL to one machine; the client takes the first that answers and moves back to a
  better one by itself. Source: `docs/user/remote-access.md:110-137`.
- Sidebar inbox, important inbox up: a Session returns to the top when it finishes, fails or waits
  for an answer; snooze, settle, and a Limited state with resume after the usage limit resets.
  Source: `docs/user/thread-sidebar.md:133-269`.
- Device tokens with scopes: one token per paired device, revocable, with read-only or full rights,
  in place of the one shared token in `apps/desktop/src-tauri/src/remote.rs`. Source:
  `packages/contracts/src/auth.ts:31-66`, `apps/server/src/auth/PairingGrantStore.ts`.
- Prompt queue: a follow-up typed while the Agent is busy is sent when it turns Finished, editable
  until then. Source: `docs/user/composer.md:63-91`.
- Checkpoints on hidden refs: a snapshot of the Worktree at each prompt, restorable without commits
  on the branch. Files only: the native TUI gives no way to roll the conversation back. Source:
  `apps/server/src/orchestration-v2/CheckpointService.ts`, `apps/server/src/vcs/GitVcsDriver.ts`.
- Archive after merge: a Session whose merge request merged settles by itself, opt-in.
- Watch a merge request: wake the Agent with input when its pipeline fails or a new thread arrives.
  Source: `docs/user/source-control.md:324-333` (`watch_pull_request`).
- Subscription limits: 5-hour and weekly windows with reset times for Claude, Codex and Cursor.
  Source: `docs/user/usage.md:113-129`.
- Worktree setup scripts: a command per Project that runs after a Worktree is created, such as
  installing dependencies. Source: `t3.json` (`runOnWorktreeCreate`).
- Dictation: speech to text into the input of a Session, from the desktop app and from a phone. T3
  Code offers three engines: on the device, on your own server (transcribe-cpp with Whisper or
  Parakeet models) and in the cloud. Source: `docs/user/voice-input.md`,
  `apps/server/src/speech/SpeechService.ts`.
- A screen in the desktop app for scheduled prompts and webhooks, which `berth schedule`,
  `berth webhook` and `berth triggers` cover from the command line today.
- Chat view of a berth Session, for people who do not want a terminal: messages, grouped tool calls
  and changed files read from the transcript, with a composer below, as a fourth view beside the
  terminal, merge request and changes. The same PTY Agent stays underneath: the composer types into
  it, and a permission request or a question shows as a card with buttons. Switching views never
  restarts the Agent. The transcript parser exists for External Sessions
  (`apps/web/src/widgets/conversation/ui/ConversationView.vue`). Source: `docs/user/composer.md`.
- Suggested next prompt in the Chat view: when a turn finishes, the empty composer shows a predicted
  next message as grey text with a `Tab` hint; `Tab` fills it in for editing, `Escape` dismisses it.
  Opt-in, since each suggestion costs one request on the user's plan; skipped on the first turn and
  near usage limits. Claude Code already predicts one in its own terminal UI, so read that before
  asking a model. Source: `docs/user/composer.md:161-169`.

## From openclaw

Source paths are in [`openclaw/openclaw`](https://github.com/openclaw/openclaw).

- HTTPS through Tailscale Serve: the remote server stays on loopback and Tailscale terminates TLS, so
  no certificate of our own. A phone browser needs HTTPS for the microphone and for Web Push, so
  this comes before the two items below. Source: `docs/gateway/tailscale.md`.
- Push when an Agent needs you: a notification on the phone when the Agent Status turns to waiting or
  Finished. Web Push from the installed web client needs no relay and no Apple or Google keys; a
  message to Telegram from the Daemon is the fallback. Source: `docs/cli/message.md`.
- Voice loop: speak a prompt, hear back the status change and a short summary of the answer. Code
  is never read aloud, the summary points to the screen. Speaking over the reply stops it. Speak on
  status changes, not only at the end: OpenClaw without realtime mode stays silent until the whole
  run finishes. Source: `docs/nodes/talk.md`, `docs/tools/tts.md`.
- Wake word and push-to-talk: a trigger phrase routed to one Session, or a held key that sends on
  release. Source: `docs/nodes/voicewake.md`.
- Voice notes over a messenger: an inbound voice message is transcribed into a prompt, the reply
  comes back as a voice message. Voice control from a phone with no app of our own. Source:
  `docs/nodes/audio.md`, `docs/tools/tts.md`.
- JSON-emitting `berth` CLI and an OpenClaw skill on top of it: list Sessions, show which need
  attention, send input. Gives every OpenClaw channel and its voice at once, before any of the
  above is built in berth. Source: `docs/tools/creating-skills.md`.

## From buzz

Ideas from an earlier team-workspace prototype whose relay was ported from
[`block/buzz`](https://github.com/block/buzz). These help one person with several Agents and need no
shared server:

- Task notes: a decision, contract or caveat attached to a task or Worktree, injected into every
  Agent that starts there through the Managed Hooks at session start, so parallel Agents follow the
  same constraints. Not "pins": **Pin** already means something else in `GLOSSARY.md`.
- Overlaps: warn when two Agents in different Worktrees of one Project touch the same module, before
  they produce conflicting merge requests. A module is the first one or two directories of a changed
  path; an overlap is a module touched by more than one Agent.
- Tracker write-back, off by default: when an Agent finishes, post its status and branch as a comment
  on the Jira or GitLab issue, so colleagues see progress without berth.
- A small MCP server for Agents: read task notes and overlaps, post a message to another Agent of the
  same task. It shares its commands with a JSON-emitting `berth` CLI, which voice control and
  messenger bots would use too.

## From the TUI

What the terminal UI did that the desktop app does not, worth keeping once the TUI is gone.

- A berth on another machine: the desktop app and the web client attach to a Daemon on a server over
  an ssh tunnel, and install berth there when it is missing. Replaces `berth ssh`, `berth tunnel` and
  `berth browser`; the tunnel code is `crates/berth/src/tunnel.rs`.
- Token use per Session, beside the subscription limits, so a spent window shows which Agent spent
  it. Prices only from rates the user enters, never a built-in table. Source:
  `crates/berth-tui/src/diagnostics.rs`, `crates/berth-tui/src/price.rs`.
- Agent presets: a saved Agent, model, effort and text put before and after the task. The start card
  and scheduled prompts launch from one. Source: `crates/berth-tui/src/agent_presets.rs`.
- Mark a file reviewed in a diff, kept per merge request. Source: `crates/berth-tui/src/review.rs`.
- Open a Project's repository page on its git host from the Project menu, the `origin` URL made
  browsable with credentials stripped.
- Daemon settings the desktop Settings do not show yet: run `git init` for a new folder, prewarm
  Agents, prewarm Sessions. Read by `crates/berth-daemon/src/config.rs`.
- A Linux build of the desktop app, since the TUI was the only interface on Linux.

## Team

A shared place on the company network where people and their Agents work on a task together. The
concepts come from Buzz and OpenClaw; the code is ours and small. Not a Buzz fork and not the Buzz
relay under berth: the port came to about 118k lines of Rust plus Postgres and Redis, more than all
of berth, and Buzz runs Agents over ACP without their native TUI.

Principles:

- An Agent runs on its owner's machine, in its owner's PTY, on its owner's subscription and tokens.
  The shared part is the conversation, never the code, the credentials or the terminal.
- An Agent takes instructions from its owner only. Messages from other people and other Agents
  reach it as quoted information. The Daemon enforces this, not a line in a skill (Buzz: "Who can
  send instructions: Only me").
- Channels exist for a task or a merge request. No direct messages, files, reactions or search in
  the first version, or this grows into a messenger.

First slice, enough to learn whether colleagues read a shared channel:

- Hub: a small service for the company network with channels, members and messages over HTTP and
  WebSocket, stored in SQLite. A new role: `berth-server` gives remote access to one person's
  Daemon, the hub connects several people. The log is append-only, every event has a sequence
  number, and a client that reconnects asks for everything after the last number it saw (Buzz).
- Members with their own identity: a person joins with a device token; each of their Agents is a
  member too, marked with its owner, so every message shows who wrote it, a person or an Agent
  (Buzz: agents are members, not bots). Access to a channel comes from membership only.
- Two-step pairing: a device is first admitted to the hub, then what it may do is approved
  separately (OpenClaw: `docs/gateway/pairing.md`).
- Channel per task or merge request, created from a Jira issue, a merge request or a Worktree and
  linked by the same issue and branch keys the project tree already uses.
- Channel view in the desktop app and the web client, next to the views of a Session, with unread
  channels in the sidebar inbox.
- `berth channel read` and `berth channel post` for Agents, part of the JSON-emitting CLI: an Agent
  reports status, asks a question or states a contract in the channel of its task.

Built so far (#49), in `crates/berth-hub` and `crates/berth/src/channel.rs`: the hub over HTTP in
SQLite with an append-only log, `berth hub serve|pair|approve`, `berth channel list|read|post`
through the daemon (`hub_url`, `hub_token` in the config), two-step pairing, Agents as members
marked with their owner. Missing from the first slice: WebSocket push (clients read with
`after=<seq>` today), creating a channel from a Jira issue, merge request or Worktree key, the
channel view in desktop and web, unread channels in the Inbox, TLS, token revocation, and
re-approving or narrowing a device's scopes.

Second slice, Agents take part:

Built so far (#50), in `crates/berth-daemon/src/team.rs` and the hub's `GET /mentions`: delivery
into the PTY and the Agent-to-agent chain limit (`channel_chain_limit`). A message that writes
`@<agent id>` tags the Agent; the Daemon polls the hub every 5 seconds, types the message when the
Agent is Finished or Fresh, and drops a message whose Agent chain is longer than the limit. Still
missing: Promote, Grant, slash commands, remote approvals, mentioning by an Agent's display name
rather than its id, registering every local Agent on the hub (an Agent is mentionable only after
its first `berth channel` call), and mentions made while the Daemon was down (the cursor starts at
the hub's head).

- Delivery into the PTY: a message that mentions an Agent is typed into its Session by the Daemon
  when the Agent is idle, as an instruction if the author is the owner and as quoted information
  otherwise.
- Promote to instruction: the owner turns a colleague's message into an instruction for their Agent
  with one action. This is how people correct each other's Agents without commanding them.
- Grant: an owner may let a named colleague instruct one Agent in one channel, revocable.
- Agent-to-agent limits: an Agent answers only when mentioned, and a chain of Agent replies stops
  after a set number of turns, so two Agents cannot loop and burn tokens.
- Slash commands from the channel: an owner's message that starts with `/` reaches the Agent as a
  command, not as text (Buzz: `crates/buzz-acp/src/pool.rs`).
- Remote approvals: the owner answers their Agent's permission request from the channel or a phone
  (OpenClaw: `docs/tools/exec-approvals.md`).

Third slice, shared context:

- Presence: who is online and which Sessions run on the task, sent as short-lived events that are
  never stored (Buzz).
- Shared task notes: a note pinned in the channel is injected into the Agents of every member who
  starts a Session on that task.
- Team-wide overlaps: the overlap warning counts the Agents of all members, not only your own.
- Live feed of an Agent for colleagues: messages, grouped tool calls and changed files, read-only,
  opt-in per Session. The cheapest form is a read-only share link to one Session over the existing
  remote server (`apps/desktop/src-tauri/src/remote.rs`), like T3 Code guest links.
- Bridge to the company messenger: attention events and task notes go out to the chat the team
  already uses (OpenClaw: `docs/cli/message.md`).

Built so far (#51): presence only. Every 15 seconds the Daemon (`crates/berth-daemon/src/presence.rs`)
tells the hub which of its running Agents are on which Channel, matched by the Channel key inside the
Worktree's branch name; the hub keeps that in memory for 45 seconds (`crates/berth-hub/src/presence.rs`,
`GET /presence`) and the Channel view shows it above the messages. Still missing: the other four
items above, a presence line for a person who has the app open but no Session, and matching by a
Task note key rather than the branch name.

Keep the event shape close to Buzz (a numeric kind, an author, a channel, tags) so a Buzz relay can
replace the hub if the team outgrows it.

## Shared agent setup

From the user. One repository holds the skills, plugins, MCP servers, hooks and instructions a
person wants; berth installs them into every Project and Worktree and into every Agent kind (Claude,
Codex, Grok, Cursor, ACP agents), translating to each one's format: `.claude/` skills and
`settings.json`, Codex `config.toml` MCP entries, Cursor rules, `AGENTS.md`. A config in that
repository says which items go to which Agent kinds and Projects, so a new Project or a new Agent
kind gets the same setup without copying by hand. Open: link or copy, how a Project overrides an
item, and keeping in step when the repository changes.
