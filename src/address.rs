//! What an argument names: a place in Drive or a local path.

use std::path::PathBuf;

use pk_cli_core::CliError;
use pk_cli_drive::google::DriveSpec;

/// A place in Drive: the store (a profile and a folder id) and a path under
/// it. `gdrive:<profile>/<folder id>[/<path>]`, parsed by [`DriveSpec`], so
/// a path holds no empty, `.` or `..` part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveAddress {
    /// The folder id's store, with no path: paths are `rel`.
    pub store: DriveSpec,
    /// `/`-separated, no leading or trailing `/`; empty for the folder.
    pub rel: String,
    /// The text ended in `/`: a folder to copy or move into.
    pub into: bool,
}

impl DriveAddress {
    pub fn parse(text: &str) -> Result<Self, CliError> {
        let spec = DriveSpec::parse(text)?;
        let store = DriveSpec::new(spec.profile().clone(), spec.folder())?;
        Ok(DriveAddress {
            store,
            rel: spec.path().to_string(),
            into: text.ends_with('/') && !spec.path().is_empty(),
        })
    }

    /// The last part of the path; empty for the folder itself.
    pub fn name(&self) -> &str {
        self.rel.rsplit('/').next().unwrap_or("")
    }
}

/// A command's argument: Drive, or this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    Drive(DriveAddress),
    Local { path: PathBuf, into: bool },
}

impl Location {
    pub fn parse(text: &str) -> Result<Self, CliError> {
        if DriveSpec::is_spec(text) {
            return DriveAddress::parse(text).map(Location::Drive);
        }
        if text.is_empty() {
            return Err(CliError::Usage("an empty path names nothing".into()));
        }
        Ok(Location::Local {
            path: PathBuf::from(text),
            into: text.ends_with('/') || text.ends_with(std::path::MAIN_SEPARATOR),
        })
    }
}

/// `parent/name`, or `name` under the root.
pub fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}

/// The folder part and the last part of a store path.
pub fn split(rel: &str) -> (&str, &str) {
    rel.rsplit_once('/').unwrap_or(("", rel))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_splits_into_its_store_and_a_path() {
        let a = DriveAddress::parse("gdrive:home/1Synthetic_folder/inbox/a.pdf").unwrap();
        assert_eq!(a.store.to_string(), "gdrive:home/1Synthetic_folder");
        assert_eq!(a.rel, "inbox/a.pdf");
        assert_eq!(a.name(), "a.pdf");
        assert!(!a.into);
        let root = DriveAddress::parse("gdrive:home/root").unwrap();
        assert_eq!(root.rel, "");
        assert_eq!(root.store.folder(), "root");
        assert!(
            DriveAddress::parse("gdrive:home/root/scratch/")
                .unwrap()
                .into
        );
        assert!(!DriveAddress::parse("gdrive:home/root/").unwrap().into);
        for bad in [
            "gdrive:home",
            "gdrive:home/",
            "gdrive:Home/root",
            "gdrive:home/root/a/../b",
            "gdrive:home/root/a//b",
        ] {
            assert_eq!(
                DriveAddress::parse(bad).unwrap_err().exit_code(),
                2,
                "{bad}"
            );
        }
    }

    #[test]
    fn anything_but_a_drive_address_is_local() {
        assert!(matches!(
            Location::parse("gdrive:home/root").unwrap(),
            Location::Drive(_)
        ));
        assert_eq!(
            Location::parse("./gdrive:x").unwrap(),
            Location::Local {
                path: "./gdrive:x".into(),
                into: false
            }
        );
        assert!(matches!(
            Location::parse("out/").unwrap(),
            Location::Local { into: true, .. }
        ));
        assert_eq!(Location::parse("").unwrap_err().exit_code(), 2);
    }

    #[test]
    fn paths_join_and_split() {
        assert_eq!(join("", "a"), "a");
        assert_eq!(join("a/b", "c"), "a/b/c");
        assert_eq!(split("a/b/c"), ("a/b", "c"));
        assert_eq!(split("c"), ("", "c"));
    }
}
