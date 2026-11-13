# Command reference

The `berth` command runs the daemon and the commands agents and scripts run against it. The interface
you work in is the desktop app; see [getting started](getting-started.md). A bare `berth` prints help,
and `berth <command> --help` describes one command.

## Projects

```sh
berth add <dir>          # add a repo as a project, named after its root directory
berth add .              # same, for the repo you're in (bare `berth <dir>` / `berth .` also work)
```

A directory whose name matches a command needs the long form or a `./` prefix.

## Daemon

```sh
berth daemon             # run the daemon (normally started for you)
berth daemon --foreground  # stay attached to the terminal instead of logging to file, for debugging
berth kill               # stop the daemon and all sessions cleanly
```

A command that needs the daemon starts one from its own binary when none is running. The desktop app
starts `berth daemon` from your `PATH` on Windows and macOS.

## Run by agents

These exist so a session can act on its own tree; you rarely type them yourself.

```sh
berth rename <title>     # title the current session (--force to retitle a named one)
berth worktree [name] [--base <ref>]
                         # move the current session into a worktree of its project, creating the
                         # branch if it's new. No name invents one; --base picks the start point
berth enter <dir>        # move the current session into the repo at <dir>, adding it as a project
                         # when berth does not have it; the conversation resumes there
berth spawn <task> [--kind <claude|codex|cursor|opencode|grok>]
                         # start a new agent session beside the current one, in the same worktree,
                         # opening on <task>; --kind defaults to this session's agent kind
```

## Scheduled prompts and webhooks

A prompt can be typed into a session, or start a new one, at a set time or when a webhook is called.
The target is `--agent <id>` (the default is the session the command runs in) or `--worktree <id>`
with an optional `--kind`, which starts a new session on the prompt each time.

```sh
berth schedule <prompt> --in <span> [--every <span>] [--name <n>] [--agent <id> | --worktree <id> [--kind <k>]]
                         # spans are like 30m, 2h, 1d; --every is at least 60s
berth webhook <prompt> [--name <n>] [--agent <id> | --worktree <id> [--kind <k>]]
                         # prints the hook's path and its secret, once
berth triggers [--remove <id>]
                         # list them as JSON, or delete one
```

A one-time schedule runs once and is then gone. Schedules live in the daemon and survive a restart;
a run missed while the daemon was down happens once when it starts, never once per missed period. A
repeating schedule is re-armed after its run.

A webhook is off until `"webhooks": true` is set in the config file (see
[configuration](configuration.md)). It then listens on `127.0.0.1` only, on `webhook_port`. Each hook
has its own secret. Call it with `POST /hooks/<id>` and the header `X-Berth-Secret: <secret>` (or
`Authorization: Bearer <secret>`). A wrong secret gets `401`. `X-Berth-Delivery: <id>` makes a repeat
of the same delivery a no-op, and a hook runs at most once every 5 seconds. The request body is
ignored: a hook only ever sends the prompt you saved, and never runs a command.

## JSON for scripts and chat bots

```sh
berth sessions [--all]   # list sessions (--all adds archived ones)
berth attention          # only the sessions that need you
berth send <session> <message...> [--no-enter]
                         # type a message into a running session and press Enter
```

Each prints one JSON object on stdout. On success it holds `"ok": true`. On failure it is
`{"ok": false, "error": "..."}` with exit code 1. Fields are only ever added, never renamed.

`sessions` and `attention` return `{"ok": true, "sessions": [...]}`. A session has `id`, `name`,
`status` (`fresh`, `running`, `finished`, `needs_feedback`, `terminated`, `disconnected`),
`attention`, `alive`, `archived`, `kind`, `model`, `status_changed_at` (epoch ms), `workspace`,
`project`, `branch` and `path`. `attention` is `"needs_feedback"` when the agent waits on a
permission prompt or question, `"unseen"` when it finished a turn nobody has read, else `null`.

`<session>` is an id, a unique id prefix or an exact name. `send` returns
`{"ok": true, "session": {"id", "name"}, "submitted": true, "bytes": N}`. It refuses a session
that is not running, and it does not wait for the agent's answer.

An [OpenClaw](https://github.com/openclaw/openclaw) skill built on these is in
`integrations/openclaw/berth/SKILL.md`; copy the `berth` folder into `~/.openclaw/workspace/skills/`.

## MCP server for agents

`berth mcp` speaks MCP over stdio and exposes the same reads and action as the JSON commands above:

- `berth_sessions` with optional `all`: the `sessions` document.
- `berth_attention`: the `attention` document.
- `berth_send` with `session`, `text` and optional `submit`: the `send` document, for messaging another agent of the same task.
- `berth_notes` with optional `session`: the Task note of that session's worktree, by default the calling session's.

Each tool returns the command's JSON as text. A failure sets `isError` and carries `{"ok": false, "error": "..."}`.
Overlaps are not served yet; the daemon has nothing to read them from.

Claude Code:

```sh
claude mcp add berth -- berth mcp
```

Codex, in `~/.codex/config.toml`:

```toml
[mcp_servers.berth]
command = "berth"
args = ["mcp"]
```

## Workspaces

```sh
berth workspace add <name>     # create a workspace (a named project group)
berth workspace open <name>    # make it the default: `berth add` adds projects there
berth workspace list           # list workspaces; * marks the default
berth workspace rename <a> <b> # rename a workspace
berth workspace delete <name>  # delete an empty workspace
```

## Upgrade

```sh
berth upgrade            # install the latest release over this one (--force on a local cargo build)
```

It runs `install.sh` again, so it works on macOS and Linux only. On Windows it refuses and names the
way to update from a checkout. Upgrading while a daemon is running is safe: sessions keep running on
the old binary until you `berth kill`, and an idle old daemon is shut down for you.
