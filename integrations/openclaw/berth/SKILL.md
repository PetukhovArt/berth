---
name: berth
description: List berth coding agent sessions, see which need attention, and send them a message.
metadata: { "openclaw": { "requires": { "bins": ["berth"] } } }
---

Use the `exec` tool to run the `berth` CLI. Every command below prints one JSON object on stdout. Check `ok` first: on failure it is `false` and `error` holds the reason (the exit code is also nonzero).

## Commands

- `berth sessions` lists live sessions. `berth sessions --all` adds archived ones.
- `berth attention` lists only the sessions that need the user.
- `berth send <session> <message...>` types the message into a running session and presses Enter. `--no-enter` leaves it in the input box.

`<session>` is a session `id`, a unique id prefix, or an exact `name`.

## Reading the output

`sessions` and `attention` return `{"ok": true, "sessions": [...]}`. Each session has:

- `id`, `name`, `kind` (the agent CLI), `model`
- `status`: `fresh`, `running`, `finished`, `needs_feedback`, `terminated` or `disconnected`
- `attention`: `"needs_feedback"` when the agent waits on a permission prompt or a question, `"unseen"` when it finished a turn nobody has read, otherwise `null`
- `alive`: whether the session is running now; `send` only works on a running session
- `archived`, `status_changed_at` (epoch ms)
- `workspace`, `project`, `branch`, `path` (the checkout), each `null` when unknown

`send` returns `{"ok": true, "session": {"id", "name"}, "submitted": true, "bytes": N}`.

## How to use them

- To answer "what needs me", run `berth attention`. An empty `sessions` array means nothing does.
- Summarise each session by `project`, `branch` and `name`, and say whether it waits (`needs_feedback`) or finished (`unseen`).
- Before `berth send`, confirm with the user which session gets which text, unless they already named both. A message goes straight into an agent that can edit code.
- When two sessions share a `name`, send by `id`.
- `berth send` does not wait for the agent's reply. Run `berth sessions` later and read `status`.

## Voice notes

OpenClaw channels such as Telegram, WhatsApp and Discord turn an inbound voice note into text before the skill sees it, and can send the reply back as a voice message. Berth adds nothing between the two: the transcript is the user's message, and the commands above are the same.

To set it up, turn on audio understanding in `~/.openclaw/openclaw.json` under `tools.media.audio` and pick an engine. A local one (the `whisper` or `whisper-cli` binary, or `sherpa-onnx-offline`) needs no account and no key; a provider engine takes its API key from the environment. Replies as voice come from OpenClaw's `messages.tts` setting. Keep bot tokens and API keys in the environment or in OpenClaw's config, never in this skill.

Rules for a voice request:

- A transcript can mishear a session name or a word that matters. Before `berth send`, read back the session and the text you will send and wait for a yes, even if the user named both aloud.
- Do not send a transcript that is empty or that you cannot make sense of; ask the user to repeat it.
- Reply short and spoken: say which sessions wait and why, never read code or paths aloud, and point to the screen for the detail.
