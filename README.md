# gdrv

Google Drive from the command line, for scripts and agents. It goes through
Drive's own API with an OAuth client of your own, and every file it copies is
checked by md5.

**Unofficial; not affiliated with or endorsed by Google.**

```sh
gdrv login --profile home                      # once per Google account, in the browser
gdrv ls gdrive:home/root                       # My Drive's top level
gdrv cp ./reports gdrive:home/root/Reports     # a tree up; unchanged files are skipped
gdrv cp gdrive:home/1Synthetic_folder ./copy   # a folder's tree down
gdrv md5sum gdrive:home/1Synthetic_folder      # as md5sum prints
gdrv rm --force gdrive:home/root/Reports/old.txt   # to the trash
```

Every command takes `--json` and prints one document tagged
`"schema": "<name>/v1"`. It follows the piekstra CLI family's spec,
`piekstra-cli/1` ([DESIGN.md](https://github.com/piekstra/cli-common/blob/main/DESIGN.md)),
and is built on cli-common's `pk-cli-drive`.

## Install

```sh
git clone https://github.com/piekstra/drive-cli && cd drive-cli
make install           # cargo install --locked, then signs the binary on macOS
gdrv self-update       # later: the latest release from GitHub
```

Release assets for `aarch64-apple-darwin`, `x86_64-apple-darwin` and
`x86_64-unknown-linux-gnu` are on the
[releases page](https://github.com/piekstra/drive-cli/releases).

## Signing in

gdrv signs in with a Desktop app OAuth client from a Google Cloud project you
own. [pk-cli-drive's guide](https://github.com/piekstra/cli-common/blob/main/crates/pk-cli-drive/docs/google-drive.md#registering-the-oauth-client)
has the steps: enable the Drive API, set up the consent screen, make a Desktop
app client, and download its JSON.

```sh
gdrv login --profile home --client-file ~/Downloads/client_secret_….json
```

`login` stores the client, opens Google's consent page in the browser, and
waits for you to allow it. The client and the tokens are stored in gdrv's vault,
not in the file you downloaded, so delete that file afterwards. Pipe the JSON
in with `--client-stdin` to keep it off disk entirely.

| Command | What it does |
|---|---|
| `login --profile <NAME>` | Sign a profile in to one Google account. `--scope file` asks for `drive.file` instead of full Drive; `--account <EMAIL>` refuses a sign-in to any other account |
| `status --profile <NAME>` | Whether the profile is signed in, and as whom. `--local` reads no keychain |
| `logout --profile <NAME>` | Remove the profile's sign-in from this machine. `--revoke` also revokes the grant at Google, `--forget` removes the profile too |
| `client import --profile <NAME>` | Store a client for a profile without signing in (`--file` or `--stdin`) |
| `profiles list`, `profiles remove <NAME>` | The profiles on this machine |
| `about --profile <NAME>` | The account and its storage |
| `folder create <NAME>`, `folder list` | Folders at the top of My Drive |
| `auth login`, `auth status`, `auth logout` | The family's spelling of the same three |

A profile is one Google account, and there is no default profile. The full
`drive` scope is the default; Google lists it as restricted, so an
unverified project shows an "unverified app" screen at each consent.

## Addresses

```text
gdrive:<profile>/<folder id>[/<path>]
```

- `<profile>` is a profile `login` made.
- `<folder id>` is the part of a folder's URL after `/folders/`, or the `id`
  that `ls` prints for it. `root` is My Drive.
- `<path>` is a path of names below that folder.

Anything that does not start with `gdrive:` is a local path; write
`./gdrive:x` for a local file of that name. A trailing `/` on a destination
means "into this folder": `gdrv cp a.txt gdrive:home/root/inbox/` writes
`inbox/a.txt` and makes `inbox` if it is missing.

File commands take the profile from the address. A `--profile` given with one
must name the same profile, or the command is exit 2.

## File commands

| Command | What it does | Listing | Schema |
|---|---|---|---|
| `ls <address> [-R]` | One level: folders and files. `-R`: every file below, by path. Each item has its Drive id, the id of the folder it is in (`parent`), and each file its size, modified time and md5. Items that share a name are each listed | `--listing`, default `whole` | `drive-entry-list/v1` |
| `stat <address>` | One item: its id, the id of the folder that holds it (`parent`), size, modified time, md5, and whether this account owns it, has shared it and may share it. A file or folder is also addressed by its id alone (`gdrive:<profile>/<id>`). Read only | `whole` | `drive-item/v1` |
| `permissions list <address>` | Who may open an item: each grant's type, role, email or domain and source (`direct`, `inherited` from a folder above, or `unknown`), `link_sharing`, and `can_share`. Read only: nothing is shared or unshared | `whole` | `drive-permission-list/v1` |
| `cat <address>` | One file's bytes on stdout. With `--json`, the content as UTF-8 text or base64 | `whole` | `drive-file/v1` |
| `cp <source> <destination>` | A file or a tree, local to Drive or Drive to local | `whole` | `drive-copy/v1` |
| `mv <source> <destination>` | Move or rename a file within one folder id's tree | `whole` | `drive-move/v1` |
| `rm <address>...` | Move files to the Drive trash, recoverable there for 30 days | `whole` | `drive-trash/v1` |
| `rmdir <address>...` | Move empty folders to the Drive trash; one holding anything is refused (reason `not_empty`) | `whole` | `drive-rmdir/v1` |
| `mkdir <address>` | Make a folder and any missing parents | `whole` | `drive-mkdir/v1` |
| `md5sum <address>` (alias `hashes`) | The md5 Drive holds for a file, or for every file below a folder | `--listing`, default `whole` | `drive-hash-list/v1` |
| `duplicates <address>` | Every name that two or more items share in one folder | `--listing`, default `whole` | `drive-duplicate-list/v1` |

### Listings

Drive holds two kinds of item that have no bytes of their own: Google's own
documents (Docs, Sheets, Slides) and shortcuts. A listing decides what
happens to them.

| Listing | Google documents | Shortcuts |
|---|---|---|
| `whole` (default) | Listed as type `document`, with no md5 | Followed to their targets; a shortcut to a folder is walked. A write through one is refused (exit 2, reason `shortcut`) |
| `bytes` | Left out | Left out |

Every command that writes uses `whole`, so nothing is written onto, through or
beside a shortcut without notice. The read-only commands `ls`, `md5sum` and
`duplicates` take `--listing bytes` to see only files with bytes.

### Who may open an item

`permissions list` reads grants; it never changes one. Drive's own answer says
where a grant comes from when it gives one. Otherwise the folder above's
grants decide, one more request: a grant the folder above also holds, at the
same role or higher, reaches the item from it. When the folder above is out of
this sign-in's sight, the source is `unknown`. `link_sharing` lists the grants
of type `anyone` and `domain`, with whether each shows in search.

Drive lists grants to an account that may change them. Another account gets
exit 3, reason `permissions_hidden`. `can_share` says which one this is. Under
the `drive.file` scope an item this Cloud project's clients did not make or
open is exit 4, as it is for every command.

### Copying

- **Unchanged files are skipped.** Every file's md5 is compared with the
  other side's before anything is written.
- **Changed files need `--force`.** A file already there with other bytes
  refuses the whole copy before anything is written (exit 2, reason
  `existing_differs`). With `--force`, Drive replaces the file in place, so
  it keeps its id and revisions.
- **Every write is read back.** Uploads are checked against the md5 Drive
  reports afterwards. Downloads are checked against Drive's md5 and written
  owner-only (`0600`, directories `0700`).
- **Google documents are skipped on download.** They have no bytes to copy,
  and `skipped_documents` counts them.
- **A download refuses unsafe names.** A Drive item named `..`, or with a
  `/` in its name, could otherwise leave the local directory. So could two
  items that land on one local path (`A.txt` and `a.txt` on macOS). gdrv
  refuses them before anything is written (reason `unsafe_name`).
- **Trees go several files at a time.** Uploads run 4 at a time and
  downloads 8, after every missing folder is made once. `--transfers N`
  (1 to 8) lowers that, and uploads never go past 4: Drive takes about three
  writes a second per user. The first failure stops the copy, and what was
  copied before it stays.
- **Copies within Drive are not supported.** `mv` moves a file within one tree.

### Confirmation

Only `rm` and `rmdir` ask before they act. Without a terminal, or with
`--json`, they need `--force` and are otherwise exit 6, before any keychain
read or request. `cp`
and `mv` never overwrite without `--force`. `mkdir` keeps a folder that is
already there.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Done |
| 1 | Something else failed, such as an unsafe name or two items sharing a name |
| 2 | Usage: a bad address, a refused overwrite, a write through a shortcut, a folder that is not empty, a profile not signed in |
| 3 | Auth: the sign-in was refused or revoked, the keychain did not answer, or Drive will not list who may open an item (reason `permissions_hidden`) |
| 4 | Not found |
| 5 | Google's API failed, or a write did not read back |
| 6 | Confirmation required (`rm` or `rmdir` without `--force`) |

With `--json`, an error is one document on stdout:
`{"error": {"code", "message", "reason", "remedy"}}`. `reason` is set when a
script can act on it: `existing_differs`, `occupied`, `google_document`,
`shortcut`, `not_empty`, `unsafe_name`, `duplicate_name`,
`permissions_hidden`.

## Other commands

`config path` and `config show` print where gdrv keeps its files; gdrv has no
settings of its own. `info` describes the CLI (`cli-info/v1`), `completions
<shell>` prints a completion script, and `self-update` installs the latest
release.

## Environment

| Variable | Effect |
|---|---|
| `GDRV_DRIVE_PROFILE` | The profile for `login`, `status`, `logout`, `about`, `client`, `folder` and `profiles` when `--profile` is not given |
| `GDRV_KEYCHAIN_WAIT` | Seconds a keychain read waits for its dialog to be answered. The default is no limit at a terminal and 60 seconds without one; after that, exit 3 |
| `NO_COLOR` | No color |
| `GDRV_KEYCHAIN_SERVICE` | Test only: the keychain service that holds the vault's key |

## Security

The OAuth client and the tokens are stored in gdrv's vault, encrypted under a
key held in one keychain item. They never appear in argv, logs or output.
[SECURITY.md](SECURITY.md) covers this, the refresh lock, and what `--revoke`
ends.

## Docs

- [docs/drive.md](docs/drive.md): how gdrv maps onto Drive, and the traps.
- [AGENTS.md](AGENTS.md): working on this repo.
- [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md), [CHANGELOG.md](CHANGELOG.md).

## License

Copyright (C) 2026 Caleb Piekstra. Licensed under the GNU Affero General Public
License, version 3 only (AGPL-3.0-only); see [LICENSE](LICENSE). Releases up to
and including v0.1.1 were published under MIT OR Apache-2.0.
