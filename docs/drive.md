# How gdrv maps onto Google Drive

gdrv is a thin layer over cli-common's `pk-cli-drive`.
[Its guide](https://github.com/piekstra/cli-common/blob/main/crates/pk-cli-drive/docs/google-drive.md)
covers the OAuth client, the vault, profiles, scopes and the store. This page
covers what gdrv adds: addresses, the file commands, and the traps they
handle.

## Addresses

`gdrive:<profile>/<folder id>[/<path>]` names a folder by its id, then a
path of names below it. An id, unlike a name, is unique and does not change
when the folder is renamed or moved. The address's folder is the store:
`mv` works within one, and every path is resolved from it.

`root` is accepted wherever a folder id is, and means the account's My
Drive. Drive's API takes `root` as an alias for that folder's real id. gdrv
resolves it once, through a metadata read, to the real id that listings
report as a parent. Without that step, every path under `root` would resolve
to nothing. pk-cli-drive v0.37.2 fixed this. Its tests and gdrv's cover
every operation under `root` against the fake server, and listing My Drive
by `root` and trashing a folder under it were checked against Drive's own
API.

## Traps

| Trap | What gdrv does |
|---|---|
| **Names are not unique.** One folder can hold two items with the same name, so a path can name two things | `ls` and `md5sum` list every item with its Drive id, both of a pair included, as rclone does. A path through a shared name stops (exit 1, reason `duplicate_name`), and so does a copy or download of a folder holding a pair, since it could not tell which is meant. `gdrv duplicates <address>` lists every shared name with each item's id. Address a folder by its own id (`gdrive:<profile>/<id>`), or keep one item in the Drive web page |
| **Google's own documents have no bytes.** Docs, Sheets and Slides have no md5 and no content to download | Listed as type `document` with no md5. `cat` and `md5sum` refuse one (reason `google_document`); `cp` skips them on download and counts them; an upload refuses to replace one |
| **Shortcuts point elsewhere.** A shortcut's target can be in another folder, or in someone else's Drive | The default `whole` listing follows them, as rclone does. Every write refuses to go onto or through one (exit 2, reason `shortcut`), so a copy never lands in a folder outside the store |
| **A name can hold `/` or be `..`.** Drive allows both, and joined onto a local path they leave the destination | A download refuses such names before writing anything (exit 1, reason `unsafe_name`) |
| **Drive is case-sensitive; macOS is not.** `A.txt` and `a.txt` can sit side by side in Drive, but on a Mac they land on one file | Refused the same way, as is a file named like a folder another file lands in |
| **Replacing a file can lose its history.** Deleting and re-uploading gives a new id and drops the revisions | `cp --force` updates the file in place: same id, a new revision |
| **An item's folder is not its path.** A path can be under a shared folder, or hold two items of one name, and a shared-with-me item has no visible folder | `stat` and `ls` print the Drive id of the folder that holds each item (`parent`), so "is this folder inside that one" compares ids. Where Drive names no parent this sign-in can see, the field is left out, never guessed |
| **A grant can come from a folder above.** Drive does not say where a My Drive grant comes from | `permissions list` says `direct`, `inherited` (with the folder) or `unknown`. Drive's own details settle it where it gives them, and otherwise the folder above's grants; `unknown` means that folder is out of sight |
| **A 200 is not proof.** | Every write is read back: the md5 after an upload or a download, the new path and the empty old one after a move, the path gone after `rm` |
| **Drive takes about three writes a second per user.** More at once only draws 429s | A tree copy uploads 4 files at a time and downloads 8, after making each missing folder once. The retries absorb a 429. `cp --transfers N` lowers both, `1` being one at a time. The first failure stops the copy, and what was already copied stays |
| **The trash is not deletion.** | `rm` trashes files and `rmdir` empty folders; nothing is deleted for good. A trashed item is gone from every listing, and an item in the trash does not count against `rmdir`'s emptiness |

## Sharing and parents

What Drive documents, what gdrv does, and what has not been tried against a
real account:

- **Scopes.** Drive's discovery document lists `drive.file` among the scopes
  `permissions.list` and file metadata accept. It does not say which grants an
  app sees on a file it did not make. An item it may not see is exit 4.
- **Parents.** Drive documents one parent per file. It does not say when a
  `drive.file` sign-in, or an item shared with the account, gets none. gdrv
  prints the one Drive names and leaves the field out otherwise.
- **Grants.** A caller Drive will not list grants for gets exit 3, reason
  `permissions_hidden`. `permissions list` never turns a refusal into an empty
  list.
- **Inherited or direct.** A folder's grants reach everything in it, and
  nothing below can lower them. `permissionDetails` names the folder a grant
  comes from only in shared drives, so in My Drive the folder above's grants
  are compared: the same grant, at that role or higher, is inherited.

How a real account answers `permissions.list` under `drive.file`, and whether
`parents` is empty there, is as documented above and has not been checked
against a live one.

## Coming from rclone

| rclone | gdrv |
|---|---|
| `rclone lsjson -R --files-only remote:dir` | `gdrv --json ls -R gdrive:<profile>/<folder id>` |
| `rclone cat` | `gdrv cat` |
| `rclone copy src remote:dir` | `gdrv cp src gdrive:…/dir` |
| `rclone copy remote:dir dst` | `gdrv cp gdrive:…/dir dst` |
| `rclone copy --transfers 4` | `gdrv cp --transfers 4` (1 to 8; uploads at most 4) |
| `rclone moveto` | `gdrv mv` |
| `rclone delete` (files) | `gdrv rm` (to the trash) |
| `rclone rmdir` | `gdrv rmdir` (to the trash) |
| `rclone mkdir` | `gdrv mkdir` |
| `rclone md5sum` | `gdrv md5sum` |
| `rclone dedupe --dry-run` | `gdrv duplicates` |

Where they differ:

- **A changed file:** `rclone copy` replaces a file whose contents differ.
  `gdrv cp` refuses the whole copy, before writing anything, unless `--force`.
- **Google documents:** rclone exports them on download (as `.docx`,
  `.xlsx` and so on) and lists them with size -1. gdrv lists them with no
  size or md5, and skips them on download.
- **Addresses:** an rclone remote is rooted at a folder in the config
  file. A gdrv address carries the profile and the folder id itself, so
  there is no config to keep.
- **The OAuth client and tokens:** rclone keeps them in `rclone.conf`.
  gdrv keeps them in its vault, sealed under a keychain-held key.
