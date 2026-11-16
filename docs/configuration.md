# Configuration

berth keeps two kinds of settings: the daemon's, in `config.json` beside the database, and the desktop
app's own, in `~/.berth`.

## Where things live

| | Path |
|---|---|
| macOS | `~/Library/Application Support/dev.berth/` |
| Linux | `~/.local/share/berth/` (data), `~/.local/state/berth/` (logs) |
| Windows | `%APPDATA%\berth\data\` |

That directory holds `config.json`, `berth.db`, and a `state/` subdirectory with `daemon.log`.

## Moving from pacer

berth was called pacer up to 0.20. The first time berth starts and finds no data directory of its own,
it copies pacer's over:

| | From | To |
|---|---|---|
| macOS | `~/Library/Application Support/dev.pacer/` | `~/Library/Application Support/dev.berth/` |
| Linux | `~/.local/share/pacer/` | `~/.local/share/berth/` |
| Windows | `%APPDATA%\pacer\data\` | `%APPDATA%\berth\data\` |

Your projects, workspaces, pins and settings come along, and `pacer.db` becomes `berth.db`. Logs,
database backups and the runtime directory stay behind. The copy happens once: when berth's directory
exists, pacer's is never read again. pacer's directory is never changed or deleted, so pacer keeps
working, and you remove it yourself when you no longer need it.

The copy is a snapshot of the moment berth first starts. Anything you do in pacer after that stays in
pacer, so stop pacer's daemon with `pacer kill` before berth's first start to carry everything over.

berth runs its own daemon (`%TEMP%\berth` on Windows, `/tmp/berth-<uid>` or `$XDG_RUNTIME_DIR/berth`
elsewhere), so a running pacer daemon and its sessions are left alone. When berth starts a session in
a checkout, it replaces the hooks pacer installed there with its own.

Environment variables are spelled `BERTH_*` now. berth still reads the old `PACER_*` spelling of
`AGENT_CMD`, `LOG`, `INSTALL_URL` and the test timing knobs when the new one is unset.
`PACER_RUNTIME_DIR` and `PACER_DATA_DIR` are different: berth refuses to start while one is set
without its `BERTH_*` counterpart, because following it would put berth on pacer's daemon or
database. With both data variables set, berth copies from `PACER_DATA_DIR` into a `BERTH_DATA_DIR`
that does not exist yet.

## Daemon settings

The daemon reads `config.json` fresh on each use, so hand edits apply without a restart. A missing
file or a missing key means the default.

| Key | Default | What it does |
|---|---|---|
| `session_idle_timeout` | `5m` | how long an unwatched idle session keeps its terminal before it is released (`off` never releases). The desktop app sets it in its settings |
| `restore_sessions` | `true` | start the sessions that were running when the daemon stopped again when it starts (agents resume their conversation). The desktop app's `sessions.restoreOnRestart` sets it |
| `prewarm_agents` | `true` | let a client have an agent CLI started ahead of time so a new session opens instantly; each idle one costs a few hundred MB |
| `prewarm_sessions` | `true` | let a client have a worktree's closed sessions booted ahead of time so they open on a booted screen |
| `claude_limits` | `false` | let the daemon read Claude Code's sign-in and ask Anthropic for your 5-hour and weekly usage every minute; off means no file read and no network call |
| `checkpoints_claude` | `false` | take Checkpoints for Claude agents too; Claude Code has its own `/rewind`, so they are off by default. Other agents always take them, and Checkpoints already taken stay listed and restorable. The desktop app's `daemon.checkpointsClaude` sets it |
| `webhooks` | `false` | listen for webhook prompts (`berth webhook`); read when the daemon starts |
| `webhook_port` | `47821` | the port the webhook listener uses |
| `webhook_expose` | `false` | listen on all interfaces instead of `127.0.0.1` only; put it behind something you trust, since the secret is the only check |
| `hub_url` | none | the team hub the `berth channel` commands talk to, like `http://hub.example:7843`, or `https://hub.example:7843` for a hub started with `--cert` and `--key`. The certificate must be trusted by the operating system or by `hub_ca` |
| `hub_ca` | none | path to a PEM file with the CA certificate (or the self-signed certificate itself) of an `https://` hub. berth trusts it in addition to the operating system's certificates, so a hub with a company-made certificate works without installing anything system-wide. `berth hub pair`, `devices`, `approve` and `revoke` read it too |
| `hub_token` | none | this device's token on the hub, from `berth hub pair` once an admin ran `berth hub approve` |
| `messenger_telegram_token`, `messenger_telegram_chat` | none | a Telegram bot token and the chat id to post to; both set turns the chat bridge on (see below) |
| `messenger_webhook_url` | none | an incoming-webhook URL to post `{"text": "..."}` to; set turns the chat bridge on |
| `channel_chain_limit` | `4` | how many Agent-authored messages in a row a channel delivers to Sessions before it stops, so two Agents cannot loop |
| `git_init_on_create` | `true` | `git init` a directory that `Add Project` creates because it did not exist |
| `shared_setup` | none | a directory, or a git URL, holding the skills, MCP servers, hooks and instructions berth installs into every project (see below) |

