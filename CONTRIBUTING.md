# Contributing to drive-cli

gdrv is a small CLI over Google Drive's API. It follows the family spec in
[cli-common](https://github.com/piekstra/cli-common)'s `DESIGN.md`, and
[`AGENTS.md`](AGENTS.md) has this repo's rules.

## Before you start

- Open or comment on an issue describing the change.
- Branch from `main`; `main` takes changes only through pull requests.
- A change to how gdrv talks to Drive (listing, copying, sign-in) usually
  belongs in cli-common's `pk-cli-drive`, with a test against its fake server.

## Local checks (must pass)

```sh
make verify
```

Nothing it runs reaches Google or reads your keychain. A new command or flag
gets a test in `tests/cli_surface.rs` (help, usage errors) and one in
`tests/flow.rs` (against the fake server).

## Ground rules

- No secrets, tokens, client JSON or real account data in code, tests,
  fixtures, docs or commits. Use synthetic values such as
  `person@example.test` and `1Synthetic_folder`. CI runs `gitleaks` and
  `tests/no_personal_data.rs`.
- Stay within Google's API terms.

## Pull requests

Describe what changed and how it was tested. A maintainer reviews every PR.

## License of contributions

By contributing, you agree your work is licensed under **AGPL-3.0-only**.
