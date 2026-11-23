//! What every command runs with: the Drive context, and a store opened from
//! an address.

use std::io::IsTerminal;
use std::time::Duration;

use pk_cli_core::CliError;
use pk_cli_drive::google::{Drive, DriveCli, DriveSpec, Listing, Scope};

pub const BIN: &str = "gdrv";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPO: &str = "piekstra/drive-cli";

/// How long a run with no terminal on stdin (a script, an agent) waits for
/// the keychain to answer whether gdrv may read its vault's key, unless
/// `GDRV_KEYCHAIN_WAIT` says otherwise.
pub const UNATTENDED_KEY_WAIT: Duration = Duration::from_secs(60);

/// The Drive context: gdrv's vault (one keychain item, `piekstra.gdrv`),
/// its profiles in `drive.json`, and full `drive` scope unless `login
/// --scope` asks for another.
///
/// On macOS each newly built or upgraded binary asks once, in a dialog,
/// before it may read the vault's key. At a terminal the read waits for the
/// answer. Without one, nobody may be there to answer, so the read waits at
/// most [`key_wait`] and then fails with exit 3 naming the dialog; the
/// dialog stays open, and an "Always Allow" lets the next run through.
pub fn drive_cli() -> Result<DriveCli, CliError> {
    let wait = key_wait(
        std::env::var("GDRV_KEYCHAIN_WAIT").ok().as_deref(),
        std::io::stdin().is_terminal(),
    )?;
    let vault = DriveCli::default_vault(BIN)?;
    let vault = match wait {
        Some(wait) => vault.bound_key_reads(wait),
        None => vault,
    };
    let cli = DriveCli::new(BIN, VERSION, BIN)?
        .default_scopes(&[Scope::Full])
        .with_vault(vault);
    test_keys::apply(cli)
}

/// The bound on a keychain read of the vault's key: `GDRV_KEYCHAIN_WAIT`
/// seconds when set, otherwise none at a terminal and
/// [`UNATTENDED_KEY_WAIT`] without one.
pub fn key_wait(setting: Option<&str>, terminal: bool) -> Result<Option<Duration>, CliError> {
    match setting.map(str::trim).filter(|s| !s.is_empty()) {
        Some(text) => match text.parse::<u64>() {
            Ok(secs) if secs > 0 => Ok(Some(Duration::from_secs(secs))),
            _ => Err(CliError::Usage(format!(
                "GDRV_KEYCHAIN_WAIT is `{}`; it takes a whole number of seconds above 0",
                text.escape_debug()
            ))),
        },
        None if terminal => Ok(None),
        None => Ok(Some(UNATTENDED_KEY_WAIT)),
    }
}

/// A store at the folder `store` names, listing as `listing` says.
pub fn open(cli: &DriveCli, store: &DriveSpec, listing: Listing) -> Result<Drive, CliError> {
    Ok(cli.drive(store)?.temp_prefix(BIN).listing(listing))
}

/// The vault's key in a file, for the black-box tests: `GDRV_TEST_KEY_FILE`
/// names it. Debug builds only, which is what `cargo test` runs; a release
/// build (`cargo install`, a release asset) has no such code, so no variable
/// can move a shipped binary's key out of the keychain.
#[cfg(debug_assertions)]
mod test_keys {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use pk_cli_auth::vault::{self, KeyStore, Vault};
    use pk_cli_core::private_file::{self, Replace};
    use pk_cli_core::CliError;
    use pk_cli_drive::google::DriveCli;
    use pk_cli_secrets::Secret;

    pub fn apply(cli: DriveCli) -> Result<DriveCli, CliError> {
        match std::env::var_os("GDRV_TEST_KEY_FILE").filter(|v| !v.is_empty()) {
            Some(path) => Ok(cli.with_vault(Vault::new(
                FileKeys(path.into()),
                vault::default_dir(super::BIN)?,
            ))),
            None => Ok(cli),
        }
    }

    struct FileKeys(PathBuf);

    impl FileKeys {
        fn load(&self) -> Result<BTreeMap<String, String>, CliError> {
            match std::fs::read(&self.0) {
                Ok(bytes) => serde_json::from_slice(&bytes)
                    .map_err(|e| CliError::Keychain(format!("the test key file: {e}"))),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
                Err(e) => Err(CliError::Keychain(format!("the test key file: {e}"))),
            }
        }

        fn save(&self, items: &BTreeMap<String, String>) -> Result<(), CliError> {
            let bytes = serde_json::to_vec(items).expect("a map of strings serializes");
            private_file::write(&self.0, &bytes, Replace::Always)
                .map_err(|e| CliError::Keychain(format!("the test key file: {e}")))
        }
    }

    impl KeyStore for FileKeys {
        fn get(&self, account: &str) -> Result<Option<Secret>, CliError> {
            Ok(self.load()?.remove(account).map(Secret::new))
        }

        fn set(&self, account: &str, value: &Secret) -> Result<(), CliError> {
            let mut items = self.load()?;
            items.insert(account.to_string(), value.expose().to_string());
            self.save(&items)
        }

        fn delete(&self, account: &str) -> Result<bool, CliError> {
            let mut items = self.load()?;
            let had = items.remove(account).is_some();
            self.save(&items)?;
            Ok(had)
        }
    }
}

#[cfg(not(debug_assertions))]
mod test_keys {
    use pk_cli_core::CliError;
    use pk_cli_drive::google::DriveCli;

    pub fn apply(cli: DriveCli) -> Result<DriveCli, CliError> {
        Ok(cli)
    }
}

#[cfg(test)]
mod tests {
    use super::{key_wait, UNATTENDED_KEY_WAIT};
    use std::time::Duration;

    #[test]
    fn a_keychain_read_is_bounded_only_without_a_terminal_unless_set() {
        assert_eq!(key_wait(None, true).unwrap(), None);
        assert_eq!(key_wait(None, false).unwrap(), Some(UNATTENDED_KEY_WAIT));
        assert_eq!(
            key_wait(Some(""), false).unwrap(),
            Some(UNATTENDED_KEY_WAIT)
        );
        assert_eq!(
            key_wait(Some("5"), true).unwrap(),
            Some(Duration::from_secs(5))
        );
        for bad in ["0", "-1", "1.5", "soon"] {
            assert_eq!(
                key_wait(Some(bad), false).unwrap_err().exit_code(),
                2,
                "{bad}"
            );
        }
    }
}
