# AGENTS.md: drive-cli

## What this is

`gdrv` is Google Drive from the command line for scripts and agents. It goes
through Drive's own API with the owner's OAuth client, and replaces rclone for
that use on this machine. It is a thin layer over cli-common's `pk-cli-drive`
(feature `google`). That crate owns sign-in, the vault, profiles, the store
(`Drive`) and the fake Drive server. This repo owns the file commands, the
addresses and the CLI surface. It follows the family spec `piekstra-cli/1`
([DESIGN.md](https://github.com/piekstra/cli-common/blob/main/DESIGN.md)).

## Build / test / run

```sh
make verify     # fmt-check, clippy -D warnings, tests, smoke: what CI runs
make install    # ~/.cargo/bin/gdrv, built --locked and signed
cargo run -- ls --help
```

## Layout

| File | What it holds |
|---|---|
| `src/main.rs` | Parsing, SIGPIPE, exit codes |
| `src/lib.rs` | `run`: one command to its module |
| `src/cli.rs` | The clap tree, and the help that names each command's listing |
| `src/address.rs` | `gdrive:<profile>/<folder id>[/<path>]` and local paths |
| `src/context.rs` | The `DriveCli` (vault, bounded keychain wait), opening a store, and the debug-only test key file |
| `src/files.rs` | ls, cat, cp, mv, rm, rmdir, mkdir, md5sum, duplicates |
| `src/standard.rs` | config, self-update, completions, info |
| `tests/cli_surface.rs` | Offline: every help, usage errors, exit codes, the JSON error document |
| `tests/flow.rs` | Every command through the binary against the fake Drive server |
| `tests/no_personal_data.rs` | Tracked files carry no personal data |
| `docs/drive.md` | How gdrv maps onto Drive, and the traps |

## Rules

- **Nothing `make verify` runs reads the real keychain.** Every test and
  `smoke` run gdrv with its own home, `GDRV_KEYCHAIN_SERVICE` set to an empty
  service, and `GDRV_TEST_KEY_FILE` holding the vault's key. That last hook
  exists only in debug builds; a release build has no such code.
- **Test against the fake.** `pk_cli_drive::google::fake::FakeDrive` serves
  Drive's API and Google's sign-in on 127.0.0.1. The binary reaches it
  through `GDRV_DRIVE_TEST_ENDPOINT`. No test reaches Google.
- **No personal data in the repo:** code, fixtures, docs, commits, PRs and
  issues. Use synthetic values (`person@example.test`, `1Synthetic_folder`).
  `tests/no_personal_data.rs` scans tracked files for the shapes.
- **Validate before reading.** A bad address, a `--profile` that is not the
  address's, and `rm` without `--force` off a terminal all fail before any
  keychain read or request.
- **Every write reads back.** Report success only after Drive or the disk
  shows the result.
- **One JSON document per run**, tagged `<name>/v1`. A schema change is a new
  version, not an edit.
- **A Drive mechanism goes into `pk-cli-drive`**, through a cli-common PR and
  tag, not into this repo.
- **Dogfood on a real account in a scratch folder gdrv makes**, and trash it
  afterwards. Never write to a folder gdrv did not make.

## The CLI family & cli-common

cli-common is pinned to a tag in `Cargo.toml`; every `pk-cli-*` is on the same
tag. Read `DESIGN.md` and `conformance.md` there before changing the surface.
PRs are reviewed by `cr` as `piekstra-dev`, and `ppr check` runs before every
`gh pr create`.
