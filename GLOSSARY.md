# Berth

Terminal multiplexer for AI coding agents: a daemon owns the PTY sessions; the desktop app and its phone client attach and detach without losing work.

## The tree

**Workspace**
A named group of Projects. Exactly one Workspace is open per client connection; the others keep running in the background.
_Avoid_: project group

**Open Workspace**
The Workspace a client connection is scoped to (`OpenWorkspace`). Two connections can have different Open Workspaces.
_Avoid_: selected workspace, current workspace

**Default Workspace**
The built-in Workspace `default` — present in every install, cannot be deleted.

**Project**
A git repository registered with berth. Lives inside a Workspace.
_Avoid_: repo

**Worktree**
Where a Session runs: the Root Worktree or a real `git worktree`. Two agents in two Worktrees do not interfere.
_Avoid_: branch (a branch is a property of a Worktree, not the Worktree itself)

**Root Worktree**
The Project's own checkout. Every Project has exactly one.
_Avoid_: main checkout, root row

**Worktree Dir**
The convention for where new Worktrees go: `<repo>/../<repo-name>-worktrees/<branch>`.
_Avoid_: sibling dir

**Session**
One row in the sessions list: an Agent or a Terminal Session, bound to a Worktree and backed by a PTY Session in the Daemon.
_Avoid_: tab

**Agent**
A Session running an agent CLI (`claude`, `codex`, `cursor-agent`).
_Avoid_: the claude

**Agent Kind**
Which CLI an Agent runs: `claude`, `codex`, or `cursor`.
_Avoid_: harness, agent type

**Terminal Session**
A Session that is a plain shell in the Worktree's directory — no agent.
_Avoid_: shell tab

**PR Session**
A Claude Agent created with a pull request URL (`pr_url`) and constrained to work only on that PR. Starts in the Root Worktree.

**Link**
A URL previously pinned to a Worktree. A legacy of older versions: existing rows stay visible and editable; new ones are not created.

**Pin**
A Project the user pinned in the desktop app's tree. Pinned Projects sit at the top of their Workspace; their rank among pinned Projects is held by the Daemon (`Project.pin_order`, set as one ordered list by `SetProjectPins`), so every client shows the same order. Distinct from a Link ("a URL pinned to a Worktree"), which is legacy vocabulary.

**Archive**
The state of an Agent whose PTY is released and whose row is moved to the ARCHIVED group. Coming back resumes the conversation.
_Avoid_: deletion (Archive is reversible)

**Orphaned Session**
A Session whose Worktree was deleted. The row is gone from the tree, but its CLI session id is kept, so the conversation can be Resumed in a Worktree that still exists. Different from Archive (the Worktree is still there, and the row comes back where it was) and from Disconnected (the PTY died, the row did not). The `OrphanedSession` type also carries live rows — conversations whose checkout still exists, marked `live`, resumable in any other Worktree the same way.
_Avoid_: archived session, lost session, dead session

