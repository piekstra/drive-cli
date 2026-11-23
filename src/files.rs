//! The file commands: ls, cat, cp, mv, rm, mkdir, md5sum, duplicates.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use pk_cli_core::{confirm, output, CliError};
use pk_cli_drive::google::{md5_file, Drive, DriveCli, Existing, ItemKind, ListedItem, Listing};
use pk_cli_drive::{reason, RemoteRead, RemoteStore};
use serde_json::{json, Map, Value};

use crate::address::{join, split, DriveAddress, Location};
use crate::cli::{CatArgs, CpArgs, HashArgs, LsArgs, MkdirArgs, MvArgs, RmArgs, RmdirArgs};
use crate::context::{open, BIN};

/// What is at a path in a store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Folder,
    /// A file with bytes, and the md5 Drive holds for it.
    File(String),
    /// A file with no md5: one of Google's own documents (Docs, Sheets),
    /// which has no bytes of its own.
    Document,
    Missing,
}

/// What `rel` is in `d`. The empty path is the address's folder, checked to
/// be one (a wrong id is exit 4).
pub fn item(d: &Drive, rel: &str) -> Result<Item, CliError> {
    if rel.is_empty() {
        return Ok(if d.exists("")? {
            Item::Folder
        } else {
            Item::Missing
        });
    }
    if let Some(md5) = d.md5(rel)? {
        return Ok(Item::File(md5));
    }
    if !d.exists(rel)? {
        return Ok(Item::Missing);
    }
    let (parent, name) = split(rel);
    let found = named(d, parent, name)?;
    let folders = found.iter().filter(|i| i.kind == ItemKind::Folder).count();
    match (folders, found.len() - folders) {
        (1, 0) => Ok(Item::Folder),
        // A document, which the bytes listing leaves out.
        (0, 0) | (0, 1) => Ok(Item::Document),
        _ => Err(shared(d, rel, &found)),
    }
}

/// The items named `name` directly in the folder `parent`, from a listing
/// that shows every item, so two that share the name are both there.
fn named(d: &Drive, parent: &str, name: &str) -> Result<Vec<ListedItem>, CliError> {
    Ok(d.list_items(parent, false)?
        .into_iter()
        .filter(|i| i.rel == name)
        .collect())
}

/// A path that names more than one item.
fn shared(d: &Drive, rel: &str, found: &[ListedItem]) -> CliError {
    let ids: Vec<&str> = found.iter().map(|i| i.id.as_str()).collect();
    CliError::Other(format!(
        "{} names {} items (ids {}); Drive allows that, but a path must name one, so nothing \
         was guessed",
        d.target(rel),
        found.len(),
        ids.join(", ")
    ))
    .because(reason::DUPLICATE_NAME)
}

/// An address that must be in Drive.
fn drive_address(text: &str, command: &str) -> Result<DriveAddress, CliError> {
    match Location::parse(text)? {
        Location::Drive(a) => Ok(a),
        Location::Local { .. } => Err(CliError::Usage(format!(
            "`gdrv {command}` works on Drive: `{}` is a local path; write \
             gdrive:<profile>/<folder id>[/<path>]",
            text.escape_debug()
        ))),
    }
}

/// `--profile`, when given, must be the address's.
pub fn check_profile(given: Option<&str>, address: &DriveAddress) -> Result<(), CliError> {
    match given {
        Some(p) if p != address.store.profile().as_str() => Err(CliError::Usage(format!(
            "--profile {p} names another profile than the address's ({}); a file command takes \
             its profile from the address",
            address.store.profile()
        ))),
        _ => Ok(()),
    }
}

fn not_found(d: &Drive, rel: &str) -> CliError {
    CliError::NotFound(format!("{}: no such file or folder", d.target(rel)))
}

fn a_document(d: &Drive, rel: &str) -> CliError {
    CliError::Usage(format!(
        "{} is one of Google's own documents (Docs, Sheets, …): it has no bytes of its own, so \
         there is nothing to read or copy",
        d.target(rel)
    ))
    .because(reason::GOOGLE_DOCUMENT)
    .with_remedy("export it from Google Drive (File → Download) as the format you want")
}

/// The remedy an error can name for a gdrv command.
fn explained(e: CliError, address: &str) -> CliError {
    match e.reason() {
        Some(r) if r == reason::DUPLICATE_NAME => e.with_remedy(format!(
            "`gdrv duplicates {address}` lists every name two items share, with their ids; \
             address a folder by its own id (gdrive:<profile>/<id>), or keep one of each in \
             Google Drive and rename the others"
        )),
        _ => e,
    }
}

