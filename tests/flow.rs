//! Every command through the binary against pk-cli-drive's fake Drive server
//! on 127.0.0.1, with synthetic files only. No keychain (the vault's key is
//! in a test file), no Google.

mod support;

use pk_cli_drive::google::fake::{ACCOUNT, MY_DRIVE, ROOT_ID};
use serde_json::Value;
use support::{md5_hex, s, Env, PROFILE};

fn store() -> String {
    format!("gdrive:{PROFILE}/{ROOT_ID}")
}

fn at(rel: &str) -> String {
    format!("{}/{rel}", store())
}

fn paths(v: &Value) -> Vec<String> {
    v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["path"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn sign_in_status_about_folders_and_logout() {
    let e = Env::with_fake();
    let login = e.login(&[]);
    assert_eq!(login["schema"], "auth-login/v1");
    assert_eq!(login["username"], ACCOUNT);
    assert_eq!(
        login["scopes"],
        serde_json::json!(["drive"]),
        "full drive by default"
    );
    let status = e.ok(&["status", "--profile", PROFILE]);
    assert_eq!(status["schema"], "auth-status/v1");
    assert_eq!(status["authenticated"], true);
    assert_eq!(status["session_valid"], true);
    let auth = e.ok(&["auth", "status", "--profile", PROFILE]);
    assert_eq!(auth["authenticated"], true);
    let about = e.ok(&["about", "--profile", PROFILE]);
    assert_eq!(about["schema"], "drive-about/v1");
    assert_eq!(about["account"], ACCOUNT);
    let env_profile = e
        .cmd()
        .env("GDRV_DRIVE_PROFILE", PROFILE)
        .args(["--json", "about"])
        .output()
        .unwrap();
    assert!(env_profile.status.success());
    let made = e.ok(&["folder", "create", "Scratch", "--profile", PROFILE]);
    assert_eq!(made["existing"], false);
    let spec = made["spec"].as_str().unwrap().to_string();
    assert!(spec.starts_with("gdrive:home/"), "{spec}");
    let again = e.ok(&["folder", "create", "Scratch", "--profile", PROFILE]);
    assert_eq!(again["existing"], true);
    let folders = e.ok(&["folder", "list", "--profile", PROFILE]);
    assert!(folders.to_string().contains("Scratch"), "{folders}");
    let listed = e.ok(&["profiles", "list"]);
    assert!(listed.to_string().contains(ACCOUNT), "{listed}");
    // The new folder is a store gdrv can write to.
    e.write("one.txt", b"one");
    e.ok(&["cp", s(&e.path("one.txt")), &format!("{spec}/")]);
    assert_eq!(paths(&e.ok(&["ls", &spec])), vec!["one.txt"]);
    let out = e.ok(&["logout", "--profile", PROFILE]);
    assert_eq!(out["signed_out"], true);
    let status = e.ok(&["status", "--profile", PROFILE, "--local"]);
    assert_eq!(status["authenticated"], false);
}

#[test]
fn a_client_imported_alone_then_a_scope_asked_for() {
    let e = Env::with_fake();
    let imported = e
        .cmd()
        .args([
            "--json",
            "client",
            "import",
            "--profile",
            PROFILE,
            "--stdin",
        ])
        .write_stdin(pk_cli_drive::google::fake::CLIENT_JSON)
        .output()
        .unwrap();
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    let login = e.login(&["--scope", "file"]);
    assert_eq!(login["scopes"], serde_json::json!(["drive.file"]));
}

#[test]
fn ls_one_level_recursive_one_file_and_listings() {
    let e = Env::signed_in();
    let f = e.fake();
    let sub = f.add_folder(ROOT_ID, "sub");
    f.add_file(ROOT_ID, "a.txt", b"aaa");
    f.add_file(&sub, "b.txt", b"bb");
    f.add_google_doc(ROOT_ID, "notes");
    let shared = f.add_file(MY_DRIVE, "elsewhere.pdf", b"%PDF-shared");
    f.add_shortcut(ROOT_ID, "link.pdf", &shared);

    let one = e.ok(&["ls", &store()]);
    assert_eq!(one["schema"], "drive-entry-list/v1");
    assert_eq!(one["listing"], "whole");
    assert_eq!(paths(&one), vec!["a.txt", "link.pdf", "notes", "sub"]);
    let items = one["items"].as_array().unwrap();
    let a = &items[0];
    assert_eq!(a["type"], "file");
    assert_eq!(a["size"], 3);
    assert_eq!(a["md5"], md5_hex(b"aaa"));
    assert!(a["modified"].as_str().unwrap().ends_with('Z'));
    assert_eq!(
        items[1]["md5"],
        md5_hex(b"%PDF-shared"),
        "a shortcut is its target"
    );
    assert_eq!(items[2]["type"], "document");
    assert!(items[2].get("md5").is_none());
    assert_eq!(items[3]["type"], "folder");

    let bytes = e.ok(&["ls", "--listing", "bytes", &store()]);
    assert_eq!(paths(&bytes), vec!["a.txt", "sub"]);

    let deep = e.ok(&["ls", "-R", &store()]);
    assert_eq!(
        paths(&deep),
        vec!["a.txt", "link.pdf", "notes", "sub/b.txt"]
    );
    assert_eq!(paths(&e.ok(&["ls", "-R", &at("sub")])), vec!["b.txt"]);
    let file = e.ok(&["ls", &at("sub/b.txt")]);
    assert_eq!(paths(&file), vec!["b.txt"]);
    assert_eq!(file["items"][0]["size"], 2);

    let err = e.fails(4, &["ls", &at("nothing")]);
    assert_eq!(err["code"], "not_found");
    let err = e.fails(4, &["ls", "gdrive:home/0BnoSuchFolderAtAll"]);
    assert_eq!(err["code"], "not_found");

    // Text mode: a table, folders with a trailing `/`.
    let out = e.run(&["ls", &store()]);
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("sub/"), "{text}");
    assert!(text.contains(&md5_hex(b"aaa")), "{text}");
}

#[test]
fn root_is_my_drive() {
    let e = Env::signed_in();
    e.fake().add_file(MY_DRIVE, "top.txt", b"top");
    let root = format!("gdrive:{PROFILE}/root");
    let listed = e.ok(&["ls", &root]);
    assert_eq!(listed["location"], root);
    assert_eq!(paths(&listed), vec!["Store", "top.txt"]);
    e.write("up/deep/x.txt", b"x");
    e.ok(&["cp", s(&e.path("up")), &format!("{root}/scratch")]);
    assert_eq!(
        paths(&e.ok(&["ls", "-R", &root])),
        vec!["scratch/deep/x.txt", "top.txt"]
    );
    let back = e.path("back");
    let down = e.ok(&["cp", &format!("{root}/scratch"), s(&back)]);
    assert_eq!(down["copied"], 1);
    assert_eq!(std::fs::read(back.join("deep/x.txt")).unwrap(), b"x");
    assert_eq!(
        paths(&e.ok(&["md5sum", &root])),
        vec!["scratch/deep/x.txt", "top.txt"]
    );
    e.ok(&[
        "mv",
        &format!("{root}/top.txt"),
        &format!("{root}/scratch/"),
    ]);
    assert_eq!(e.fake().children(MY_DRIVE, "top.txt").len(), 0);
    e.ok(&["rm", "--force", &format!("{root}/scratch/top.txt")]);
}

#[test]
fn cat_prints_bytes_and_refuses_what_has_none() {
    let e = Env::signed_in();
    let f = e.fake();
    f.add_file(ROOT_ID, "hello.txt", b"hello\n");
    f.add_file(ROOT_ID, "blob.bin", &[0xff, 0xfe, 0x00, 0x01]);
    f.add_google_doc(ROOT_ID, "sheet");
    f.add_folder(ROOT_ID, "dir");
    let out = e.run(&["cat", &at("hello.txt")]);
    assert!(out.status.success());
    assert_eq!(out.stdout, b"hello\n");
    let bin = e.run(&["cat", &at("blob.bin")]);
    assert_eq!(bin.stdout, [0xff, 0xfe, 0x00, 0x01]);
    let doc = e.ok(&["cat", &at("hello.txt")]);
    assert_eq!(doc["schema"], "drive-file/v1");
    assert_eq!(doc["encoding"], "utf-8");
    assert_eq!(doc["content"], "hello\n");
    assert_eq!(doc["md5"], md5_hex(b"hello\n"));
    let doc = e.ok(&["cat", &at("blob.bin")]);
    assert_eq!(doc["encoding"], "base64");
    assert_eq!(doc["content"], "//4AAQ==");
    let err = e.fails(2, &["cat", &at("sheet")]);
    assert_eq!(err["reason"], "google_document");
    assert_eq!(e.fails(2, &["cat", &at("dir")])["code"], "usage");
    assert_eq!(
        e.fails(4, &["cat", &at("missing.txt")])["code"],
        "not_found"
    );
}

#[test]
fn cp_a_file_up_and_down_compares_md5_and_refuses_other_bytes_without_force() {
    let e = Env::signed_in();
    let local = e.write("report.txt", b"v1");
    let up = e.ok(&["cp", s(&local), &at("inbox/")]);
    assert_eq!(up["schema"], "drive-copy/v1");
    assert_eq!(up["direction"], "upload");
    assert_eq!(up["copied"], 1);
    assert_eq!(up["bytes"], 2);
    assert_eq!(up["destination"], at("inbox/report.txt"));
    assert_eq!(e.fake().content("inbox/report.txt").unwrap(), b"v1");
    let id = e.fake().find("inbox/report.txt").unwrap();
    let same = e.ok(&["cp", s(&local), &at("inbox/report.txt")]);
    assert_eq!(
        (same["copied"].as_u64(), same["unchanged"].as_u64()),
        (Some(0), Some(1))
    );
    std::fs::write(&local, b"v2").unwrap();
    let err = e.fails(2, &["cp", s(&local), &at("inbox/report.txt")]);
    assert_eq!(err["reason"], "existing_differs");
    assert!(err["remedy"].as_str().unwrap().contains("--force"), "{err}");
    assert_eq!(e.fake().content("inbox/report.txt").unwrap(), b"v1");
    let forced = e.ok(&["cp", "--force", s(&local), &at("inbox/report.txt")]);
    assert_eq!(forced["replaced"], 1);
    assert_eq!(e.fake().content("inbox/report.txt").unwrap(), b"v2");
    assert_eq!(
        e.fake().find("inbox/report.txt").unwrap(),
        id,
        "replaced in place"
    );
    // Into an existing folder without the trailing slash.
    e.ok(&["cp", s(&local), &at("inbox")]);
    assert_eq!(e.fake().count("inbox", "report.txt"), 1);

    // Down: to a path, into a directory, unchanged, other bytes.
    let down = e.path("down/copy.txt");
    let got = e.ok(&["cp", &at("inbox/report.txt"), s(&down)]);
    assert_eq!(got["direction"], "download");
    assert_eq!(got["copied"], 1);
    assert_eq!(std::fs::read(&down).unwrap(), b"v2");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&down).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "downloads are owner-only");
    }
    let dir = e.path("down");
    e.ok(&["cp", &at("inbox/report.txt"), s(&dir)]);
    assert_eq!(std::fs::read(dir.join("report.txt")).unwrap(), b"v2");
    let same = e.ok(&["cp", &at("inbox/report.txt"), s(&down)]);
    assert_eq!(same["unchanged"], 1);
    std::fs::write(&down, b"local edit").unwrap();
    let err = e.fails(2, &["cp", &at("inbox/report.txt"), s(&down)]);
    assert_eq!(err["reason"], "existing_differs");
    assert_eq!(std::fs::read(&down).unwrap(), b"local edit");
    e.ok(&["cp", "--force", &at("inbox/report.txt"), s(&down)]);
    assert_eq!(std::fs::read(&down).unwrap(), b"v2");

    assert_eq!(
        e.fails(4, &["cp", s(&e.path("nope.txt")), &at("x.txt")])["code"],
        "not_found"
    );
    assert_eq!(
        e.fails(4, &["cp", &at("nope.txt"), s(&e.path("x.txt"))])["code"],
        "not_found"
    );
}

