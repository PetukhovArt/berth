# Agent platform 2027

Background for [VISION.md](../../VISION.md): an outside essay about where agent platforms are going, with a glossary of its terms. It describes the industry, not Berth; the Berth decisions it led to are in [the ADRs](../adr/README.md).

## Glossary

### Agents and runtime

**Temporal**
A service that keeps an agent's long-running work from being lost when a server goes down. Temporal records the result of every step (a model answer, a tool call) and, after a failure, resumes the work on another server from the first step that did not finish.
_Not to be confused with_: a Claude Code session transcript. The log is similar, but after a failure a person resumes it with `claude --resume`.

**Actor**
The unit of the actor model: its own state, its own message mailbox, its own isolation. Actors talk to each other only through messages. Example: Cloudflare Durable Objects (DO).

**Harness**
The program around a model: the agent loop, tools, hooks, extensions. Claude Code, Codex and Cursor Agent are harnesses.
_Not to be confused with_: the model (Opus and so on).

**Agent loop**
The repeat of "the model answers → calls a tool → gets the result" until the turn ends.

**Self-improving harness**
A harness in which the agent writes its own hooks and extensions on the fly.

**Tenant**
A customer of the platform with isolated data and settings: a company, a team or a person.

**Agent definition / instance / session**
A definition describes an agent (prompt, tools, code). An instance is a running agent made from a definition. A session is one history of an instance's work.

**Mega-agent**
One agent with every tool and integration at once.
_Not to be confused with_: the "Grokbot" model, where each agent has a narrow set of tools defined in code.

**MCP**
Model Context Protocol: the standard a harness uses to connect external tools and data (Gmail, Google Drive, search).

**Turn**
One request to an agent: from the message to the end of its answer, with all its tool calls.

### Computers and files

**Sandbox**
An isolated VM or container in which an agent runs code.

**Checkpoint**
A saved state of a sandbox. Clones with the same environment start from it quickly.

**Swarm**
Many agents working on one task in parallel.

**POSIX filesystem**
A filesystem with ordinary Unix semantics: directories, permissions, `open`/`read`/`write`. A cloud version mounts like a normal disk. Examples: JuiceFS, filesystems on top of S3.

**RBAC**
Role-Based Access Control: permissions are granted to roles, and roles to people or agents.

**`jj`**
Jujutsu: a version control system compatible with git.

### Conversation and inbox

**Agent session**
The agent's full internal history: model messages, tool calls, their results.

**Conversation**
What the user sees, like a Slack channel or an email thread. The agent posts to it through a tool call.
_Not to be confused with_: the agent session; the session is for debugging, like reading code.

**ADE**
Agentic Development Environment: an app that wraps coding agent sessions (T3, Cursor and so on).
_Not to be confused with_: an IDE.

**Innovator's dilemma**
Clayton Christensen's term: a market leader finds it hard to replace a successful product with a new approach, and newcomers overtake it.

**Inbox**
A short list of what waits on a person: approvals and questions from agents.

### Events and workflows

**Listener / monitor**
Code or an agent that watches a stream of external events and turns them into signals for an agent.
_Not to be confused with_: cron, which only starts an agent on a schedule.

**WSS**
WebSocket over TLS: a persistent connection over which a server sends events.

**Kafka**
An event stream broker: producers write to topics, consumers read them.

**CDC**
Change Data Capture: a stream of changes to database rows (insert, update, delete).

**Webhook**
An HTTP request an external service sends to your address when an event happens.

**Durable workflow**
A multi-step process that survives restarts and can last days or weeks.

**Drag-and-drop automation**
A workflow assembled from blocks with the mouse in a visual editor (n8n, Zapier).
_Not to be confused with_: a workflow defined in code.

### Apps

**App / artifact**
An interface that lives in the user's filesystem and that an agent can change on request. The author treats Claude Artifacts, ChatGPT Apps and Replit apps as the same thing.

**Generative UI**
An interface the model creates on the fly: JSON made of ready components, an HTML-based DSL, or a whole app.

**DSL**
Domain-Specific Language: a small language for one task.