fn rfc3339(secs: Option<i64>) -> Option<String> {
    secs.map(pk_cli_core::dates::fmt_rfc3339)
}

fn kind_name(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Folder => "folder",
        ItemKind::File => "file",
        ItemKind::Document => "document",
    }
}

/// One listed item: its path under the listed folder, its type and id, and
/// what Drive holds for it.
fn item_row(path: &str, i: &ListedItem) -> Value {
    let mut row = Map::new();
    row.insert("path".into(), json!(path));
    row.insert("type".into(), json!(kind_name(i.kind)));
    row.insert("id".into(), json!(i.id));
    if let Some(size) = i.size {
        row.insert("size".into(), json!(size));
    }
    if let Some(at) = rfc3339(i.modified) {
        row.insert("modified".into(), json!(at));
    }
    if let Some(md5) = &i.md5 {
        row.insert("md5".into(), json!(md5));
    }
    if i.via_shortcut {
        row.insert("via_shortcut".into(), json!(true));
    }
    Value::Object(row)
}

// ── ls ──────────────────────────────────────────────────────────────────

pub fn ls(
    cli: &DriveCli,
    profile: Option<&str>,
    args: &LsArgs,
    json: bool,
) -> Result<(), CliError> {
    let a = drive_address(&args.address, "ls")?;
    check_profile(profile, &a)?;
    let d = open(cli, &a.store, args.listing.listing())?;
    let items = list(&d, &a, args.recursive).map_err(|e| explained(e, &args.address))?;
    let payload = json!({
        "location": d.target(&a.rel),
        "recursive": args.recursive,
        "listing": args.listing.name(),
        "items": items,
    });
    output::emit(json, "drive-entry-list", payload, |v| {
        let rows: Vec<Value> = output::rows_of(v, "items")
            .into_iter()
            .map(|mut r| {
                if r["type"] == "folder" {
                    let shown = format!("{}/", r["path"].as_str().unwrap_or_default());
                    r["path"] = json!(shown);
                }
                r
            })
            .collect();
        output::table(&output::table_view(
            &rows,
            &["path", "type", "size", "modified", "md5", "id"],
        ));
    });
    Ok(())
}

fn list(d: &Drive, a: &DriveAddress, recursive: bool) -> Result<Vec<Value>, CliError> {
    match item(d, &a.rel)? {
        Item::Missing => Err(not_found(d, &a.rel)),
        // Every file below, as `rclone lsjson -R --files-only` lists.
        Item::Folder if recursive => Ok(d
            .list_items(&a.rel, true)?
            .iter()
            .filter(|i| i.kind != ItemKind::Folder)
            .map(|i| item_row(&i.rel, i))
            .collect()),
        Item::Folder => Ok(d
            .list_items(&a.rel, false)?
            .iter()
            .map(|i| item_row(&i.rel, i))
            .collect()),
        found @ (Item::File(_) | Item::Document) => {
            let (parent, name) = split(&a.rel);
            let listed = named(d, parent, name)?
                .into_iter()
                .find(|i| i.kind != ItemKind::Folder);
            Ok(vec![match (listed, found) {
                (Some(i), _) => item_row(name, &i),
                (None, Item::File(md5)) => json!({"path": name, "type": "file", "md5": md5}),
                // A document under the bytes listing, which leaves it out.
                (None, _) => json!({"path": name, "type": "document"}),
            }])
        }
    }
}

// ── cat ─────────────────────────────────────────────────────────────────

