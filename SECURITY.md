# Security policy

## Reporting a vulnerability

Report it privately through GitHub's
[private vulnerability reporting](https://github.com/piekstra/drive-cli/security/advisories/new),
not in a public issue. Include the version (`gdrv --version`), the command
that shows the problem, and what you expected. Please do not include tokens,
client secrets or the contents of your files.

## What gdrv stores, and where

| What | Where |
|---|---|
| The OAuth client (id and secret) | Vault entry `drive/<profile>/client`: a sealed, owner-only file under `~/.config/gdrv/vault/` |
| The tokens (access and refresh) | Vault entry `drive/<profile>/token`, sealed the same way |
| The vault's key | One keychain item: service `piekstra.gdrv`, account `vault-key` |
| Profiles: each one's name, the OAuth client's id (not its secret) and, once signed in, its account | `~/.config/gdrv/drive.json`, owner-only, nothing secret |

The client and the tokens never appear in argv, in logs, in output or in an
unsealed file. `client import` and `login` read the client JSON from a file
you name (`--file`, `--client-file`) or from stdin (`--stdin`,
`--client-stdin`). Once imported, the downloaded file is no longer needed.

On macOS, each newly built or upgraded binary asks once, in a dialog, before
it may read the vault's key. Choose "Always Allow" for that binary. With no
terminal on stdin (a script, an agent), a read waits at most 60 seconds
(`GDRV_KEYCHAIN_WAIT` changes this) and then fails with exit 3 naming the
dialog. So a run nobody is watching ends; it does not hang.

## Tokens and revoking

- **The refresh lock.** Tokens are renewed under gdrv's refresh lock, so two
  processes never spend one refresh token twice. A 401 renews once and
  retries.
- **Where tokens go.** Tokens are sent only to Google's endpoints. The
  test-only `GDRV_DRIVE_TEST_ENDPOINT` accepts nothing but a loopback
  `http://127.0.0.1:<port>` or `http://localhost:<port>`, so it cannot send a
  token off the machine.
- **`logout`** removes the tokens from this machine. Google still holds the
  grant.
- **`logout --revoke`** also revokes the grant at Google. That ends every
  sign-in of the account through any OAuth client of the same Cloud project:
  another CLI's, a web app's, on every machine.
- **A refused refresh** is exit 3, with `gdrv login --profile <NAME>` as the
  remedy. Google refuses one that was revoked, or unused for six months, or,
  for a project left "In testing", older than seven days.

## Scopes

The default is the full `drive` scope, which reads and writes every file the
account can. `login --scope file` asks for `drive.file` instead. That covers
only files a client of the same Cloud project created, or that the person
opened with one, so a folder made in the Drive web page is not visible to it.
[pk-cli-drive's guide](https://github.com/piekstra/cli-common/blob/main/crates/pk-cli-drive/docs/google-drive.md#scope-drivefile-or-full-drive)
records what Google says about both and the test that settled it.

## What gdrv does to your files

- `rm` moves files, and `rmdir` empty folders, to the Drive trash, where
  they are recoverable for 30 days. Neither deletes for good; `rm` never
  touches a folder, and `rmdir` refuses one that holds anything. Both ask
  first, or need `--force`.
- `cp` and `mv` never replace a file without `--force`. A replaced file keeps
  its Drive revisions (`cp`) or goes to the trash (`mv`).
- Downloads are written owner-only and checked against Drive's md5.
- A download refuses item names that would leave the destination directory
  (`..`, a `/` in a name), or that would land two items on one local path.

## Network

gdrv talks to Google's OAuth and Drive endpoints, and to GitHub's releases
API for `self-update`. It sends no telemetry.

## Supported versions

Only the latest release gets fixes. `gdrv self-update` installs it.