#[test]
fn cp_trees_both_ways_skip_what_matches() {
    let e = Env::signed_in();
    e.write("tree/a.json", b"a");
    e.write("tree/sub/b.json", b"b");
    e.write("tree/sub/c.json", b"c");
    let tree = e.path("tree");
    let up = e.ok(&["cp", s(&tree), &at("copy")]);
    assert_eq!(
        (up["copied"].as_u64(), up["unchanged"].as_u64()),
        (Some(3), Some(0))
    );
    let again = e.ok(&["cp", s(&tree), &at("copy")]);
    assert_eq!(
        (again["copied"].as_u64(), again["unchanged"].as_u64()),
        (Some(0), Some(3))
    );
    std::fs::write(tree.join("sub/c.json"), b"changed").unwrap();
    let err = e.fails(2, &["cp", s(&tree), &at("copy")]);
    assert_eq!(err["reason"], "existing_differs");
    assert!(err["remedy"].as_str().unwrap().contains("--force"));
    let forced = e.ok(&["cp", "--force", s(&tree), &at("copy")]);
    assert_eq!(forced["replaced"], 1);

    e.fake()
        .add_google_doc(&e.fake().find("copy").unwrap(), "plan");
    let back = e.path("back");
    let down = e.ok(&["cp", &at("copy"), s(&back)]);
    assert_eq!(down["copied"], 3);
    assert_eq!(down["skipped_documents"], 1);
    assert_eq!(std::fs::read(back.join("sub/c.json")).unwrap(), b"changed");
    let again = e.ok(&["cp", &at("copy"), s(&back)]);
    assert_eq!(
        (again["copied"].as_u64(), again["unchanged"].as_u64()),
        (Some(0), Some(3))
    );
    std::fs::write(back.join("a.json"), b"mine").unwrap();
    let err = e.fails(2, &["cp", &at("copy"), s(&back)]);
    assert_eq!(err["reason"], "existing_differs");
    assert_eq!(std::fs::read(back.join("a.json")).unwrap(), b"mine");
    let forced = e.ok(&["cp", "--force", &at("copy"), s(&back)]);
    assert_eq!(forced["replaced"], 1);
    assert_eq!(std::fs::read(back.join("a.json")).unwrap(), b"a");

    // A file in the way of a tree, either side.
    let err = e.fails(2, &["cp", s(&tree), &at("copy/a.json")]);
    assert_eq!(err["code"], "usage");
    let err = e.fails(2, &["cp", &at("copy"), s(&back.join("a.json"))]);
    assert_eq!(err["code"], "usage");
}