pub fn cat(
    cli: &DriveCli,
    profile: Option<&str>,
    args: &CatArgs,
    json: bool,
) -> Result<(), CliError> {
    let a = drive_address(&args.address, "cat")?;
    check_profile(profile, &a)?;
    if a.rel.is_empty() {
        return Err(CliError::Usage(
            "name a file under the folder id: gdrive:<profile>/<folder id>/<path>".into(),
        ));
    }
    let d = open(cli, &a.store, Listing::Whole)?;
    let md5 = match item(&d, &a.rel).map_err(|e| explained(e, &args.address))? {
        Item::File(md5) => md5,
        Item::Missing => return Err(not_found(&d, &a.rel)),
        Item::Document => return Err(a_document(&d, &a.rel)),
        Item::Folder => {
            return Err(CliError::Usage(format!(
                "{} is a folder; `gdrv ls {}` lists it",
                d.target(&a.rel),
                args.address
            )))
        }
    };
    let tmp = pk_cli_drive::private_temp_dir(&format!("{BIN}-cat"))?;
    let path = tmp.path().join("file");
    d.copy_out(&a.rel, &path)?;
    if json {
        let bytes = fs::read(&path).map_err(|e| CliError::Other(format!("reading back: {e}")))?;
        let (encoding, content) = match String::from_utf8(bytes) {
            Ok(text) => ("utf-8", text),
            Err(e) => ("base64", crate::base64::encode(e.as_bytes())),
        };
        let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        output::emit(
            true,
            "drive-file",
            json!({
                "location": d.target(&a.rel),
                "size": size,
                "md5": md5,
                "encoding": encoding,
                "content": content,
            }),
            |_| {},
        );
        return Ok(());
    }
    let mut file = fs::File::open(&path).map_err(|e| CliError::Other(format!("reading: {e}")))?;
    let mut out = std::io::stdout().lock();
    match std::io::copy(&mut file, &mut out).and_then(|_| out.flush()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(CliError::Other(format!("writing to stdout: {e}"))),
    }
}

// ── cp ──────────────────────────────────────────────────────────────────

/// What a copy did, in files.
#[derive(Debug, Default)]
struct Copied {
    copied: usize,
    replaced: usize,
    unchanged: usize,
    skipped_documents: usize,
    bytes: u64,
}

/// The remedy for a copy refused over files with other bytes: `--force`,
/// and what replacing costs on that side.
fn force_remedy(e: CliError, force: bool, upload: bool) -> CliError {
    if force || e.reason() != Some(reason::EXISTING_DIFFERS) {
        return e;
    }
    e.with_remedy(if upload {
        "pass --force to replace them; Drive keeps each replaced file's earlier revisions"
    } else {
        "pass --force to replace them; the local files' bytes are overwritten, so copy them \
         elsewhere first if they are wanted"
    })
}

fn other_bytes(target: &str) -> CliError {
    CliError::Usage(format!(
        "{target} is already there with other bytes; nothing was copied"
    ))
    .because(reason::EXISTING_DIFFERS)
}

/// `cp --transfers`, when given, lowers how many files a tree copy moves
/// at once.
fn with_transfers(d: Drive, transfers: Option<u8>) -> Drive {
    match transfers {
        Some(n) => d.transfers(usize::from(n)),
        None => d,
    }
}

pub fn cp(
    cli: &DriveCli,
    profile: Option<&str>,
    args: &CpArgs,
    json: bool,
) -> Result<(), CliError> {
    let from = Location::parse(&args.source)?;
    let to = Location::parse(&args.destination)?;
    let (direction, source, destination, done) =
        match (from, to) {
            (Location::Local { path, .. }, Location::Drive(a)) => {
                check_profile(profile, &a)?;
                let d = with_transfers(open(cli, &a.store, Listing::Whole)?, args.transfers);
                let (target, done) = upload(&d, &path, &a, args.force)
                    .map_err(|e| explained(force_remedy(e, args.force, true), &args.destination))?;
                ("upload", path.display().to_string(), target, done)
            }
            (Location::Drive(a), Location::Local { path, into }) => {
                check_profile(profile, &a)?;
                let d = with_transfers(open(cli, &a.store, Listing::Whole)?, args.transfers);
                let (target, done) = download(&d, &a, &path, into, args.force)
                    .map_err(|e| explained(force_remedy(e, args.force, false), &args.source))?;
                ("download", d.target(&a.rel), target, done)
            }
            (Location::Drive(_), Location::Drive(_)) => return Err(CliError::Usage(
                "gdrv copies between this machine and Drive, not within Drive; `gdrv mv` moves \
                 a file within one folder's tree"
                    .into(),
            )),
            (Location::Local { .. }, Location::Local { .. }) => {
                return Err(CliError::Usage(
                    "one side of a copy is a Drive address: gdrive:<profile>/<folder id>[/<path>]"
                        .into(),
                ))
            }
        };
    let payload = json!({
        "direction": direction,
        "source": source,
        "destination": destination,
        "copied": done.copied,
        "replaced": done.replaced,
        "unchanged": done.unchanged,
        "skipped_documents": done.skipped_documents,
        "bytes": done.bytes,
    });
    output::emit(json, "drive-copy", payload, |_| {
        let mut parts = vec![format!(
            "{} {} file(s), {} bytes",
            if direction == "upload" {
                "Uploaded"
            } else {
                "Downloaded"
            },
            done.copied + done.replaced,
            done.bytes
        )];
        if done.replaced > 0 {
            parts.push(format!("{} of them replaced", done.replaced));
        }
        if done.unchanged > 0 {
            parts.push(format!("{} unchanged", done.unchanged));
        }
        if done.skipped_documents > 0 {
            parts.push(format!(
                "{} Google document(s) skipped",
                done.skipped_documents
            ));
        }
        println!("{} → {destination}: {}.", source, parts.join("; "));
    });
    Ok(())
}