## Shared setup

Keep the skills, MCP servers, hooks and instructions you want in every project in one directory or
git repository, and point `shared_setup` in `config.json` at it: a local path like
`"~/agent-setup"`, or a URL like `"https://github.com/you/agent-setup.git"` or
`"git@github.com:you/agent-setup.git"`. berth copies it into every checkout of every project,
translated to what each agent reads. It does so when you add a project, when berth creates a worktree,
and when you run `berth shared-setup sync`. Only that command fetches a repository (it clones it into
`shared-setup/` in the data directory the first time and pulls after that), so run it after you change
the repository.

The directory holds `berth-setup.toml` and the items it names:

```
agent-setup/
  berth-setup.toml
  skills/review/SKILL.md      # a skill is a directory with its files
  instructions/style.md       # instructions are one markdown file each
  hooks/format.json           # Claude Code hooks, the "hooks" object of a settings file
```

```toml
# Every agent in every project
[[set]]
skills = ["review"]
instructions = ["style"]
mcp = ["context7"]

# Claude only, and only in two projects
[[set]]
kinds = ["claude"]
projects = ["berth", "web-app"]
hooks = ["format"]

[mcp.context7]
command = "npx"
args = ["-y", "@upstash/context7-mcp"]
env = { DEBUG = "0" }

[mcp.docs]
url = "https://example.com/mcp"
```

