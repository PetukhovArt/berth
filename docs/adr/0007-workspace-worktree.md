# 0007. A session can span the repos of a Workspace

- Status: Accepted

## Context

A Workspace is a System of co-dependent repos (0002), and a feature in a System often touches several of them: a new field goes through an SDK, shared types and a client. Today a session lives in one worktree of one repo. Giving every repo of a Workspace its own branch for every session is wasteful when the session only needs to read most of them, and reading a repo's main checkout is unsafe because someone else's branch or uncommitted work may be there.

## Decision

A Workspace Worktree is one folder per session with an entry for each repo of the Workspace the session uses. Each repo is in one of two modes. In `write` mode it gets its own git worktree on the session branch. In `read` mode it uses a shared read-only worktree pinned to the base branch from the project's worktree base setting, kept current by fetching, and used by every session at once. A repo can move from `read` to `write` during a session. The agent's working directory is the shared folder. Merge requests are created per repo and linked, so their state and pipelines show together. A single repo is the same mechanism with one entry.

## Alternatives

- One repo per session; work across repos uses several sessions coordinated by a Reaction or by spawning siblings. Simpler, but no agent sees the whole change.
- Multi-repo work only through a monorepo or submodules. Nothing new to build, but most companies with many repos are left out.
- Every repo in `write` mode always. Simple rules, but many worktrees and branches nobody needs.

## Consequences

- The worktree layout on disk changes for multi-repo sessions and needs care with existing worktrees and their paths.
- Read-only worktrees are shared state that needs updating and cleanup.
- Linked merge requests need their own view and their own state.