/// The local file's name, as a Drive name.
fn file_name(path: &Path) -> Result<String, CliError> {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .ok_or_else(|| {
            CliError::Usage(format!(
                "{} has no file name that is valid UTF-8",
                path.display()
            ))
        })
}

fn upload(
    d: &Drive,
    local: &Path,
    a: &DriveAddress,
    force: bool,
) -> Result<(String, Copied), CliError> {
    let meta = fs::metadata(local).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            CliError::NotFound(format!("{}: no such file or directory", local.display()))
        }
        _ => CliError::Other(format!("reading {}: {e}", local.display())),
    })?;
    if meta.is_dir() {
        match item(d, &a.rel)? {
            Item::File(_) | Item::Document => {
                return Err(CliError::Usage(format!(
                    "{} is a file; a directory's contents go into a folder",
                    d.target(&a.rel)
                )))
            }
            Item::Folder | Item::Missing => {}
        }
        let existing = if force {
            Existing::Replace
        } else {
            Existing::Keep
        };
        let report = d.put_tree(local, &a.rel, existing)?;
        return Ok((
            d.target(&a.rel),
            Copied {
                copied: report.uploaded,
                replaced: report.replaced,
                unchanged: report.unchanged,
                skipped_documents: 0,
                bytes: report.bytes,
            },
        ));
    }
    if !meta.is_file() {
        return Err(CliError::Usage(format!(
            "{} is neither a file nor a directory",
            local.display()
        )));
    }
    let name = file_name(local)?;
    let at = item(d, &a.rel)?;
    let (dest, there) = match at {
        Item::Folder => (join(&a.rel, &name), None),
        _ if a.into => (join(&a.rel, &name), None),
        other => (a.rel.clone(), Some(other)),
    };
    let there = match there {
        Some(known) => known,
        None => item(d, &dest)?,
    };
    let target = d.target(&dest);
    let md5 = md5_file(local)?;
    let mut done = Copied::default();
    match there {
        Item::Folder => {
            return Err(CliError::Usage(format!(
                "{target} is a folder; name the file inside it, or end the address with `/`"
            )))
        }
        Item::Document => return Err(a_document(d, &dest)),
        Item::File(m) if m == md5 => {
            done.unchanged = 1;
            return Ok((target, done));
        }
        Item::File(_) if !force => return Err(other_bytes(&target)),
        Item::File(_) => done.replaced = 1,
        Item::Missing => done.copied = 1,
    }
    d.copy_in(local, &dest)?;
    match d.md5(&dest)? {
        Some(m) if m == md5 => {}
        other => {
            return Err(CliError::Upstream(format!(
                "{target}: Drive holds md5 {} after the upload, not the local file's {md5}",
                other.as_deref().unwrap_or("none")
            )))
        }
    }
    done.bytes = meta.len();
    Ok((target, done))
}

/// A local directory for a download: made owner-only when missing.
fn private_dir(dir: &Path) -> Result<(), CliError> {
    pk_cli_core::private_file::create_dir_all(dir)
        .map_err(|e| CliError::Other(format!("making {}: {e}", dir.display())))
}

/// A path from Drive's names is plain relative parts once joined locally.
fn plain(rel: &str) -> bool {
    !rel.is_empty()
        && Path::new(rel)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}

