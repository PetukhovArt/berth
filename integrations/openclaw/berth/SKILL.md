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