#[test]
fn a_download_refuses_names_that_would_leave_the_directory() {
    let e = Env::signed_in();
    let shared = e.fake().add_folder(ROOT_ID, "shared");
    e.fake().add_file(&shared, "fine.txt", b"fine");
    e.fake().add_file(&shared, "..", b"theirs");
    let back = e.path("back");
    let err = e.fails(1, &["cp", &at("shared"), s(&back)]);
    assert_eq!(err["reason"], "unsafe_name", "{err}");
    assert!(!e.root.join("theirs").exists());
}

#[test]
fn writes_do_not_go_through_shortcuts() {
    let e = Env::signed_in();
    let target = e.fake().add_folder(MY_DRIVE, "Theirs");
    e.fake().add_shortcut(ROOT_ID, "linked", &target);
    e.write("x.txt", b"x");
    let err = e.fails(2, &["cp", s(&e.path("x.txt")), &at("linked/")]);
    assert_eq!(err["reason"], "shortcut", "{err}");
    assert_eq!(e.fake().children(&target, "x.txt").len(), 0);
}

#[test]
fn mv_renames_and_moves_and_refuses_to_overwrite_without_force() {
    let e = Env::signed_in();
    let f = e.fake();
    f.add_file(ROOT_ID, "a.txt", b"a");
    f.add_file(ROOT_ID, "b.txt", b"b");
    f.add_folder(ROOT_ID, "done");
    let moved = e.ok(&["mv", &at("a.txt"), &at("done/")]);
    assert_eq!(moved["schema"], "drive-move/v1");
    assert_eq!(moved["to"], at("done/a.txt"));
    assert_eq!(moved["replaced"], false);
    e.ok(&["mv", &at("done/a.txt"), &at("done/renamed.txt")]);
    assert_eq!(f.content("done/renamed.txt").unwrap(), b"a");
    let err = e.fails(2, &["mv", &at("b.txt"), &at("done/renamed.txt")]);
    assert_eq!(err["reason"], "occupied");
    assert!(err["remedy"].as_str().unwrap().contains("--force"), "{err}");
    assert_eq!(f.content("b.txt").unwrap(), b"b");
    let old = f.find("done/renamed.txt").unwrap();
    let forced = e.ok(&["mv", "--force", &at("b.txt"), &at("done/renamed.txt")]);
    assert_eq!(forced["replaced"], true);
    assert_eq!(f.content("done/renamed.txt").unwrap(), b"b");
    assert!(f.is_trashed(&old), "the replaced file went to the trash");
    assert_eq!(
        e.fails(4, &["mv", &at("gone.txt"), &at("x.txt")])["code"],
        "not_found"
    );
    assert_eq!(
        e.fails(2, &["mv", &at("done"), &at("elsewhere")])["code"],
        "usage"
    );
}