fn download(
    d: &Drive,
    a: &DriveAddress,
    local: &Path,
    into: bool,
    force: bool,
) -> Result<(String, Copied), CliError> {
    let mut done = Copied::default();
    match item(d, &a.rel)? {
        Item::Missing => Err(not_found(d, &a.rel)),
        Item::Document => Err(a_document(d, &a.rel)),
        Item::Folder => {
            if local.is_file() {
                return Err(CliError::Usage(format!(
                    "{} is a file; a folder's contents go into a directory",
                    local.display()
                )));
            }
            let entries = d.list_entries(&a.rel)?;
            let mut differ = Vec::new();
            let mut sizes = 0;
            for e in &entries {
                let Some(md5) = &e.md5 else {
                    done.skipped_documents += 1;
                    continue;
                };
                // fetch_tree refuses such a name before writing; nothing is
                // read here for it either.
                if !plain(&e.rel) {
                    continue;
                }
                let at = local.join(&e.rel);
                if at.is_dir() {
                    return Err(CliError::Usage(format!(
                        "{} is a directory, where {} would go",
                        at.display(),
                        d.target(&join(&a.rel, &e.rel))
                    )));
                }
                if at.is_file() {
                    if md5_file(&at)? == *md5 {
                        done.unchanged += 1;
                        continue;
                    }
                    differ.push(at.display().to_string());
                }
                sizes += e.size.unwrap_or(0);
            }
            if !differ.is_empty() && !force {
                return Err(CliError::Usage(format!(
                    "refusing to replace {} local file(s) that hold other bytes ({}{}); nothing \
                     was downloaded",
                    differ.len(),
                    differ
                        .iter()
                        .take(3)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", "),
                    if differ.len() > 3 { ", …" } else { "" }
                ))
                .because(reason::EXISTING_DIFFERS));
            }
            private_dir(local)?;
            let fetched = d.fetch_tree(&a.rel, local)?;
            done.replaced = differ.len().min(fetched);
            done.copied = fetched - done.replaced;
            done.bytes = sizes;
            Ok((local.display().to_string(), done))
        }
        Item::File(md5) => {
            let dest: PathBuf = if into || local.is_dir() {
                local.join(a.name())
            } else {
                local.to_path_buf()
            };
            if dest.is_dir() {
                return Err(CliError::Usage(format!(
                    "{} is a directory; name the file inside it, or end the path with `/`",
                    dest.display()
                )));
            }
            if dest.is_file() {
                if md5_file(&dest)? == md5 {
                    done.unchanged = 1;
                    return Ok((dest.display().to_string(), done));
                }
                if !force {
                    return Err(other_bytes(&dest.display().to_string()));
                }
                done.replaced = 1;
            } else {
                done.copied = 1;
            }
            if let Some(parent) = dest.parent().filter(|p| !p.as_os_str().is_empty()) {
                if !parent.exists() {
                    private_dir(parent)?;
                }
            }
            d.copy_out(&a.rel, &dest)?;
            if md5_file(&dest)? != md5 {
                return Err(CliError::Other(format!(
                    "{}: the file written does not match Drive's md5",
                    dest.display()
                )));
            }
            done.bytes = fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
            Ok((dest.display().to_string(), done))
        }
    }
}

// ── mv ──────────────────────────────────────────────────────────────────

pub fn mv(
    cli: &DriveCli,
    profile: Option<&str>,
    args: &MvArgs,
    json: bool,
) -> Result<(), CliError> {
    let from = drive_address(&args.source, "mv")?;
    let to = drive_address(&args.destination, "mv")?;
    check_profile(profile, &from)?;
    if from.store != to.store {
        return Err(CliError::Usage(format!(
            "gdrv mv moves within one folder id's tree, and {} and {} are two; copy the file \
             down and up again to move it elsewhere",
            from.store, to.store
        )));
    }
    if from.rel.is_empty() {
        return Err(CliError::Usage(
            "name the file to move under the folder id".into(),
        ));
    }
    let d = open(cli, &from.store, Listing::Whole)?;
    let moved = move_file(&d, &from, &to, args.force).map_err(|e| explained(e, &args.source))?;
    let payload = json!({
        "from": d.target(&from.rel),
        "to": d.target(&moved.0),
        "replaced": moved.1,
    });
    output::emit(json, "drive-move", payload, |v| {
        println!(
            "Moved {} → {}{}.",
            v["from"].as_str().unwrap_or_default(),
            v["to"].as_str().unwrap_or_default(),
            if moved.1 {
                " (the file there went to the trash)"
            } else {
                ""
            }
        );
    });
    Ok(())
}

