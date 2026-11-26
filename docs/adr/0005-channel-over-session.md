# 0005. People read a Channel; the session is for debugging

- Status: Accepted

## Context

Opening a session today shows the agent's terminal, or a chat view that renders the agent's own transcript: every model message and tool call. An hour of work is hundreds of entries, and a teammate cannot tell from them what was done. Hub channels already carry messages that agents post themselves, can be keyed to a task, merge request branch or worktree, and already mirror an agent's questions so they can be answered there. The messenger bridge already sends a summary of a finished turn.

The industry essay behind this decision calls the people-facing thread a "conversation". Berth already uses that word for an agent CLI's transcript (resuming a session continues its conversation), so the thread keeps the name Channel, which already means a stream of messages people and Agents post to and read.

## Decision

Each task, or each Workspace Worktree, has a Channel. Agents post to it with a `berth` command or an MCP tool, in kinds a client can show differently: a summary, a question, a result, a request for approval. When an agent posts nothing in a turn, Berth posts a summary of its last message, so every harness works. Colleagues and other agents write in the same Channel. The Inbox is built from Channel entries that wait on a person.

The Channel is the first view of a session. The terminal and the transcript stay as tabs for inspecting and debugging. A solo user gets the same through the embedded hub.

## Alternatives

- A new Conversation entity separate from channels, with channels left as team chat. Cleaner boundaries, but two similar message streams, the Inbox would merge both, and "conversation" would mean two things.
- Keep the name Conversation for the thread and rename today's conversation to transcript across the code and the glossary. Keeps the essay's word, at the cost of a rename with no gain for users.
- Keep the transcript chat view as the main view and add generated summaries. Cheapest, but it keeps showing the agent's internals as the interface.

## Consequences

- Agents need to know when and what to post; the instructions Berth injects grow by that.
- The automatic summary keeps harnesses without a posting tool working, at lower quality.
- Every session has a Channel, so the hub stores more, including for solo users.
- Channel stops being a team-only feature: a solo user has Channels on the embedded hub.
