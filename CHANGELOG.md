# Changelog

All notable changes to gdrv. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## 0.2.0

- `permissions list <address>` (`drive-permission-list/v1`): who may open an
  item. Each grant has its type, role, email or domain, and its source
  (`direct`, `inherited` from a named folder above, or `unknown`);
  `link_sharing` lists the grants to anyone or to a domain; `can_share` says
  whether this account may change them. Read only. An item Drive will not list
  grants for is exit 3, reason `permissions_hidden`.
- `stat <address>` (`drive-item/v1`): one item's id, parent, size, modified
  time, md5, and whether this account owns it, has shared it and may share it.
  An address may name any file or folder by its id alone.
- `ls` rows have `parent`, the id of the folder each item is in.
- `client import --from-profile <NAME>` copies another profile's OAuth client
  (from cli-common v0.37.6), so a second sign-in of a Cloud project needs no
  client JSON.
- Built on cli-common v0.38.1. Licensed under AGPL-3.0-only.

## 0.1.1

- `cp` copies a tree several files at a time: uploads 4 at a time, after
  making every missing folder once, and downloads 8. Each file is still
  checked by md5, and the counts are unchanged. The first failure stops the
  copy, and what was copied before it stays.
- `cp --transfers N` (1 to 8) lowers how many files are in flight at once,
  as rclone's flag does. Uploads never go past 4.
- Built on cli-common v0.37.4.

## 0.1.0

The first release.

- Sign-in through Drive's own API with the owner's Desktop OAuth client:
  `login`, `status`, `logout`, `client import`, `profiles`, `about`, `folder`,
  and the family's `auth` spelling. Full `drive` scope by default, one
  profile per Google account, no default profile.
- Addresses `gdrive:<profile>/<folder id>[/<path>]`, where `root` is My Drive.
- File commands: `ls [-R]`, `cat`, `cp` (files and trees, both ways, every
  md5 compared, unchanged files skipped), `mv`, `rm` (to the trash), `mkdir`,
  `md5sum`, `duplicates`, and `rmdir` (an empty folder to the trash).
  `--json` on every command.
- `ls` and `md5sum` list every item with its Drive id, including items that
  share a name, as rclone does. A path through a shared name is refused
  (reason `duplicate_name`), and a folder's id addresses it directly.
- With no terminal on stdin, a keychain read waits at most 60 seconds
  (`GDRV_KEYCHAIN_WAIT`) and then exits 3.
- Built on cli-common v0.37.3.