**External Session**
A Claude Code or Codex conversation berth did not start, found on disk by the desktop app and shown in the tree beside the Sessions of the Worktree it ran in. Archiving one stores only a mark in the Daemon (`archived_external_sessions`, keyed by Agent Kind and the CLI's session id); the transcript is never touched, and a transcript written after the mark brings the row back out of the archive.
_Avoid_: foreign session, outside session

## Processes

**Daemon**
The background process that owns every PTY Session, the store, git operations, and agent statuses. Survives every client closing, the desktop app included.
_Avoid_: server

**PTY Session**
The Daemon-owned pseudo-terminal of one Session, together with its child process.

**Attach**
Connecting a client's terminal view to a Session: the Daemon replays scrollback, then streams live output. Detaching does not kill the process.
_Avoid_: drill in

**Resume**
Restoring an Agent from its stored CLI session id; a fresh session when the id is gone.
_Avoid_: restore, restart (a restart is a new process; Resume continues the conversation)

**Sampler**
The Daemon task that takes the one periodic reading every `GetMetrics` caller reads, the desktop app's metrics panel among them. It is the only owner of the CPU delta and the transcript byte offsets, so `GetMetrics` answers from its last sample rather than sweeping.
_Avoid_: poller, metrics collector

## Transport and runtime files

**Daemon Socket**
The channel a client (the desktop app or a `berth` command) talks to the Daemon over. On unix an `AF_UNIX` socket in the Runtime Dir; on Windows, which has none, a loopback TCP listener plus a bearer token. One or the other, never both — callers above the transport see a single API.
_Avoid_: the IPC port, the server, the API (the Hook Receiver is the HTTP one, not this)

**Runtime Dir**
The per-user directory holding the Daemon Socket, the Pidfile and the Endpoint File. On unix its `0700` mode is the whole authorization boundary — a connect that succeeds is already a connect from this user. Overridable, which is how a dev instance per checkout stays off the real Daemon.
_Avoid_: temp dir, socket dir

**Endpoint File**
`<port>\n<token>\n` beside the Pidfile. Windows only, and load-bearing there: a TCP port is not a fixed name the way a socket path is, so this file is how a client finds the Daemon Socket at all. Rewritten whole, never appended — a shorter token must not leave the tail of the old one behind.

**Pidfile Lock**
The exclusive lock a Daemon holds on its pidfile (`flock` on unix, `LockFileEx` on Windows). It, not the pid inside, is what refuses a second Daemon on one Runtime Dir — a pid outlives the process it names. The Windows lock sits on a byte far past the end of the content, because file locks there are mandatory and a locked byte 0 would make the pidfile unreadable to the two readers that need it.
_Avoid_: pid check

**Daemon Setsid**
The unix step that puts a spawned Daemon in its own session with no controlling terminal, so it outlives the client that started it and no keystroke reaches it. Windows has no sessions to detach from; the equivalent is spawning the Daemon detached and windowless.

**Version Skew**
A client and a Daemon built from different code, so their protocol versions disagree — the ordinary state of a checkout where a live Daemon predates the build you just made. The client names which side is older instead of failing opaquely, and the Daemon keeps it distinguishable from a rejected token.
_Avoid_: version mismatch, stale daemon (a stale Daemon is merely older, and may still speak the same protocol)

## Statuses

**Agent Status**
The Daemon-owned state of an Agent: Fresh, Running, Finished, Needs Feedback, Terminated, Disconnected.

**Fresh**
The agent has not run a turn yet.
_Avoid_: gray

**Running**
A turn is in progress.
_Avoid_: yellow, thinking, mid-turn

**Finished**
The turn is complete.
_Avoid_: done, green

**Needs Feedback**
The agent waits on a human: a permission prompt or a question.
_Avoid_: red, waiting on me
**Feedback Request**
What a Needs Feedback agent waits on, as its hook reported it: a permission with the tool and its command or path, or a question with its options. Held in memory by the Daemon, cleared when the status leaves Needs Feedback, and answered by typing into the PTY.


**Terminated**
The agent process died mid-run.

**Disconnected**
The Daemon restarted while the agent was live.

**Unseen**
The flag "a turn finished and nobody has looked". Not the same as Finished: this is the read/unread axis.
_Avoid_: unread ≠ done

**Mark Seen**
Clearing Unseen — happens on Attach to the session.

**Rollup**
Parent rows (Worktree, Project, Workspace) show their children's worst status: red beats yellow; violet whenever anything Unseen is underneath.
_Avoid_: parent dot

**Stop Gate**
The rule that a turn does not count as Finished while its subagents are still alive.
_Avoid_: "the session went green too early"

## Agents and hooks

**Managed Hooks**
The hook groups berth writes into the agent CLI's config at every spawn; user hooks are preserved.
_Avoid_: the hooks berth installs

**Hook Receiver**
The Daemon's loopback HTTP endpoint that the agent CLIs' hooks report events to. Not MCP.
_Avoid_: hook server

**Auto-Title**
A session left on its default name gets a 3–4 word title from the agent after the first prompt. A name a human typed always wins.
_Avoid_: auto name, self-rename

**Worktree Relocation**
Moving a Session to another Worktree via `berth worktree`, or to another Project's checkout via `berth enter`: the row moves at once; the CLI restarts inside the Worktree with the conversation resumed. A move across Projects also moves the Claude transcript into the new checkout's directory under `~/.claude/projects`.
_Avoid_: move into a worktree

**Starting Prompt**
The first prompt passed to the CLI as an argument on a cold spawn. Never persisted — Resume does not replay it.
_Avoid_: first prompt

## Daemon mechanisms

**Prewarm Pool**
Pre-booted agent CLIs that get adopted when an Agent is created, so Attach lands on a booted screen.
_Avoid_: warm spares as "sub agents" (they are not subagents)

**Warm Spare**
One pre-booted CLI from the Prewarm Pool: a real process with no store row.

**Idle Reaper**
The mechanism that kills idle PTYs in Worktrees nobody is viewing. Working agents and ones waiting on a human are spared; a reaped Agent resumes on the next Attach.
_Avoid_: auto suspend

**Checkpoint**
A commit of an Agent's Worktree files, taken at each prompt under `refs/berth/checkpoints/<agent>/<number>` without touching the branch, the index or the stash. Restoring one puts the files back and first checkpoints what it replaces.
_Avoid_: snapshot, backup

**Channel**
A named stream of messages on the team hub (`hub_url`) that people and Agents post to and read, with an unread count per member. The Inbox lists Channels with unread messages and opens them as a view beside the Sessions. Not the Transport.
_Avoid_: chat room, topic

## Cloud sessions

**Cloud Session**
An Agent row whose work runs in Claude's cloud rather than a local PTY.

**Cloud Launch**
Creating a Cloud Session: the task goes to the cloud as a process argument — no place for secrets.

**Cloud Teleport**
A repeatable snapshot pull of the cloud transcript and branch into a local session — a fork, not a live link. Always in a Cloud Worktree.

**Cloud Worktree**
The `cloud-<id>` Worktree a Cloud Session is re-homed into before Attach or Teleport — the cloud CLI switches branches.

**Cloud Mirror**
A periodic re-Teleport so the pane follows the cloud agent. Ends on the first keystroke into the pane.
