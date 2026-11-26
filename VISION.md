# Vision

Berth is a self-hosted agent platform: it runs coding agents from any vendor for you and your team, on your own machines.
On one machine, with no server and no team, it works the same for a solo developer.

A few words first. A harness is the agent program around a model: Claude Code, Codex, Grok, or an agent that speaks the Agent Client Protocol (ACP), such as Cursor, OpenCode or Gemini. The hub is a small server a team runs itself; members' Berth installs connect to it. Capitalised terms below are defined under [Platform terms](#platform-terms).

## Who it is for

Teams of developers who already use several agent CLIs and subscriptions, keep their code on their own network next to their own GitLab or GitHub, and have no platform team to run agent infrastructure. Team adoption comes first.

Solo developers come second, and a team feature that makes the solo path worse is a bug. Today a solo user runs the daemon and the desktop app with no hub at all. Once the hub can be embedded in the daemon, a solo install runs the same code as a team, with one person in it.

## How it differs

T3 Code also runs agents from several vendors on your own machines, but it connects one person's machines: it has guest links, not a team hub with roles, channels and shared rules. Cursor's cloud agents and Claude Code's self-hosted runners can execute on your hardware, but orchestration and conversations stay in the vendor's cloud. Berth keeps the whole control path, from the rules to the channels, on the team's own machines, and works with whichever harness each person uses.

## What Berth is becoming

Today Berth runs agent CLIs in git worktrees, shows which agent needs you in the Inbox, reviews merge requests, and connects a team through a self-hosted hub with channels and roles. The plan is to build these layers on top:

- The hub becomes the control plane: the place that holds the team's rules, Agent Definitions, Channels and permissions. Member daemons and headless `berth-server` containers become Machines that run agents, so a team rule keeps working when its author's laptop is closed.
- Events from outside become work. A Watcher turns a source (a forge, a tracker, a schedule, a webhook, a messenger, Berth's own agents) into Events. A Trigger picks the Events it cares about, and a Reaction runs a chain of steps: post to a Channel, ask in the Inbox, wait, branch, run an agent, run a script. Agents write rules in a strict format, the hub runs them step by step and records each step so a run survives a restart, and a canvas shows every rule and every run.
- The organization model follows how companies already split work. An Organization owns its git namespaces. A Workspace is one team's set of co-dependent repos, what service catalogues call a system, and Workspaces link to each other along their dependencies.
- A session can span the repos of a Workspace, writing to some and reading the base branch of others.
- People read a Channel, not a terminal. Agents post what matters to people, the Inbox collects what waits on a person, and the terminal and transcript stay one click away for debugging.
- Agent Definitions describe roles, such as a pipeline fixer or a reviewer, on top of the harness configuration a team already has. They do not replace it.
- Model Access is configurable: personal subscriptions stay on their owner's machine, and team rules use API keys, an organization gateway or service accounts.
- Every screen becomes an App, including the Inbox, the Channel and the canvas. Users and their agents can fork them, build new ones and place panels where they want them.

## Now, Next, Later

Now: clean docs and this vision; the team hub tried on a real team before more is built on it; a Channel per task as the first view of a session; one Watcher and Event interface for today's event sources; panels each user can drag and arrange, with the layout kept.

Next: a versioned hub protocol; the embedded hub for solo users; Organizations, Workspaces and Projects on the hub; the Hub Store; Agent Definitions and shared setup on the hub; rules with a read-only canvas; Machines, placing agents on them, and container isolation; Model Access; Workspace Worktrees.

Later: editing rules on the canvas and following their runs; Apps and forks of the default Apps; cloud sandboxes as Machines; a hosted hub for many organizations, as an option a team may choose.

## What Berth will not be

Berth does not run its own model and does not resell tokens: agents use the vendor CLIs and the accounts their users already have. It does not require a cloud service: the self-hosted hub stays the default and keeps the control path on the team's machines, and a hosted hub, if it comes, is a choice, not a dependency. It is not a drag-and-drop automation builder: rules are text an agent can write and a person can review, and the canvas is a view of that text. It does not replace the harnesses: Claude Code, Codex and the others stay the agents, and their own configuration stays where it lives.

## Open questions

- Licensing and a hosted offer. The core is MIT today; whether a hosted hub becomes a paid offer is not decided.
- An Organization owning several git namespaces (for example an internal GitLab group and a public GitHub organization) is the working assumption and is open to change.

## Platform terms

These terms name things the code does not have yet. They move to [GLOSSARY.md](GLOSSARY.md) when the code lands.

- Watcher: an adapter that turns one outside source into Events.
- Event: one fact from a source, in one shape for every source.
- Trigger: the kind of Event, its sources and an expression filter that start a Reaction. Today's scheduled prompts and webhooks become Triggers.
- Reaction: the chain of steps a rule runs when its Trigger fires, from a closed set of step kinds.
- Organization: the company on a hub; it owns git namespaces, members, roles and Workspaces.
- Workspace, in its new meaning: the co-dependent repos one team owns and changes together. Today a Workspace is a personal group of Projects in one app; it becomes a Workspace of the embedded hub.
- Project, in its new meaning: one repo, known on the hub by its normalized git remote, or one path inside a monorepo; it belongs to exactly one Workspace. Today a Project is a repo added to Berth on one machine; it becomes that Project's checkout.
- Workspace Worktree: one checkout for a session across the repos of a Workspace, each repo either writable on the session branch or read-only on its base branch.
- Channel, in its wider meaning: the thread people read for each task or Workspace Worktree, where agents post summaries, questions and results. Today a Channel is a team hub stream; it becomes the first view of every session, solo users included.
- Agent Definition: a role an Agent plays, with its harness, model, instructions, setup bundle, machine needs, limits and account. An Agent runs from a Definition on a Machine.
- Machine: a daemon registered on a hub, with labels that say what it can run.
- Isolation: how an Agent is separated from others on its Machine: on the host, in a container, or later in a cloud sandbox.
- Model Access: where an agent's model calls are billed and routed: a personal subscription, an API key, an organization gateway or a service account.
- App: a screen built from a folder of web files and a manifest, shown in a dock panel, inside a Channel or on the canvas.
- Hub Store: the git repository a hub keeps per Organization for its rules, Definitions, Apps and setup bundles.

## Decisions and background

The decisions behind this page, with the options they rejected, are in [docs/adr](docs/adr/README.md). The industry essay that started them is [docs/vision/agent-platform-2027.md](docs/vision/agent-platform-2027.md).
