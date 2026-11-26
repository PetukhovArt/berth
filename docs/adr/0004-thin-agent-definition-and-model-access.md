# 0004. Thin Agent Definitions and configurable Model Access

- Status: Accepted

## Context

A Reaction step that runs an agent, and a hub that places agents on Machines, need to know which agent to run. Today one Agent record is both the running agent and the row people see, and the nearest thing to a definition is a set of presets kept in one client's local storage.

Harness configuration already lives in layers: organization policy and plugins, the user's global configuration, files in the project, and local overrides. Berth's shared setup already copies skills, MCP servers, hooks and instructions from a directory or repo into every checkout, in each agent's format, but it is configured per daemon.

When a rule starts an agent at night on a shared Machine, it must be clear whose model account pays and whose forge token is used. Anthropic's consumer terms say an account's login and credentials may not be shared and the account may not be made available to anyone else, and they cover the Free, Pro and Max plans including Claude Code. OpenAI's terms carry the same rule for credentials in the version checked; whether its current consumer wording for ChatGPT plans says the same was not confirmed. Companies also route model traffic through their own gateways, and Berth already has a gateway app that speaks the Anthropic API with fallback chains.

## Decision

An Agent Definition is thin. It holds what no harness layer has: the role and its instructions, the harness, model and effort, a reference to a shared-setup bundle, the Machine requirements, the permission mode for unattended work, limits on time, turns and spend, and the Model Access to run with. Harness configuration stays in its own layers, and shared setup moves from a daemon setting to bundles at Organization and Workspace level on the hub. An Agent runs from a Definition on a Machine; its row is a Session and its history is its transcript. Presets become personal Definitions. Agents may write Definitions, published through hub roles like rules.

Model Access comes in four kinds, and an admin decides through roles which kinds each Workspace and each kind of rule may use: a personal subscription, which never leaves its owner's machine; a personal API key; an organization gateway; a service account, kept encrypted on the hub and handed to a Machine for one run. Forge tokens follow the same split between personal and bot tokens. Spending is recorded per rule and per account.

## Alternatives

- Keep presets and describe the agent inline in each Reaction. Simpler, but settings are copied from rule to rule and a team cannot reuse an agent.
- Use each harness's own agent files as the definition. Native to each harness, but there is no single format across harnesses.
- Always run as the rule's owner, on the owner's machines. Safe with vendor terms, but team rules stop when the owner's laptop closes.
- The hub keeps everyone's personal accounts and uses whichever is free. Highest throughput, against vendor terms and a large leak risk.

## Consequences

- Berth adds no fifth copy of harness configuration; Definitions point at what teams already maintain.
- Teams that want unattended work need an API key, a gateway or a vendor team seat; a personal subscription only powers its owner's own agents.
- The hub stores secrets and must encrypt them and limit their delivery to one run.
