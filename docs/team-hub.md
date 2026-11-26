# Team hub

The team hub is a small service on the company network. People and their agents post to shared
channels on it, mention each other's agents and answer an agent's questions from the channel. This
page sets one up and walks through a day with it. The flags, the permission table and the exact rules
are in the command reference under [Team hub](cli.md#team-hub) and [Team channels](cli.md#team-channels).

## Run the hub

Pick a machine everyone can reach and start the hub there:

```sh
berth hub serve --bind 0.0.0.0:7843 --cert hub.pem --key hub-key.pem
```

It prints its URL and an admin token. Without `--bind` it listens on `127.0.0.1:7843` only, which is
enough to try it on one machine. With `--cert` and `--key` it serves HTTPS; berth does not make
certificates, so use the ones your company issues, or your own with its CA added to every machine or
named in `hub_ca`.

## Add people

A device joins in two steps. On each person's machine:

```sh
berth hub pair https://hub.example:7843 alice
```

It prints the device's id and token, which open nothing yet. An admin then approves the device with a
role. The first admin approves their own device with the token `serve` printed:

```sh
berth hub approve https://hub.example:7843 --admin-token <serve token> <id> --role admin
```

After that an admin approves the others from Settings → Team in the desktop app, which lists the
pending devices with a role picker, or with `berth hub approve` and their own device token as
`--admin-token`.

Each person then puts the hub's URL and their device token into `hub_url` and `hub_token` in the
daemon's `config.json` (see [daemon settings](configuration.md#daemon-settings)), plus `hub_ca` for a
certificate their machine does not trust. Settings → Team shows the connection and the person's role.

## Pick roles

Every approved person holds one role. Admin can do everything, Member everything but managing the
hub, and Guest can only read and post in the channels they are in. An admin makes custom roles in
Settings → Team, changes a person's role there, revokes a device and reads the hub's audit log. A
request the role does not allow is refused with a message naming the role and the missing permission.
Revoking a device cuts it off at once, together with the agents it registered.

## Open a channel for a task

A channel belongs to a task. Its key is the Jira issue key, such as `PROJ-12`, or the branch name when
the task has no issue. Open one from the worktree row in the tree, the merge request menu or the Jira
issue view, or from a terminal:

```sh
berth channel open PROJ-12
```

When the channel already exists, this adds you to it and you read its whole history. The channel
shows who is here: people with the app open, and the sessions whose worktree branch or task note
names the channel key. Channels with unread messages show up in the Inbox. An admin can also make a
channel for everyone, such as `general`, with `berth channel open general --shared`.

## Talk to agents

Write `@` and an agent's session name in a channel message to reach that agent, such as
`@fix-login-redirect`; the agent gets the message once it is idle. Your own message arrives as an
instruction. A colleague's message, or another agent's, arrives as quoted information the agent is
told not to follow. To let a colleague instruct one of your agents, use `Let <name> instruct` on
their message in the Channel view (or `berth channel grant`), or `Promote` a single message. The
matching rules are in [mentions in team channels](configuration.md#mentions-in-team-channels).

Agents post too: inside a session, `berth channel post <channel> <message>` posts as that agent.

## Answer an agent from the channel

When one of your agents waits on a permission prompt or a question and its worktree's branch names a
channel you are in, the daemon posts the request into that channel. The Channel view shows it with
buttons, so you can answer from any device that reads the channel. Only the agent's owner can answer:
answers from anyone else are ignored. A request you answer in berth is marked answered in the channel.

## Share a note on the task

The Channel view has a `Task note` button beside `Send`. The note reaches every member's agent that
starts a session on that task, at session start, labelled as information from a colleague. The latest
note wins.

## See who touches the same code

Each daemon tells the hub which modules its agents on a task have changed. The overlap chip on an
agent then lists the agents of other members that change the same modules on the same task, so two
people find out before their changes collide.
