# 0008. The hub and the daemon speak a versioned protocol

- Status: Accepted

## Context

Berth releases every week. With the hub as the control plane (0001), every member's daemon talks to one hub, and members update on different days. A self-hosted hub is updated by whoever runs it, often later than the members. Without a contract between versions, one early update breaks a member's link to the team, or a hub update breaks everyone who has not updated.

## Decision

The protocol between the hub and the daemon carries a version. A hub accepts daemons of its own minor version and the one before it. A daemon that finds a hub older than that refuses with a message that names both versions and what to update. A change that cannot stay compatible across one minor version waits for the next one, or ships behind a capability both sides advertise.

## Alternatives

- No versioning. Works only while one person runs every side, which is not the case for a team.
- Exact version match. Safe, but a team must update everyone and the hub at the same moment every week.
- Unlimited backward compatibility. Easiest for users, but the hub would carry every old shape forever.

## Consequences

- Every change to hub messages states the oldest daemon version it serves.
- Release notes say when a hub update is needed before a daemon update.
- Tests cover a hub talking to a daemon one minor version older.
