# 0001. The hub is the control plane; daemons are Machines

- Status: Accepted

## Context

Berth is going from a tool that supervises agent CLIs on one machine to a self-hosted agent platform for teams, without losing the solo path. Team rules, listeners and long-running Reactions cannot live on a member's laptop: they stop when the laptop closes. Agents that run with nobody watching need permission modes without prompts, and on a shared server such an agent can reach other agents' worktrees and secrets.

Most competing products that call themselves self-hosted run only the tool execution on the customer's hardware; orchestration and conversations stay in the vendor's cloud. Keeping the control path on the team's own machines is the position Berth takes.

## Decision

The hub holds the team's state that must outlive any one machine: Organizations, roles and permissions, Agent Definitions, rules, Channels. Daemons on members' machines and headless `berth-server` boxes register on the hub as Machines with labels (operating system, GPU, network reach, repos already cloned), and the hub places Agents on them by a Definition's requirements.

A solo install embeds the hub in its daemon, with the same code and one Organization.

An Agent runs with an Isolation chosen by its Definition or rule: `host` for work a person attends on their own machine, `container` with only its worktree, its own secrets and a network policy, and later `sandbox` through cloud providers behind the same Machine interface. Team rules that run unattended use `container` unless an admin's policy says otherwise.

## Alternatives

- The daemon stays the centre and the hub stays a thin message bus. Simpler and closer to today, but team rules stop with their owner's machine, and N agents across M machines is out of reach.
- Decide later and grow the hub feature by feature. Cheaper now, but the hub-to-daemon protocol would have to break once the control plane is designed.
- No per-Agent isolation; unattended agents run only on dedicated servers. One agent in a no-prompt mode could then touch another's worktree or secrets.
- Cloud sandboxes first. Conflicts with self-hosting as the priority.

## Consequences

- The hub becomes the service that must stay up and be backed up, not an optional chat relay.
- Solo and team share one code path; the embedded hub has to be as reliable as the daemon.
- Machines need registration, heartbeats and placement, and a container runtime on the boxes that run unattended work.
- The hub has to be tried on a real team before more layers are built on it.
