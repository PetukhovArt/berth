# Command reference

The `berth` command runs the daemon and the commands agents and scripts run against it. The interface
you work in is the desktop app; see [getting started](getting-started.md). A bare `berth` prints help,
and `berth <command> --help` describes one command.

## Projects

```sh
berth add <dir>          # add a repo as a project, named after its root directory
berth add .              # same, for the repo you're in (bare `berth <dir>` / `berth .` also work)
```

A directory whose name matches a command needs the long form or a `./` prefix.

## Daemon

```sh
berth daemon             # run the daemon (normally started for you)
berth daemon --foreground  # stay attached to the terminal instead of logging to file, for debugging
berth kill               # stop the daemon and all sessions cleanly
```

A command that needs the daemon starts one from its own binary when none is running. The desktop app
starts `berth daemon` from your `PATH` on Windows and macOS.

## Run by agents

These exist so a session can act on its own tree; you rarely type them yourself.

```sh
berth rename <title>     # title the current session (--force to retitle a named one)
berth worktree [name] [--base <ref>]
                         # move the current session into a worktree of its project, creating the
                         # branch if it's new. No name invents one; --base picks the start point
berth enter <dir>        # move the current session into the repo at <dir>, adding it as a project
                         # when berth does not have it; the conversation resumes there
berth spawn <task> [--kind <claude|codex|cursor>]
                         # start a new agent session beside the current one, in the same worktree,
                         # opening on <task>; --kind defaults to this session's agent kind
```

## Workspaces

```sh
berth workspace add <name>     # create a workspace (a named project group)
berth workspace open <name>    # make it the default: `berth add` adds projects there
berth workspace list           # list workspaces; * marks the default
berth workspace rename <a> <b> # rename a workspace
berth workspace delete <name>  # delete an empty workspace
```

## Upgrade

```sh
berth upgrade            # install the latest release over this one (--force on a local cargo build)
```

It runs `install.sh` again, so it works on macOS and Linux only. On Windows it refuses and names the
way to update from a checkout. Upgrading while a daemon is running is safe: sessions keep running on
the old binary until you `berth kill`, and an idle old daemon is shut down for you.
