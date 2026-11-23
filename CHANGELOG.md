# Changelog

All notable changes to gdrv. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## 0.1.0

The first release.

- Sign-in through Drive's own API with the owner's Desktop OAuth client:
  `login`, `status`, `logout`, `client import`, `profiles`, `about`, `folder`,
  and the family's `auth` spelling. Full `drive` scope by default, one
  profile per Google account, no default profile.
- Addresses `gdrive:<profile>/<folder id>[/<path>]`, where `root` is My Drive.
- File commands: `ls [-R]`, `cat`, `cp` (files and trees, both ways, every
  md5 compared, unchanged files skipped), `mv`, `rm` (to the trash), `mkdir`,
  `md5sum`, `duplicates`. `--json` on every command.
- With no terminal on stdin, a keychain read waits at most 60 seconds
  (`GDRV_KEYCHAIN_WAIT`) and then exits 3.
- Built on cli-common v0.37.2.
