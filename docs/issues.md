# Issues

An issue moves through four states: filed, triaged, fixed on `main`, released. Labels show the state; `.github/labels.yml` defines them and the `Labels` workflow syncs them to GitHub.

## Filing

- One defect or one request per issue. Search open and closed issues first.
- The web form asks for what happened, what was expected, how to reproduce, the berth version, where it happened (desktop app, phone web client, a `berth` command, the daemon), the OS and the Agent. Blank issues are off.
- From the command line or an Agent, the body uses the form's headings, so every issue reads the same:

  ```sh
  gh issue create -R PetukhovArt/berth --label bug,needs-triage --title "<area>: <what berth did>" --body-file body.md
  ```

  `-R` names the repository: an Agent often files from the project it runs in, and without it `gh` files into that project.

  `body.md` has `### What happened`, `### Expected`, `### How to reproduce`, `### berth version`, `### Where`, `### OS`, `### Agent`.
- The title starts with the area, then the defect as observed: `desktop: closing a tab leaves the Agent running`.
- Output is pasted as is, with private names, paths, merge request numbers and task keys replaced by placeholders. The repository is public.

## Triage

`needs-triage` comes off, and the issue gets one of:

| outcome | labels | action |
| --- | --- | --- |
| reproduced or accepted | `confirmed`, one `area: *`, `os: *` when it shows only there | stays open |
| details missing | `needs-info` | a comment says what is missing |
| already reported | `duplicate` | `gh issue close <n> --duplicate-of <m>` |
| out of scope or not berth | `wontfix` or `invalid` | `gh issue comment <n>` with a one-line reason, then `gh issue close <n> --reason "not planned"`: on a closed issue, `close --comment` drops the comment and still exits 0 |

## Fixing

- A bug fix starts with a test that fails on the reported case.
- The commit subject stays conventional (`fix(desktop): ...`); the body ends with `Fixes #<n>`, or `Refs #<n>` when the commit fixes only part of it. One issue may take several commits: only the last one says `Fixes`.
- A change a user would notice carries its `CHANGELOG.md` line under `## Unreleased`, ending with `(#<n>)`, and its `docs/` update in the same commit.
- After the commit, the issue gets `fixed-on-main` and a comment naming the commits. Pushing `main` closes the issue.

## Release

- The push to `main` closes every issue named by `Fixes #<n>`. Nothing is released until a `version` bump lands, as `docs/releasing.md` describes.
- When the `Release` workflow publishes a version, it comments `Released in berth <version>` on every closed `fixed-on-main` issue and removes the label.
