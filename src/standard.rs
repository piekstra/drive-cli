//! The family's standard commands: config, self-update, completions, info.

use clap::CommandFactory;
use clap_complete::Shell;
use pk_cli_config::ConfigStore;
use pk_cli_core::info::{AuthInfo, CliInfo};
use pk_cli_core::{output, CliError};
use pk_cli_drive::google::ProfileStore;
use pk_cli_selfupdate::{SelfUpdateArgs, Updater};
use serde_json::json;

use crate::cli::{Cli, ConfigCommand};
use crate::context::{BIN, REPO, VERSION};

/// What `info` lists as gdrv's commands.
pub const CAPABILITIES: &[&str] = &[
    "login",
    "status",
    "logout",
    "about",
    "client",
    "folder",
    "profiles",
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
];

pub fn config(command: &ConfigCommand, json: bool) -> Result<(), CliError> {
    let store = ConfigStore::new(BIN);
    match command {
        ConfigCommand::Path => {
            let path = store.path()?;
            output::emit(
                json,
                "config-path",
                json!({"path": path.display().to_string()}),
                |_| println!("{}", path.display()),
            );
            Ok(())
        }
        ConfigCommand::Show => {
            let payload = json!({
                "config": store.path()?.display().to_string(),
                "profiles": ProfileStore::for_binary(BIN)?.path().display().to_string(),
                "vault": pk_cli_auth::vault::default_dir(BIN)?.display().to_string(),
                "keychain_service": format!("piekstra.{BIN}"),
                "settings": {},
            });
            output::emit(json, "gdrv-config", payload, |v| output::kv(v, 0));
            Ok(())
        }
        ConfigCommand::Set { key, .. } | ConfigCommand::Unset { key } => {
            Err(CliError::Usage(format!(
                "gdrv has no setting `{}`, nor any other: a Drive profile is made by `{BIN} \
                 login --profile <NAME>` and listed by `{BIN} profiles list`",
                key.escape_debug()
            )))
        }
    }
}

pub fn self_update(args: &SelfUpdateArgs, json: bool, quiet: bool) -> Result<(), CliError> {
    Updater {
        repo: REPO.into(),
        binary: BIN.into(),
        target: env!("BUILD_TARGET").into(),
        current: VERSION.into(),
    }
    .run(args, json, quiet)
}

pub fn completions(shell: Shell) {
    clap_complete::generate(shell, &mut Cli::command(), BIN, &mut std::io::stdout());
}

pub fn info() {
    let info = CliInfo::new(
        BIN,
        VERSION,
        &format!("https://github.com/{REPO}"),
        AuthInfo {
            required: true,
            method: "browser-session".into(),
            login_hint: Some(format!("{BIN} login --profile <NAME>")),
        },
        CAPABILITIES,
    );
    output::json(&serde_json::to_value(&info).expect("CliInfo serializes to JSON"));
}