/// The destination path and whether a file there was replaced.
fn move_file(
    d: &Drive,
    from: &DriveAddress,
    to: &DriveAddress,
    force: bool,
) -> Result<(String, bool), CliError> {
    match item(d, &from.rel)? {
        Item::Missing => return Err(not_found(d, &from.rel)),
        Item::Folder => {
            return Err(CliError::Usage(format!(
                "{} is a folder; gdrv mv moves files",
                d.target(&from.rel)
            )))
        }
        Item::File(_) | Item::Document => {}
    }
    let at = item(d, &to.rel)?;
    let (dest, there) = match at {
        Item::Folder => (join(&to.rel, from.name()), None),
        _ if to.into => (join(&to.rel, from.name()), None),
        other => (to.rel.clone(), Some(other)),
    };
    if dest == from.rel {
        return Err(CliError::Usage(format!(
            "{} is already there",
            d.target(&dest)
        )));
    }
    let there = match there {
        Some(known) => known,
        None => item(d, &dest)?,
    };
    let replaced = match there {
        Item::Folder => {
            return Err(CliError::Usage(format!(
                "{} is a folder; name the file inside it, or end the address with `/`",
                d.target(&dest)
            )))
        }
        Item::File(_) | Item::Document if !force => {
            return Err(occupied(d, &dest));
        }
        Item::File(_) | Item::Document => true,
        Item::Missing => false,
    };
    let res = if force {
        d.move_to(&from.rel, &dest)
    } else {
        d.move_no_clobber(&from.rel, &dest)
    };
    res.map_err(|e| {
        if e.reason() == Some(reason::OCCUPIED) {
            occupied(d, &dest)
        } else {
            e
        }
    })?;
    if !d.exists(&dest)? || d.exists(&from.rel)? {
        return Err(CliError::Upstream(format!(
            "Drive answered the move, but {} is not where it was moved to",
            d.target(&from.rel)
        )));
    }
    Ok((dest, replaced))
}

fn occupied(d: &Drive, rel: &str) -> CliError {
    CliError::Usage(format!(
        "{} is already there; nothing was moved",
        d.target(rel)
    ))
    .because(reason::OCCUPIED)
    .with_remedy("pass --force to replace it; the file there goes to the Google Drive trash")
}

/// A trash that Drive did not carry out, or that did not read back.
fn not_trashed(target: &str) -> CliError {
    CliError::Upstream(format!("{target} was not moved to the trash"))
}

/// An error partway through trashing several items, naming the ones already
/// in the trash, so whoever runs it again knows what is left.
fn after_trashing(e: CliError, trashed: &[String]) -> CliError {
    if trashed.is_empty() {
        return e;
    }
    let done = format!("already trashed: {}", trashed.join(", "));
    let remedy = match e.remedy() {
        Some(r) => format!("{r}; {done}"),
        None => done,
    };
    e.with_remedy(remedy)
}

// ── rm ──────────────────────────────────────────────────────────────────

pub fn rm(
    cli: &DriveCli,
    profile: Option<&str>,
    args: &RmArgs,
    json: bool,
    interactive: bool,
) -> Result<(), CliError> {
    let addresses = args
        .addresses
        .iter()
        .map(|t| drive_address(t, "rm"))
        .collect::<Result<Vec<_>, _>>()?;
    for a in &addresses {
        check_profile(profile, a)?;
        if a.rel.is_empty() {
            return Err(CliError::Usage(
                "name a file under the folder id; gdrv rm moves files to the trash, never a \
                 folder"
                    .into(),
            ));
        }
    }
    // Before any keychain read or request: a script that forgot --force
    // gets exit 6 having touched nothing.
    confirm::require_confirmable(
        args.force,
        interactive,
        &format!(
            "moving {} file(s) to the Google Drive trash",
            addresses.len()
        ),
    )?;
    let mut stores = Vec::new();
    for (a, text) in addresses.iter().zip(&args.addresses) {
        let d = open(cli, &a.store, Listing::Whole)?;
        match item(&d, &a.rel).map_err(|e| explained(e, text))? {
            Item::File(_) | Item::Document => {}
            Item::Missing => return Err(not_found(&d, &a.rel)),
            Item::Folder => {
                return Err(CliError::Usage(format!(
                    "{} is a folder; gdrv rm moves files to the trash, never a folder",
                    d.target(&a.rel)
                )))
            }
        }
        stores.push(d);
    }
    let shown: Vec<String> = stores
        .iter()
        .zip(&addresses)
        .map(|(d, a)| d.target(&a.rel))
        .collect();
    confirm::confirm(
        args.force,
        &format!(
            "Move to the Google Drive trash (recoverable there for 30 days)?\n  {}\n",
            shown.join("\n  ")
        ),
    )?;
    let mut trashed = Vec::new();
    for ((d, a), target) in stores.iter().zip(&addresses).zip(&shown) {
        let gone = d
            .delete_file(&a.rel)
            .and_then(|done| Ok(done && !d.exists(&a.rel)?))
            .map_err(|e| after_trashing(e, &trashed))?;
        if !gone {
            return Err(after_trashing(not_trashed(target), &trashed));
        }
        trashed.push(target.clone());
    }
    let payload = json!({
        "trashed": trashed.len(),
        "items": trashed.iter().map(|t| json!({"location": t})).collect::<Vec<_>>(),
    });
    output::emit(json, "drive-trash", payload, |_| {
        for t in &trashed {
            println!("Trashed {t}");
        }
    });
    Ok(())
}

