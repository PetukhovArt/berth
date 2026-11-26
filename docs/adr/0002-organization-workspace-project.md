# 0002. Organization, Workspace as System, Project, and links between Workspaces

- Status: Accepted

## Context

Rules, Agent Definitions and the canvas need a scope that every member of a team names the same way. A Project in Berth today is known only by its local path on one machine, and a Workspace is a personal grouping on one client, so two members have no shared name for the same code. Companies already describe their code as service catalogues do: business domains, systems of co-dependent repos owned by one team, the repos inside them, and dependencies between systems. Projects increasingly span several repos, and some live inside one monorepo.

## Decision

An Organization is the company on a hub: it holds its members, roles, Workspaces and Hub Store. It owns one or more git remote namespaces, so an internal GitLab group and a public GitHub organization can belong to one company.

A Workspace is a System: the co-dependent repos one team owns and changes together. A Project is a repo, identified on the hub by its normalized git remote, or a path inside a monorepo, and it belongs to exactly one Workspace. A member's local Project is that Project's checkout on their machine. A library several Systems use belongs to the one that owns it; the others depend on it.

Workspaces link to each other along their dependencies, from provider to consumer. Events flow along a link by default; reading the provider's repos and starting work in another Workspace are granted explicitly through roles. Inside a Workspace, Events and agents move freely.

The local Workspace becomes a Workspace of the embedded hub; existing local Workspaces move into it with a migration that keeps their sessions, pins and settings.

Nobody draws the map by hand. An agent proposes it from forge groups, dependency manifests between repos, code-owner files and service catalogue files where they exist, and a person confirms it on the canvas.

## Alternatives

- A separate team-level grouping next to the local Workspace. No migration, but two similar groupings in one product, and the word would mean two things.
- A hub project keyed by a single git remote. Fails for systems of several repos.
- A repo in several Workspaces. Easier to browse, but rules fire more than once and ownership blurs.
- Workspace equals forge group, one to one. No setup, but groups follow org charts and history rather than systems, and GitHub has no nested groups for repos.
- Scope rules to channels. A channel is a stream of messages, not a body of code; "every merge request in repo X" does not fit.

## Consequences

- Every rule, Definition, App and Channel has a scope among Organization, Workspace, Project or a single member.
- The local Workspace changes meaning on disk and needs a migration, not a rename.
- A monorepo needs path-level Projects from the start.
- Whether a hosted hub serves many Organizations is a later decision; a self-hosted hub serves one.
