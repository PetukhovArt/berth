# 0006. Every screen is an App, layouts are per user, and the Hub Store is git

- Status: Accepted

## Context

Teams want screens Berth does not ship, such as a review queue with service-level targets across Workspaces, and want to change the ones it does. Today users can change settings, key bindings and the theme. The dock has tabs and splits, but on the desktop panels cannot be dragged and the split layout is not kept.

Comparable products let users restyle the app, let an agent publish a theme, and show sandboxed pages from agents inside the chat; none lets users fork the product's own screens. MCP Apps is now an official MCP extension, stable since its 2026-01-26 specification: a server declares HTML resources under a `ui://` scheme, the host renders them in a sandboxed iframe, the two talk over a defined message channel, and hosts advertise support during capability negotiation. Claude supports it.

Rules, Agent Definitions, Apps and setup bundles all need versions, history, forks and review on the hub, which today stores members, channels and events in SQLite and has no file storage.

## Decision

Every screen is an App: a folder of web files with a manifest. The Inbox, the Channel and the canvas ship as default Apps. Users and their agents build new Apps and fork the default ones; publishing goes through hub roles. A client shows an App in a dock panel, inside a Channel entry, or as a node on the canvas, in a sandboxed frame. The bridge between an App and Berth builds on MCP Apps, so interfaces from MCP servers work in Berth and Berth's own Apps follow a public contract. Through the bridge an App reads sessions, Events and Channels within the user's permissions, posts to a Channel and starts Reactions.

A fork is a branch of the App's folder. After a release Berth offers to rebase the fork on the new default and an agent resolves conflicts. A fork that fails the bridge contract is replaced by the default App for that user, with a notice.

The dock is fully flexible for every user. The layout is a file the user owns, and an agent can change it on request.

The Hub Store is one git repository per Organization, held by the hub, with folders per Workspace for rules, Definitions, Apps and setup bundles. The hub checks roles on push by path. An agent-written change waits on a branch until its owner approves it in the Inbox. The store can be mirrored to the company's forge. Files such as uploads and agent reports go to object storage later.

## Alternatives

- Apps only for new screens, with Berth's own screens built in. Lower risk to the core screens, but teams cannot change what they use most.
- No forks, only extension points in the default screens. Stable updates, less freedom.
- Forks as full copies with no updates. Simplest, but a fork falls behind and breaks unnoticed.
- A private bridge format. Full control, but no MCP App would work in Berth and the contract would have to be designed from scratch.
- The store as a repository in the company's forge. Familiar review, but two approval paths and permissions split between forge and hub, and no solo path without a forge.
- The store in the hub database with its own version table. One place, but versions, branches and rebases written by hand.

## Consequences

- The default screens must be written against the same bridge as user Apps, which means rewriting the current screens as Apps.
- Each release has to keep the bridge contract or say which part changed, so forks can be checked.
- The hub runs a git service with permission checks on push.
