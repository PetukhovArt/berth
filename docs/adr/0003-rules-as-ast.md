# 0003. Triggers and Reactions are AST rules run by a deterministic executor

- Status: Accepted

## Context

Berth already reacts to a few outside events, each with its own code: watched merge requests, scheduled prompts and webhooks, inbound messenger prompts, and mentions in hub channels. None of them can be configured beyond its built-in behaviour, and the only reaction is typing a prompt into an agent. Teams want their own: "when a pipeline fails, start the fixer; if it has not finished in an hour, ask me".

Drag-and-drop automation builders produce rules that cannot be reviewed or diffed. Workflow engines that replay code need that code to be deterministic, which breaks when agents edit it. Scenario builders that store a trigger and a reaction as a tree are a proven pattern in operator-facing products, and an editor can show such a tree as a graph.

Berth also has a scheduling concept called a trigger today, which this decision reuses with a wider meaning.

## Decision

A rule is a Trigger and a Reaction. A Watcher adapts one source into Events of one shape. A Trigger names the Event kind, its sources and an expression filter, with a minimum interval between firings. A Reaction is a chain of steps from a closed catalogue: post to a Channel, ask in the Inbox, wait, branch on a condition, run an agent from a Definition, run a script on a Machine.

The source of truth is an AST stored as JSON, with a schema generated from the Rust types. Agents and people write YAML that maps one to one onto it and check it with a `berth` command that reports errors by line. The canvas reads and writes the same AST. The hub runs Reactions with a durable, deterministic executor: every step's result is recorded, a run survives restarts, and it continues from the first unfinished step. Flexible work happens only inside the agent and script steps, which run on Machines, never on the hub.

Publishing goes through hub roles: writing personal rules, writing team rules that may start agents on any team Machine, and letting agent-written rules go live without review, which no built-in role holds. A rule runs with the permissions of the person who published it. A rule an agent wrote or changed waits for its owner to approve the diff in the Inbox.

Today's scheduled prompts and webhooks become Triggers with a "send prompt" Reaction. Their commands stay as short forms that write a rule, and their stored rows migrate.

## Alternatives

- Agents write TypeScript through a typed builder that compiles to the AST. Good for agents and autocompletion, but two representations of one rule and a Node toolchain on the author's machine. It can be added later on top of the AST.
- Reactions as executable code in the style of replayed workflow functions. Most flexible, but it needs a JavaScript runtime on the hub, determinism rests on discipline, and the canvas cannot show the steps reliably.
- Rules stored only in the hub database and edited only on a canvas. Closest to a visual builder, but changes have no diff or review and agents cannot write them comfortably.
- No approval for agent-written rules. Less friction, but unattended work could appear without a person ever looking at it, and Event text can carry prompt injection into an agent step.

## Consequences

- The step catalogue is a contract: adding a step kind is a schema change and a release, which is the price of determinism.
- Today's merge request Watch becomes the first forge Watcher, and the other three sources become Watchers too.
- A new kind of source needs a Watcher adapter; generic webhook and HTTP polling Watchers cover most cases without code.
- The canvas is a view and an editor of text, not a separate model.
- The old scheduling concept is renamed in place and needs a data migration.
