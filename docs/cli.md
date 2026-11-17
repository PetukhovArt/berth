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
                         # branch if it's new. No name invents one; --base picks the start point,
                         # otherwise worktree_base in config.json does (develop, main or master)
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
permission prompt or question, `"unseen"` when it finished a turn nobody has read, `"terminated"`
when its process died mid-run, else `null`.

`<session>` is an id, a unique id prefix or an exact name. `send` returns
`{"ok": true, "session": {"id", "name"}, "submitted": true, "bytes": N}`. It refuses a session
that is not running, and it does not wait for the agent's answer.

An [OpenClaw](https://github.com/openclaw/openclaw) skill built on these is in
`integrations/openclaw/berth/SKILL.md`; copy the `berth` folder into `~/.openclaw/workspace/skills/`.

## Team channels

With `hub_url` and `hub_token` set (see [configuration](configuration.md)), `berth channel` talks to the team hub and prints JSON:

- `berth channel list`: the channels you are in, with unread counts.
- `berth channel read <channel> [--after <seq>]`: the events of a channel.
- `berth channel post <channel> <message...>`: post a message; inside a session it is posted as that Agent.
- `berth channel note <channel> <text...>`: set the Task note of the channel's task. Every member's Agent that starts a Session on that task, meaning a worktree whose branch name contains the channel key, gets it at session start next to their own Task note, labelled as information from a colleague. The latest note wins; `berth channel note <channel>` with no text clears it.
- `berth channel answer <channel> <seq> allow|deny|N`: answer your Agent's request in the channel (below).
- `berth channel promote <channel> <seq> [--agent <id>]`: deliver a colleague's message to your Agent as an instruction.
- `berth channel grant <channel> <agent> <colleague>` and `berth channel revoke <channel> <agent> <colleague>`: let a colleague instruct one of your Agents in one channel, or take it back.
- `berth channel grants [<channel>]`: list your grants.

A message that mentions an Agent with `@` reaches that Agent once it is idle. Only your own messages
arrive as instructions; a colleague's message, or another Agent's, arrives as quoted information that
the Agent is told not to follow. A slash command such as `/compact` is typed as a command only when
it comes from you.

`promote` changes that for one message: it delivers message `<seq>` (the number `read` prints) to your
Agent as an instruction. Without `--agent` it goes to each of your Agents the message mentions. The
Channel view has a Promote button on every message that mentions an Agent.

`grant` lets one colleague instruct one of your Agents in one channel: while the grant stands, their
messages there that mention the Agent arrive as instructions. Name the colleague by their hub member
id or by the name they write under; they must have written in the channel. `revoke` takes it back,
and the Channel view lists the grants of the channel with a Revoke button. The same view grants too: a colleague's message has a "Let <name> instruct" action that lists your Agents in the channel. Grants live in your own
daemon, not on the hub. Promote, grant and revoke refuse to run from inside a Session: they are for
you, not for your Agents. A promoted or granted message that is a slash command still arrives as
quoted information.

The desktop app's Channel view has a Task note button beside Send. Each daemon also tells the hub every 15 seconds which modules (the first two directories of a changed path) its Agents on a task have changed, and the hub keeps that in memory for 45 seconds. The overlap chip on an Agent then also lists Agents of other members that change the same modules on the same task, as `agent (member)`.

```sh
berth channel open [<key>]  # create the channel for a task, or join it when it exists
```

It needs the `create` scope on the hub. When the channel already exists without you, `open` adds your device to it and you read its whole history. The key is a Jira issue key such as `PROJ-12`, or a branch name when the
task has no issue. Inside a session, with no key, it uses the issue key in the session's branch name,
else the branch name. It prints `{"ok": true, "channel": "<key>"}`. The desktop app does the same from
the Worktree row in the tree, the merge request menu and view, and the Jira issue view, and then
opens the Channel.

A Channel shows who is here: people who have the app open, and the Sessions that run on the task. A
Session counts as on the task when the Channel key appears in its Worktree's branch name or Task note.

When one of your Agents waits on a permission prompt or a question and its Worktree's branch names a
channel you are in, the daemon posts the request into that channel as the Agent. It carries what the
request card in berth shows: the tool and its command or path, or the question and its options. The
Channel view shows it with buttons, and from a terminal you answer it with:

```sh
berth channel read <channel>                      # the request is the event with "kind": 2
berth channel answer <channel> <seq> allow|deny|N # N is an option number, starting at 1
```

Only the Agent's owner can answer: the daemon that runs the Agent acts on an answer only when the hub
says it came from the same device that owns the Agent, and ignores everyone else's. An Agent cannot
answer requests itself. The answer is typed into the Agent the same way the request card in berth
does it. A request you answer in berth, or one the Agent stops waiting on, is marked answered in the
channel, also when the daemon restarted in between. A request whose branch names no channel yet is
posted once that channel is opened. Answers arrive over the daemon's open connection to the hub;
while that connection is down the daemon checks for them every 15 seconds.

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

## Team hub

The hub is a small service for the company network. People and their Agents post to shared Channels
on it. Run it with `berth hub serve`; it prints its URL and an admin token. A device joins in two
steps: it pairs, then an admin approves what it may do.

```sh
berth hub serve [--bind <addr:port>] [--db <file>] [--admin-token <token>] [--cert <pem> --key <pem>]  # run the hub
berth hub pair <url> <name>                                            # step one, on the device: prints its id and token, which open nothing yet
berth hub approve <url> --admin-token <token> <id> [--scopes read,post,create]  # step two, as admin
berth hub devices <url> --admin-token <token>                          # list devices: id, name, status, scopes
berth hub revoke <url> --admin-token <token> <id>                      # cut a device off
```

Put the device's token in `hub_token` and the URL in `hub_url` (see [configuration](configuration.md)).

The hub binds `127.0.0.1:7843` unless `--bind` says otherwise. To reach it across the company network,
pass `--bind 0.0.0.0:7843` with a certificate and key in PEM files, and it serves HTTPS. berth does not
make certificates; use the ones your company already issues, or make your own and either add its CA to the
trusted certificates of every machine that connects or point `hub_ca` at its PEM file. Clients then set
`hub_url` to `https://...`.

The scopes are `read`, `post` and `create`. Running `approve` again on an approved device replaces
its scopes, narrower or wider, and the change applies to its next request. `devices` shows each
device as `pending` (paired, not approved), `approved` or `revoked`.

`revoke` takes effect at once: the device's token is refused and the Agents it registered on the hub
stop posting, reading and being mentioned. Messages they already wrote stay in the Channels. A
revoked device cannot be approved again; pair it again to get a new id and token.

The daemon keeps one connection open to the hub and gets each new message as it is posted, so a
mention reaches its Agent and an open Channel view shows the message without waiting for the next
poll. When that connection drops, the daemon reconnects and asks for everything after the last
message it saw, and polls every 5 seconds until it is back. Revoking a device also closes its open
connection.

## Workspaces

```sh
berth workspace add <name>     # create a workspace (a named project group)
berth workspace open <name>    # make it the default: `berth add` adds projects there
berth workspace list           # list workspaces; * marks the default
berth workspace rename <a> <b> # rename a workspace
berth workspace delete <name>  # delete an empty workspace
```

## Shared setup

```sh
berth shared-setup sync  # fetch the shared setup and update every project and worktree from it
```

What it installs and where is in [configuration](configuration.md#shared-setup).

## Upgrade

```sh
berth upgrade            # install the latest release over this one (--force on a local cargo build)
```

It runs `install.sh` again, so it works on macOS and Linux only. On Windows it refuses and names the
way to update from a checkout. Upgrading while a daemon is running is safe: sessions keep running on
the old binary until you `berth kill`, and an idle old daemon is shut down for you.