**Vertical SaaS**
SaaS for one industry (accounting, healthcare and so on).
_Not to be confused with_: horizontal SaaS for every industry (Slack, Excel).

**Infinite canvas**
A workspace without edges that you pan and zoom (tldraw, Figma). ComfyUI shows its image generation graph this way.

**Metabase**
A BI tool; its dashboards are a grid of resizable panels.

## Agents as actors

The status quo for most scalable agent workloads is to deploy agents using some form of Temporal. I think this will shift toward actors that allow each agent harness to have a fully independent runtime. The actor model provides runtime isolation, which is necessary for self-improving harnesses, where self-improving refers to an agent being able to write its own extensions or hooks on the fly. Actors help because they allow different implementations of the agent loop to run safely on the same platform backend, even within a single tenant.

The platform should support a variety of agent configurations, separating the definition of an agent from its instances and sessions. Some users may want a mega-agent, but others may prefer bespoke agent definitions. Most notably, Anthropic argues for one mega-agent, but its revealed preference in [Formalizing Fermat's Last Theorem] suggests something different: each agent had access to only a shell and a file editor. It's unclear to me whether, even if Opus can distinguish the right tools, the researcher would have wanted agents loaded with Gmail, Google Drive, and internet search MCPs when commanding this swarm. Therefore, even within companies advocating for one mega-agent, my observation is that their revealed preference is closer to a code-defined "Grokbot" model.

*Relevant: Cloudflare DO, Pi-Durable x Extensions, Vercel Eve, Rivet, DeepSeek Agent*

## Environments and computers

There are two types of computers the platform needs to support: clones of checkpointed sandboxes across a variety of providers, and people's local computers. N agents should be able to work with M computers (e.g., one agent spinning up five sandboxes to experiment with different configurations, one VM per coding agent with a full environment, or 1,000 agents using 100 VMs in parallel as a swarm). These environments should be configurable agentically.

*Relevant: Claude Remote, Cursor Agent, Cua AI*

## RBAC-ed filesystem

Swarms of agents will operate on a shared POSIX filesystem in the cloud, with access controls attached to the filesystem separately from the computer. This layer acts as part GitHub, where agent-generated code, extensions, and definitions live; part Google Drive, where uploaded, downloaded, and generated files and "apps" live; and more. Since it's effectively a remote shared filesystem, you could host `.env` files and have each directory use `.git` or `jj` for version control. There will be various configurations of RBAC that organizations will want, whether it's agents like humans, or permission granted based on who initiated the particular turn, or a mixture of both.

*Relevant: Archil, S3 Filesystems, JuiceFS*

## Separation of session and conversation

The status quo for most coding agent applications is to expose the agent session, including its intermediate tool calls, directly to the user. I think this is wrong.

What the user interacts with should be abstracted from the core agent session as a "conversation." Your app should natively have a concept of a conversation, at the same level of abstraction as a Slack channel, iMessage chat, WhatsApp conversation, or email thread. Content intended for the user should be posted through a tool call rather than by directly exposing the assistant messages from the underlying agent session.

Direct access to agent sessions should be akin to reading the codebase: a way to inspect and debug the system, rather than the primary user interface.
ADEs created in 2025–2026 that wrap coding agent sessions (e.g., T3) will face the same kind of dilemma Cursor faced in 2025–2026. I suspect most will not survive this transition because of the innovator's dilemma.

*Relevant: Duet / Grokbot / Dots*

## Inbox

Rather than reading through conversations, users will increasingly interact with a focused inbox of approvals and questions. The inbox should also support tenant-specific apps for reviewing intermediate work relevant to the business that requires human action.

*Relevant: MS Today by @nicbstme*

## Listeners, monitors, single-tenant data

Agents should be able to define streams of events from the external world to listen to on a per-tenant basis (WSS, Kafka streams, CDC, etc.) and translate those events, deterministically or agentically, into signals they can act on. These listeners should be buildable on the fly rather than requiring built-in support from the agent platform. This supports agents to define any form of integration to the outside world either as code or as agent. Anything from "I want my Capy Captain / Dots / Duet / main Grok Bot actually have a function for which every email comes in, have JEV filter over it, and trigger an agent run on that email" / "Nevermind, I actually want it to be batches of emails across all my accounts" / "Actually, I want emails from senders with acmecorp.com to be handled for the next week because we need to watch for specific emails from them but I need it to be time sensitive". Right now, the industry (Grok bot, Muse, Instinct, etc) really only supports hourly cron jobs that trigger an agent from a generalized platform perspective, and if not otherwise write bespoke webhook integration as plugins into these platforms - listeners and monitors as a concept is a strict superset that supports tenants to flexibly configure the type of event-driven mechanism the actual core agent should reason over.

*Relevant: Claude Code Monitor, Grokbot future feature*

## Workflows

Agents should be able to define durable workflows on a per-tenant basis. Many business and enterprise workflows operate over long horizons and involve a mixture of agentic and deterministic steps.
The paradigm of shoddily defined drag-and-drop workflow automation (n8n, the original Zapier) is inferior to writing custom code. Given that the agent platform centralizes data flows through APIs and MCPs, it should also be the place where workflows are defined. But these should be code-defined workflows rather than drag-and-drop automations.

*Relevant: Zapier Zap, Rivet Workflows*

## Apps

Aka Artifacts.
The status quo is that people view Replit as one category, separate from Claude Artifacts, ChatGPT Apps, or embedded widgets in ChatGPT. I think this is wrong. These all converge on the same idea of an app. All interfaces should be apps in your filesystem that users can customize. For example, the entire `/conversations` page should be defined as an artifact. Each user would get a "clone" of the codebase in their filesystem that can be extended or modified as needed. This parallels how an OS ships with default applications.

"Generative UI is going to come with default design components and be represented as JSON to be token-efficient!"

"No, we're going to have a separate DSL that's an HTML hybrid to be more flexible."

"No, we're going to have everything be a Rune App."

I think the answer is that the platform will simply support all of these under the concept of an App, and the specific representation won't matter as much.

The agent platform becomes the OS within which all these apps are built. At first glance, it's not obvious why we need an OS within an OS.

The core reason, and the value to the user, comes down to the customizability and extensibility of the application layer built on top of the underlying OS.

Consider how ten different accounting companies review tax forms. Every company within a vertical shares the same "noun" of "review XYZ," but the process by which each company evaluates and understands the information is bespoke. Ideally, these companies could fork Excel and create bespoke interfaces for their workloads.

This is exactly the assumption behind vertical SaaS: that companies within a vertical share enough common workflows to justify common ways of representing them. But I think this idea is broken as a venture thesis, at least in its original form. The best vertical SaaS products eventually evolve into configurable platforms, with internal or external sales engineers, consultants, and deployment teams responsible for configuring the apps for each company. Which then begs the question, why do you want a shittier platform than a generalized one where the agent can fully operate on?

My view is that this entire process will be done by agents on command, down to the most granular interface (whether you call it a single Claude Artifact or something else). Instead of relying on a vendor's implementation team to customize the software, users will be able to ask their agents to modify the applications directly. Creating these types of reverse-engineering artifacts is simply not friendly within the bounds of existing OS, but a separate application, whether on the browser or as a desktop application, has the full opportunity to support this.

T3, Claude Cowork with customization, and installable Claude Artifacts all point in this direction.

We will have different containers for apps (which themselves can also be apps):

- **Tmux-like interfaces.** Already heavily explored by the better coding agent applications.
- **Apps embedded within messages.** Already a default pattern, with apps in the sidebar also becoming standard in 2025–2026.
- **App-centered interfaces with chat in the sidebar.** This inverts the relationship, making the app primary and chat secondary (e.g., Claude in Excel), similar to how agents are embedded in existing applications.

These types of views will all continue to exist. But we'll also have:

- **Infinite canvases (tldraw).** Useful for expressing things like ComfyUI workflows or designs in tools like Paper or Figma, where an interface could be represented as an infinite canvas with each component being an image-display App.
- **Fixed canvases with layered windows.** The default OS experience.
- **Resizable panels with fixed widths and heights.** Similar to Metabase.

*Relevant: Rex, OpenCode Rune*