// ── rmdir ───────────────────────────────────────────────────────────────

fn not_empty(d: &Drive, rel: &str, held: usize) -> CliError {
    CliError::Usage(format!(
        "{} holds {held} item(s); only an empty folder is trashed, so nothing was",
        d.target(rel)
    ))
    .because(reason::NOT_EMPTY)
    .with_remedy("move or trash what it holds first (`gdrv rm`, then `gdrv rmdir`)")
}

pub fn rmdir(
    cli: &DriveCli,
    profile: Option<&str>,
    args: &RmdirArgs,
    json: bool,
    interactive: bool,
) -> Result<(), CliError> {
    let addresses = args
        .addresses
        .iter()
        .map(|t| drive_address(t, "rmdir"))
        .collect::<Result<Vec<_>, _>>()?;
    for a in &addresses {
        check_profile(profile, a)?;
        if a.rel.is_empty() {
            return Err(CliError::Usage(
                "name a folder under the folder id; gdrv rmdir never trashes the address's own \
                 folder (address it from its parent, or from root)"
                    .into(),
            ));
        }
    }
    confirm::require_confirmable(
        args.force,
        interactive,
        &format!(
            "moving {} folder(s) to the Google Drive trash",
            addresses.len()
        ),
    )?;
    let mut stores = Vec::new();
    for (a, text) in addresses.iter().zip(&args.addresses) {
        let d = open(cli, &a.store, Listing::Whole)?;
        match item(&d, &a.rel).map_err(|e| explained(e, text))? {
            Item::Folder => {}
            Item::Missing => return Err(not_found(&d, &a.rel)),
            Item::File(_) | Item::Document => {
                return Err(CliError::Usage(format!(
                    "{} is a file; `gdrv rm` moves files to the trash",
                    d.target(&a.rel)
                )))
            }
        }
        let held = d.list_items(&a.rel, false)?.len();
        if held > 0 {
            return Err(not_empty(&d, &a.rel, held));
        }
        stores.push(d);
    }
    let shown: Vec<String> = stores
        .iter()
        .zip(&addresses)
        .map(|(d, a)| d.target(&a.rel))
        .collect();
    confirm::confirm(
        args.force,
        &format!(
            "Move these empty folders to the Google Drive trash (recoverable there for 30 \
             days)?\n  {}\n",
            shown.join("\n  ")
        ),
    )?;
    let mut trashed = Vec::new();
    for ((d, a), target) in stores.iter().zip(&addresses).zip(&shown) {
        let gone = d
            .trash_empty_folder(&a.rel)
            .and_then(|done| Ok(done && item(d, &a.rel)? == Item::Missing))
            .map_err(|e| after_trashing(e, &trashed))?;
        if !gone {
            return Err(after_trashing(not_trashed(target), &trashed));
        }
        trashed.push(target.clone());
    }
    let payload = json!({
        "trashed": trashed.len(),
        "items": trashed.iter().map(|t| json!({"location": t})).collect::<Vec<_>>(),
    });
    output::emit(json, "drive-rmdir", payload, |_| {
        for t in &trashed {
            println!("Trashed {t}/");
        }
    });
    Ok(())
}

// ── mkdir ───────────────────────────────────────────────────────────────

pub fn mkdir(
    cli: &DriveCli,
    profile: Option<&str>,
    args: &MkdirArgs,
    json: bool,
) -> Result<(), CliError> {
    let a = drive_address(&args.address, "mkdir")?;
    check_profile(profile, &a)?;
    if a.rel.is_empty() {
        return Err(CliError::Usage(
            "name the folder to make under the folder id; `gdrv folder create <NAME>` makes one \
             at the top of My Drive"
                .into(),
        ));
    }
    let d = open(cli, &a.store, Listing::Whole)?;
    let existed = match item(&d, &a.rel).map_err(|e| explained(e, &args.address))? {
        Item::Folder => true,
        Item::Missing => false,
        Item::File(_) | Item::Document => {
            return Err(CliError::Usage(format!(
                "{} is a file; Drive would hold a file and a folder of one name",
                d.target(&a.rel)
            )))
        }
    };
    if !existed {
        d.mkdir(&a.rel)?;
        if item(&d, &a.rel)? != Item::Folder {
            return Err(CliError::Upstream(format!(
                "Drive answered, but {} is not a folder after it",
                d.target(&a.rel)
            )));
        }
    }
    let payload = json!({"location": d.target(&a.rel), "created": !existed});
    output::emit(json, "drive-mkdir", payload, |v| {
        println!(
            "{} {}",
            if existed { "Already there:" } else { "Made" },
            v["location"].as_str().unwrap_or_default()
        );
    });
    Ok(())
}

