//! The command surface through the binary, offline: every command's help,
//! usage errors, the `--json` error document, `info`, completions, and what
//! runs before any keychain read or request. No Drive server is running, so
//! a test that reached the network would fail.

mod support;

use support::{one_document, Env};

/// Every command and subcommand, as `gdrv <these> --help` takes them.
const COMMANDS: &[&[&str]] = &[
    &[],
    &["login"],
    &["status"],
    &["logout"],
    &["about"],
    &["client"],
    &["client", "import"],
    &["profiles"],
    &["profiles", "list"],
    &["profiles", "remove"],
    &["folder"],
    &["folder", "create"],
    &["folder", "list"],
    &["auth"],
    &["auth", "login"],
    &["auth", "status"],
    &["auth", "logout"],
    &["ls"],
    &["stat"],
    &["permissions"],
    &["permissions", "list"],
    &["cat"],
    &["cp"],
    &["mv"],
    &["rm"],
    &["rmdir"],
    &["mkdir"],
    &["md5sum"],
    &["hashes"],
    &["duplicates"],
    &["config"],
    &["config", "path"],
    &["config", "show"],
    &["config", "set"],
    &["config", "unset"],
    &["self-update"],
    &["completions"],
    &["info"],
];

#[test]
fn every_commands_help_renders() {
    let e = Env::offline();
    for c in COMMANDS {
        let mut args = c.to_vec();
        args.push("--help");
        let out = e.run(&args);
        assert!(
            out.status.success(),
            "gdrv {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(!out.stdout.is_empty(), "gdrv {args:?} printed no help");
    }
}

#[test]
fn file_commands_help_names_the_listing_and_the_address() {
    let e = Env::offline();
    for c in [
        "ls",
        "cat",
        "cp",
        "mv",
        "rm",
        "rmdir",
        "md5sum",
        "duplicates",
    ] {
        let help = String::from_utf8(e.run(&[c, "--help"]).stdout).unwrap();
        assert!(help.contains("listing"), "{c}: {help}");
        assert!(help.contains("gdrive:<profile>/<folder id>"), "{c}: {help}");
    }
    let ls = String::from_utf8(e.run(&["ls", "--help"]).stdout).unwrap();
    assert!(ls.contains("[default: whole]"), "{ls}");
    assert!(ls.contains("`root` is My Drive"), "{ls}");
}

#[test]
fn version_info_and_completions() {
    let e = Env::offline();
    let out = e.run(&["--version"]);
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("gdrv "));
    let info = e.ok(&["info"]);
    assert_eq!(info["schema"], "cli-info/v1");
    assert_eq!(info["name"], "gdrv");
    assert_eq!(info["spec"], "piekstra-cli/1");
    assert_eq!(info["auth"]["method"], "browser-session");
    let caps: Vec<&str> = info["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    for c in [
        "ls",
        "stat",
        "permissions",
        "cat",
        "cp",
        "mv",
        "rm",
        "rmdir",
        "mkdir",
        "md5sum",
        "duplicates",
    ] {
        assert!(caps.contains(&c), "{c}");
    }
    // Text mode prints the same document.
    assert_eq!(one_document(&e.run(&["info"]))["name"], "gdrv");
    for shell in ["bash", "zsh", "fish"] {
        let out = e.run(&["completions", shell]);
        assert!(out.status.success() && !out.stdout.is_empty(), "{shell}");
    }
}

#[test]
fn no_color_in_the_environment_is_not_a_usage_error() {
    let e = Env::offline();
    let out = e.cmd().env("NO_COLOR", "1").arg("info").output().unwrap();
    assert!(out.status.success());
}

#[test]
fn a_bad_address_is_exit_2_with_one_json_error_document() {
    let e = Env::offline();
    for args in [
        &["ls", "gdrive:home"][..],
        &["ls", "gdrive:Home/root"],
        &["ls", "gdrive:home/ro'ot"],
        &["ls", "gdrive:home/root/a/../b"],
        &["ls", "./local-dir"],
        &["cat", "gdrive:home/root"],
        &["mkdir", "gdrive:home/root"],
        &["cp", "a.txt", "b.txt"],
        &["cp", "gdrive:home/root/a", "gdrive:home/root/b"],
        &["mv", "gdrive:home/one/a.txt", "gdrive:home/two/a.txt"],
        &["mv", "gdrive:home/root/a.txt", "./a.txt"],
        &["rm", "gdrive:home/root"],
        &["rm", "./a.txt"],
        &["rmdir", "gdrive:home/root"],
        &["rmdir", "./a-dir"],
    ] {
        let err = e.fails(2, args);
        assert_eq!(err["code"], "usage", "{args:?}: {err}");
        assert!(err["message"].as_str().is_some_and(|m| !m.is_empty()));
    }
}