#[test]
fn rm_trashes_after_checking_every_address() {
    let e = Env::signed_in();
    let f = e.fake();
    let a = f.add_file(ROOT_ID, "a.txt", b"a");
    let b = f.add_file(ROOT_ID, "b.txt", b"b");
    f.add_folder(ROOT_ID, "dir");
    // A missing one stops the whole removal before anything is trashed.
    let err = e.fails(4, &["rm", "--force", &at("a.txt"), &at("missing.txt")]);
    assert_eq!(err["code"], "not_found");
    assert!(!f.is_trashed(&a));
    assert_eq!(e.fails(2, &["rm", "--force", &at("dir")])["code"], "usage");
    f.clear_requests();
    assert_eq!(
        e.fails(6, &["rm", &at("a.txt")])["code"],
        "confirmation_required"
    );
    assert!(
        f.requests().is_empty(),
        "exit 6 sent nothing: {:?}",
        f.requests()
    );
    let gone = e.ok(&["rm", "--force", &at("a.txt"), &at("b.txt")]);
    assert_eq!(gone["schema"], "drive-trash/v1");
    assert_eq!(gone["trashed"], 2);
    assert!(f.is_trashed(&a) && f.is_trashed(&b));
}

#[test]
fn mkdir_makes_parents_and_keeps_what_is_there() {
    let e = Env::signed_in();
    let made = e.ok(&["mkdir", &at("x/y/z")]);
    assert_eq!(made["schema"], "drive-mkdir/v1");
    assert_eq!(made["created"], true);
    assert!(e.fake().find("x/y/z").is_some());
    assert_eq!(e.ok(&["mkdir", &at("x/y")])["created"], false);
    e.fake().add_file(ROOT_ID, "plain", b"p");
    assert_eq!(e.fails(2, &["mkdir", &at("plain")])["code"], "usage");
}