// ── md5sum ──────────────────────────────────────────────────────────────

pub fn md5sum(
    cli: &DriveCli,
    profile: Option<&str>,
    args: &HashArgs,
    json: bool,
) -> Result<(), CliError> {
    let a = drive_address(&args.address, "md5sum")?;
    check_profile(profile, &a)?;
    let d = open(cli, &a.store, args.listing.listing())?;
    let items = hashes(&d, &a).map_err(|e| explained(e, &args.address))?;
    let payload = json!({
        "location": d.target(&a.rel),
        "listing": args.listing.name(),
        "items": items,
    });
    output::emit(json, "drive-hash-list", payload, |v| {
        let mut out = std::io::stdout().lock();
        for row in output::rows_of(v, "items") {
            let line = format!(
                "{}  {}",
                row["md5"].as_str().unwrap_or_default(),
                row["path"].as_str().unwrap_or_default()
            );
            if writeln!(out, "{line}").is_err() {
                return;
            }
        }
    });
    Ok(())
}

fn hashes(d: &Drive, a: &DriveAddress) -> Result<Vec<Value>, CliError> {
    match item(d, &a.rel)? {
        Item::Missing => Err(not_found(d, &a.rel)),
        Item::Document => Err(a_document(d, &a.rel)),
        Item::File(md5) => {
            let (parent, name) = split(&a.rel);
            let listed = named(d, parent, name)?
                .into_iter()
                .find(|i| i.kind == ItemKind::File);
            let mut row = json!({"path": name, "md5": md5});
            if let Some(i) = listed {
                row["id"] = json!(i.id);
                if let Some(size) = i.size {
                    row["size"] = json!(size);
                }
            }
            Ok(vec![row])
        }
        Item::Folder => Ok(d
            .list_items(&a.rel, true)?
            .into_iter()
            .filter(|i| i.kind == ItemKind::File)
            .filter_map(|i| {
                let md5 = i.md5?;
                Some(json!({"path": i.rel, "id": i.id, "size": i.size, "md5": md5}))
            })
            .collect()),
    }
}

// ── duplicates ──────────────────────────────────────────────────────────

pub fn duplicates(
    cli: &DriveCli,
    profile: Option<&str>,
    args: &HashArgs,
    json: bool,
) -> Result<(), CliError> {
    let a = drive_address(&args.address, "duplicates")?;
    check_profile(profile, &a)?;
    let d = open(cli, &a.store, args.listing.listing())?;
    match item(&d, &a.rel)? {
        Item::Folder => {}
        Item::Missing => return Err(not_found(&d, &a.rel)),
        Item::File(_) | Item::Document => {
            return Err(CliError::Usage(format!(
                "{} is a file; gdrv duplicates looks in a folder",
                d.target(&a.rel)
            )))
        }
    }
    let found = d.duplicates(&a.rel)?;
    let items: Vec<Value> = found
        .iter()
        .map(|g| {
            json!({
                "dir": g.dir,
                "dir_id": g.dir_id,
                "name": g.name,
                "type": if g.folders { "folder" } else { "file" },
                "ids": g.ids,
            })
        })
        .collect();
    let payload = json!({
        "location": d.target(&a.rel),
        "listing": args.listing.name(),
        "items": items,
    });
    output::emit(json, "drive-duplicate-list", payload, |v| {
        let rows: Vec<Value> = output::rows_of(v, "items")
            .into_iter()
            .map(|r| {
                let ids: Vec<&str> = r["ids"]
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                json!({
                    "dir": r["dir"],
                    "name": r["name"],
                    "type": r["type"],
                    "ids": ids.join(", "),
                })
            })
            .collect();
        if rows.is_empty() {
            println!(
                "No two items share a name below {}.",
                v["location"].as_str().unwrap_or_default()
            );
            return;
        }
        output::table(&output::table_view(&rows, &["dir", "name", "type", "ids"]));
    });
    Ok(())
}