A set says which items go to which agents (`kinds`: `claude`, `codex`, `cursor`, `opencode`, `grok`,
`acp`) in which projects (`projects`: the name shown in berth or the repository's folder name). Leave
either out to mean all of them. An item in several sets goes to every agent any of them names. An MCP
server is a `command` with optional `args` and `env`, or a `url`.

Where each item lands in a checkout:

| Item | Claude | Cursor | Codex | Other agents |
|---|---|---|---|---|
| skill | `.claude/skills/<name>/` | `.agents/skills/<name>/` | `.agents/skills/<name>/` | `.agents/skills/<name>/` |
| instructions | `.claude/rules/<name>.md` | `.cursor/rules/<name>.mdc`, always applied | `AGENTS.md` | `AGENTS.md` |
| MCP server | `.mcp.json` | `.cursor/mcp.json` | `.codex/config.toml` | not installed |
| hooks | `.claude/settings.local.json` | not installed | not installed | not installed |

`AGENTS.md` joins every instructions item in the order the sets name them. Codex reads
`.codex/config.toml` only in a folder you trusted, and Claude Code asks once per checkout before it
starts the servers in `.mcp.json`. Hooks go to Claude only: Codex asks to trust hooks again in every
worktree, and Cursor's hooks are written differently. Hook commands run in the checkout, so a script
they call needs an absolute path.

berth copies files rather than linking them, and it remembers what it wrote in each checkout (in
`berth-setup.json` inside the checkout's git directory). A sync updates and removes only those files,
and only while they are still as berth wrote them. A file the project already has, or one you changed
after berth wrote it, is left alone and the sync names it, so a project overrides an item by keeping
its own file of the same name. The hooks berth adds to `.claude/settings.local.json` are marked
`_berthShared` and replaced on every sync; the rest of that file, and the hooks berth installs for its
own status tracking, stay as they are. berth also lists the files it writes in the repository's
`.git/info/exclude`, so they stay out of `git status` and out of commits. Remove `shared_setup` and
run `berth shared-setup sync` to take everything back out.

## Chat bridge

The daemon can post a short message to a chat when an Agent starts waiting on you, when an Agent
finishes a turn, and when you save a Task note. It is off until you set the
keys below in `config.json`, and nothing you type in the chat comes back to berth.

```json
{
  "messenger_telegram_token": "123456:ABC-your-bot-token",
  "messenger_telegram_chat": "-1001234567890"
}
```

For Telegram, create a bot with `@BotFather`, add it to the chat, and use the chat id (a number,
negative for a group) as `messenger_telegram_chat`. For a company messenger that offers an incoming
webhook, such as Slack or Mattermost, set `messenger_webhook_url` instead: berth sends a `POST` with
the JSON body `{"text": "..."}`. Both can be set at once.

A message carries the Session title, the Project and the status, like `Needs feedback: fix login (api)`
or `Finished: fix login (api)`. A Task note goes out as its first 300 characters with the Project and
branch. No code or terminal output is sent. The token and the webhook URL are secrets: they sit in
`config.json` in plain text like `hub_token`, so keep that file private. The daemon posts through
`curl`, which must be on `PATH`, and hands it the secret on stdin, not in its arguments. A failed post
is logged in `daemon.log` and not retried.

## Mentions in team channels

With `hub_url` and `hub_token` set, the daemon registers every Agent on the hub under its session
name, and a channel message that writes `@<name>` reaches that Agent. The name is matched in lower
case with spaces and punctuation as dashes, so the Agent "Fix Login Redirect" is
`@fix-login-redirect`. When people on the team have Agents with the same name, a mention goes to
the author's own Agent if the author has one. The Agent gets the message when it is idle: your own
message as an instruction, anyone else's as quoted information. Your own message that starts with
`/` once the mention is taken out, like `@fix-login-redirect /compact`, is typed as that command.
The daemon remembers the last mention it fetched, so mentions made while it was stopped arrive when
it starts again.

## Router rules

`router.toml`, beside `config.json`, says what berth suggests when an Agent hits a provider error
or a usage limit. The daemon writes the default file on first start and rereads it when it changes.
Suggestions show under the Agent's row in the sidebar for an Agent that has a provider error or
has finished its turn or waits on you, never for one that is working normally. With the default
`mode = "suggest"` nothing switches until you click one. Settings → Sessions edits the same file:
the fallback chain and each rule's conditions as rows joined by and / or, with not and groups. The
daemon validates and formats what you save, so hand edits and the editor stay in one compact shape.

```toml
schema_version = 1
mode = "suggest"
fallback = ["claude", "codex", "grok"]

[[rule]]
id = "fallback"
name = "Next agent in the fallback chain"
enabled = true
when = { any = [{ provider_error = { kinds = ["usage_limit", "overloaded", "server"] } }, { limit_used = { pct = 95.0 } }] }
then = [{ next_in_chain = {} }]
```

`fallback` entries are `kind` or `kind:model`. `next_in_chain` suggests the first entry after the
Agent's own one, skipping agents whose usage window is at 95% or more, and never wraps around to the
start. Conditions are `all`, `any`, `not`, `provider_error` (`kinds`, `for_secs`), `limit_used`
(`pct`, `window_minutes`), `status`, `kind` and `model`. Actions are `next_in_chain`,
`switch_model = { model = "sonnet" }` and `continue_in = { kind = "codex", model = "..." }`. A
model downgrade such as opus to sonnet happens only through a rule you write, for example
`when = { all = [{ model = ["opus"] }, { provider_error = { kinds = ["overloaded"], for_secs = 60 } }] }`
with `then = [{ switch_model = { model = "sonnet" } }]`. A condition or action berth does not know
never matches, and it is kept as written when the file is saved. A file with a newer
`schema_version` is ignored in favour of the default rule.

`mode = "auto"` applies the suggestion without a click. It stays careful: one switch per Agent per
turn, no more hops along one task's chain of sessions than `fallback` has entries, a running Agent
only after its error has lasted a minute, and a rule that switched an Agent waits
`auto_cooldown_secs` (default 600) before it switches any Agent again. When an Agent moved because
of a usage limit, the session that took over offers to return to the original agent or model once
the limit has reset and its turn has ended. That offer is always a chip you click; berth never moves
a task back on its own. The offer lives in the daemon's memory and is gone after a daemon restart.

## Desktop app settings

The desktop app keeps its own settings in `~/.berth/settings.json` and its key bindings in
`~/.berth/keymap.json`. Both are edited from the app's Settings, which can also open either file in
your editor; hand edits apply while the app runs. `BERTH_HOME` moves the directory.

### Voice

Voice input uses the speech recognition built into the browser engine the app runs in, so it works
only where that engine offers it (Chrome, Edge and Safari do; some desktop webviews do not, and then
the voice controls stay hidden or report that speech recognition is unavailable).

- Hold `mod-shift-space` (the `voice.pushToTalk` action) to dictate: what you say while the key is
  down is sent as a prompt to the Session in view when you release it.
- The Chat view of a Session has a microphone button that works the same way while you hold it.
- The wake word is off until you run `Wake word: listen for the wake phrase for the Session in
  view` from the command palette (`voice.wakeWord`). The microphone then stays on for that one
  Session: say the wake phrase (setting `voice.wakePhrase`, default `hey berth`) and then your
  prompt, and it is sent when you pause. A bar at the bottom of the window shows that the microphone
  is listening; its `Turn off` button, or the same command again, stops it. It does not survive an
  app restart.

## Environment

| Variable | Effect |
|---|---|
| `BERTH_LOG` | `RUST_LOG`-style filter for the daemon log (`BERTH_LOG=debug`) |
| `BERTH_CLOUD_MIRROR_SECS` | cloud-mirror refresh cadence in seconds; `0` turns the follow off |
| `BERTH_DATA_DIR` | data dir (database, config, logs), which is how tests and parallel instances isolate themselves |
| `BERTH_RUNTIME_DIR` | runtime dir holding the socket / endpoint file and pidfile |
| `BERTH_METRICS_SAMPLE_MS` | how often the daemon samples CPU, memory and token use; tests shorten it |
| `BERTH_AGENT_CMD` | replace every agent CLI with one command line, taken verbatim (`make dev AGENT=/bin/cat`) |
| `BERTH_INSTALL_URL` | install-script URL used by `berth upgrade` |
| `BERTH_INSTALL_DIR` | install destination for `install.sh` (default `~/.local/bin`) |
| `BERTH_HOME` | the desktop app's settings directory (default `~/.berth`) |
| `BERTH_REMOTE` | `ssh` destination of another machine; the desktop app shows that machine's Daemon instead of the local one (see below) |

### Berth on another machine

Set `BERTH_REMOTE=user@host` (or a `Host` name from your ssh config) in the environment the desktop app
starts from, then restart the app. The app runs your own `ssh` binary twice: once to start the Daemon on
the server and learn its socket (`berth socket`), installing berth with `install.sh` first when the
server has none, and once as a `-L` forward from a local port or socket to that Daemon socket. berth
stores no ssh credentials. Login must work without a prompt (`BatchMode`): an ssh key, agent or
certificate, and a host key already in `known_hosts`.

The server must be a Linux or macOS machine. Sessions, terminals and their output come from the server;
what the app does on the local disk (opening files, git, worktree cleanup) still looks at this machine's
paths. A server whose berth is older than the app shows as incompatible: run `berth upgrade` there.
Unset the variable to go back to the local Daemon.

Agent PTYs additionally carry `BERTH_AGENT_ID`, `BERTH_API_URL` and `BERTH_API_TOKEN` so hooks and
`berth rename` / `worktree` / `enter` / `spawn` can find their own session. They are scrubbed from plain
terminals.
