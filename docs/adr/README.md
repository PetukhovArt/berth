# Architecture decisions

Decisions that are hard to reverse and came out of a real choice. Each record has its context, the decision, the alternatives it rejected and the consequences. The direction they serve is in [VISION.md](../../VISION.md); the terms they use are defined there until the code has them, then in [GLOSSARY.md](../../GLOSSARY.md).

A new record takes the next number, starts as `Proposed` and becomes `Accepted` when the owner accepts the decision, which may be before the code exists. A record that a later one replaces says `Superseded by NNNN`. The date is in git.

| # | Decision | Status |
| --- | --- | --- |
| [0001](0001-hub-is-the-control-plane.md) | The hub is the control plane; daemons are Machines | Accepted |
| [0002](0002-organization-workspace-project.md) | Organization, Workspace as System, Project, and links between Workspaces | Accepted |
| [0003](0003-rules-as-ast.md) | Triggers and Reactions are AST rules run by a deterministic executor | Accepted |
| [0004](0004-thin-agent-definition-and-model-access.md) | Thin Agent Definitions and configurable Model Access | Accepted |
| [0005](0005-channel-over-session.md) | People read a Channel; the session is for debugging | Accepted |
| [0006](0006-apps-docking-hub-store.md) | Every screen is an App, layouts are per user, and the Hub Store is git | Accepted |
| [0007](0007-workspace-worktree.md) | A session can span the repos of a Workspace | Accepted |
| [0008](0008-versioned-hub-protocol.md) | The hub and the daemon speak a versioned protocol | Accepted |