#[test]
fn an_argument_clap_refuses_is_exit_2() {
    let e = Env::offline();
    for args in [
        &["ls", "--listing", "some", "gdrive:home/root"][..],
        &["rm"],
        &["cp", "only-one"],
        &["cp", "--transfers", "0", "a", "gdrive:home/root/a"],
        &["cp", "--transfers", "9", "a", "gdrive:home/root/a"],
        &["cp", "--transfers", "many", "a", "gdrive:home/root/a"],
    ] {
        assert_eq!(e.run(args).status.code(), Some(2), "{args:?}");
    }
}

#[test]
fn a_profile_flag_that_is_not_the_addresss_is_refused() {
    let e = Env::offline();
    let err = e.fails(2, &["--profile", "work", "ls", "gdrive:home/root"]);
    assert!(err["message"].as_str().unwrap().contains("work"), "{err}");
}

#[test]
fn rm_without_force_and_without_a_terminal_is_exit_6_before_anything_is_read() {
    let e = Env::offline();
    // No profile is configured: had rm read anything first, it would be
    // exit 2 naming the sign-in.
    let err = e.fails(6, &["rm", "gdrive:home/root/a.txt"]);
    assert_eq!(err["code"], "confirmation_required", "{err}");
    let out = e.run(&["rm", "gdrive:home/root/a.txt"]);
    assert_eq!(out.status.code(), Some(6));
    let err = e.fails(6, &["rmdir", "gdrive:home/root/empty"]);
    assert_eq!(err["code"], "confirmation_required", "{err}");
}

#[test]
fn a_store_on_a_profile_not_signed_in_names_the_login() {
    let e = Env::offline();
    let err = e.fails(2, &["ls", "gdrive:home/root"]);
    assert!(
        err["remedy"]
            .as_str()
            .unwrap()
            .contains("gdrv login --profile home"),
        "{err}"
    );
}

#[test]
fn logged_out_status_and_profiles_read_no_keychain() {
    let e = Env::offline();
    let profiles = e.ok(&["profiles", "list"]);
    assert_eq!(profiles["schema"], "profile-list/v1");
    let err = e.fails(2, &["status"]);
    assert!(err["message"].as_str().is_some(), "{err}");
    let err = e.fails(2, &["auth", "status"]);
    assert!(err["message"].as_str().is_some(), "{err}");
}

#[test]
fn config_names_where_gdrv_keeps_things_and_has_no_settings() {
    let e = Env::offline();
    let path = e.ok(&["config", "path"]);
    assert!(path["path"]
        .as_str()
        .unwrap()
        .starts_with(e.root.to_str().unwrap()));
    let show = e.ok(&["config", "show"]);
    assert_eq!(show["schema"], "gdrv-config/v1");
    assert!(show["profiles"].as_str().unwrap().ends_with("drive.json"));
    assert!(show["vault"].as_str().unwrap().ends_with("vault"));
    assert_eq!(show["keychain_service"], "piekstra.gdrv");
    assert_eq!(
        e.fails(2, &["config", "set", "listing", "bytes"])["code"],
        "usage"
    );
    assert_eq!(e.fails(2, &["config", "unset", "listing"])["code"], "usage");
}

#[test]
fn a_keychain_wait_that_is_not_seconds_is_exit_2_before_any_read() {
    let e = Env::offline();
    let out = e
        .cmd()
        .env("GDRV_KEYCHAIN_WAIT", "soon")
        .args(["--json", "ls", "gdrive:home/root"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = one_document(&out)["error"].clone();
    assert!(
        err["message"]
            .as_str()
            .unwrap()
            .contains("GDRV_KEYCHAIN_WAIT"),
        "{err}"
    );
}

#[test]
fn stat_and_permissions_take_one_drive_address_and_stay_read_only() {
    let e = Env::offline();
    // Local paths, missing addresses and unknown subcommands are exit 2,
    // before any keychain read or request.
    for args in [
        &["stat"][..],
        &["stat", "./local"],
        &["permissions"],
        &["permissions", "list"],
        &["permissions", "list", "./local"],
        &["permissions", "add", "gdrive:home/1Synthetic"],
    ] {
        let mut a = vec!["--json"];
        a.extend_from_slice(args);
        let out = e.run(&a);
        assert_eq!(out.status.code(), Some(2), "gdrv {args:?}");
    }
    // `permissions` only lists.
    let help = String::from_utf8(e.run(&["permissions", "--help"]).stdout).unwrap();
    assert!(help.contains("list"), "{help}");
    for word in ["add", "remove", "share", "unshare"] {
        assert!(!help.contains(&format!("  {word}")), "{word}: {help}");
    }
    let list = String::from_utf8(e.run(&["permissions", "list", "--help"]).stdout).unwrap();
    assert!(
        list.contains("Nothing is shared or unshared here"),
        "{list}"
    );
}