#[test]
fn md5sum_lists_files_below_or_one_file() {
    let e = Env::signed_in();
    let f = e.fake();
    let sub = f.add_folder(ROOT_ID, "sub");
    f.add_file(&sub, "b.txt", b"bb");
    f.add_file(ROOT_ID, "a.txt", b"aaa");
    f.add_google_doc(ROOT_ID, "notes");
    let all = e.ok(&["md5sum", &store()]);
    assert_eq!(all["schema"], "drive-hash-list/v1");
    assert_eq!(paths(&all), vec!["a.txt", "sub/b.txt"]);
    assert_eq!(paths(&e.ok(&["hashes", &at("sub")])), vec!["b.txt"]);
    let one = e.ok(&["md5sum", &at("a.txt")]);
    assert_eq!(one["items"][0]["md5"], md5_hex(b"aaa"));
    assert_eq!(one["items"][0]["size"], 3);
    let text = String::from_utf8(e.run(&["md5sum", &store()]).stdout).unwrap();
    assert_eq!(
        text,
        format!(
            "{}  a.txt\n{}  sub/b.txt\n",
            md5_hex(b"aaa"),
            md5_hex(b"bb")
        )
    );
    assert_eq!(
        e.fails(2, &["md5sum", &at("notes")])["reason"],
        "google_document"
    );
}

#[test]
fn duplicates_lists_every_shared_name_and_ls_names_the_command() {
    let e = Env::signed_in();
    let f = e.fake();
    let one = f.add_file(ROOT_ID, "index.json", b"one");
    let two = f.add_file(ROOT_ID, "index.json", b"two");
    let none = e.ok(&["duplicates", &at("")]);
    assert_eq!(none["schema"], "drive-duplicate-list/v1");
    let groups = none["items"].as_array().unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0]["name"], "index.json");
    assert_eq!(groups[0]["type"], "file");
    let mut ids = vec![one, two];
    ids.sort();
    assert_eq!(groups[0]["ids"], serde_json::json!(ids));
    let err = e.fails(1, &["ls", &store()]);
    assert_eq!(err["reason"], "duplicate_name");
    assert!(
        err["remedy"].as_str().unwrap().contains("gdrv duplicates"),
        "{err}"
    );
}
