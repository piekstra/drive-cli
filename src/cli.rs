//! The clap tree.

use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use pk_cli_core::CommonArgs;
use pk_cli_drive::google::{DriveCommand, Listing, LoginArgs, LogoutArgs, StatusArgs};
use pk_cli_selfupdate::SelfUpdateArgs;

const ADDRESS_HELP: &str = "Drive is addressed as gdrive:<profile>/<folder id>[/<path>]: a Drive \
profile (one Google account, made by `gdrv login --profile <NAME>`), the id of a folder (the part of \
its URL after /folders/; `root` is My Drive), and a path under it. Anything else is a local path; \
write ./gdrive:x for a local file of that name.";

/// Google Drive from the command line, for scripts and agents, through
/// Drive's own API with an OAuth client of the owner's (piekstra-cli/1).
#[derive(Parser, Debug)]
#[command(name = "gdrv", version, about, after_help = ADDRESS_HELP)]
pub struct Cli {
    #[command(flatten)]
    pub common: CommonArgs,

    /// The Drive profile that login, status, logout, about, client, folder
    /// and profiles act on: one Google account (env GDRV_DRIVE_PROFILE).
    /// There is no default. File commands take the profile from their
    /// address; one given here must match it.
    #[arg(long, value_name = "NAME", global = true)]
    pub profile: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    #[command(flatten)]
    Drive(DriveCommand),

    /// The family's spelling of login, status and logout (SPEC v1 §1.2).
    #[command(subcommand)]
    Auth(AuthCommand),

    /// List a folder, a folder's whole tree (-R), or one file
    /// (drive-entry-list/v1).
    ///
    /// One level lists folders and files; -R lists every file below by its
    /// path, folders only in those paths, as `rclone lsjson -R --files-only`.
    /// Each item has its Drive id, which addresses a folder directly
    /// (gdrive:<profile>/<id>), the id of the folder it is in (`parent`), and
    /// each file its size, modified time and md5. Items that share a name are each listed, though a path cannot
    /// name one of them. The default listing is `whole`: Google's own
    /// documents (Docs, Sheets) are listed with type `document` and no md5,
    /// and shortcuts are followed, as rclone lists.
    #[command(visible_alias = "list", after_help = ADDRESS_HELP)]
    Ls(LsArgs),

    /// One item: its id, its parent folder, its size and md5, and whether
    /// this account may share it (drive-item/v1).
    ///
    /// The address is a folder id, a path under one, or the id of any file
    /// or folder by itself (gdrive:<profile>/<id>). `parent` is the Drive id
    /// of the folder that holds the item, and is left out where Drive names
    /// none this sign-in can see, as for My Drive itself. An item shared with
    /// the account may have none. Shortcuts are followed (the whole listing).
    /// Read only.
    #[command(after_help = ADDRESS_HELP)]
    Stat(StatArgs),

    /// Who may open an item in Drive. Read only.
    #[command(subcommand)]
    Permissions(PermissionsCommand),

    /// Print one file's bytes on stdout (--json: drive-file/v1).
    ///
    /// The download is checked against Drive's md5 before a byte is printed.
    /// Shortcuts are followed (the whole listing). One of Google's own
    /// documents has no bytes to print and is exit 2.
    #[command(after_help = ADDRESS_HELP)]
    Cat(CatArgs),

    /// Copy files and trees between this machine and Drive (drive-copy/v1).
    ///
    /// A file goes to the path named, or into a folder named with a trailing
    /// `/` or already there. A folder's contents go into the destination
    /// folder, made as needed, as `rclone copy` does. Every file's md5 is
    /// compared: an unchanged one is skipped, and one already there with
    /// other bytes refuses the whole copy, before anything is written,
    /// unless --force replaces it (in Drive in place, keeping the file's id
    /// and revisions). Downloads are written owner-only and checked against
    /// Drive's md5; Google's own documents have no bytes and are skipped.
    /// Uses the whole listing, so nothing is written onto or through a
    /// shortcut (exit 2) and nothing lands beside one. Copies within Drive
    /// are not supported.
    #[command(after_help = ADDRESS_HELP)]
    Cp(CpArgs),

    /// Move or rename a file within one Drive folder's tree
    /// (drive-move/v1).
    ///
    /// Both addresses name the same profile and folder id. The destination
    /// is a path, or a folder named with a trailing `/` or already there. A
    /// file already at the destination refuses the move unless --force,
    /// which moves the old one to the trash. Folders are not moved. Uses the
    /// whole listing: nothing moves through a shortcut.
    #[command(after_help = ADDRESS_HELP)]
    Mv(MvArgs),

    /// Move files to the Google Drive trash, recoverable there for 30 days
    /// (drive-trash/v1).
    ///
    /// Asks first; without a terminal to ask on (or with --json) it needs
    /// --force and is otherwise exit 6, before anything is read. Every
    /// address is checked before anything is trashed. Folders are not
    /// removed: `gdrv rmdir` trashes an empty one. Uses the whole listing:
    /// nothing is trashed through a shortcut.
    #[command(visible_alias = "trash", after_help = ADDRESS_HELP)]
    Rm(RmArgs),

    /// Move empty folders to the Google Drive trash, recoverable there for
    /// 30 days (drive-rmdir/v1).
    ///
    /// Only an empty folder is trashed: one holding anything outside the
    /// trash is refused (exit 2, reason `not_empty`), as `rclone rmdir`
    /// refuses. Asks first; without a terminal to ask on (or with --json) it
    /// needs --force and is otherwise exit 6, before anything is read. Every
    /// address is checked before anything is trashed, so trash a folder's
    /// empty subfolders first, in a run of their own. The address's own
    /// folder id is never trashed. Uses the whole listing: neither a
    /// shortcut nor what it points at is trashed.
    #[command(after_help = ADDRESS_HELP)]
    Rmdir(RmdirArgs),

    /// Make a folder and any missing parents under a folder id
    /// (drive-mkdir/v1). One already there is kept.
    #[command(after_help = ADDRESS_HELP)]
    Mkdir(MkdirArgs),

    /// The md5 Drive holds for a file, or for every file below a folder, as
    /// `md5sum` prints (drive-hash-list/v1).
    ///
    /// Paths are relative to the folder named, and each file has its Drive
    /// id; files that share a name are each listed. Google's own documents
    /// have no md5 and are left out. The default listing is `whole` (shortcuts
    /// followed).
    #[command(alias = "hashes", after_help = ADDRESS_HELP)]
    Md5sum(HashArgs),

    /// Every name that two or more items share in one folder, anywhere
    /// below a folder (drive-duplicate-list/v1).
    ///
    /// Drive allows two items of one name; gdrv never guesses between them,
    /// so a path through such a name is an error naming both. This lists
    /// them all, with their ids, in one walk. `dir` is the folder holding
    /// them, relative to the address's folder id. The default listing is
    /// `whole`: shortcuts count as items, as they do in `ls`.
    #[command(after_help = ADDRESS_HELP)]
    Duplicates(HashArgs),

    /// Where gdrv keeps its files. It has no settings of its own: profiles
    /// are made by `login` and listed by `profiles list`.
    #[command(subcommand)]
    Config(ConfigCommand),

    /// Update to the latest release from GitHub.
    SelfUpdate(SelfUpdateArgs),

    /// Print a shell completion script.
    Completions { shell: Shell },

    /// Machine-readable capability discovery (cli-info/v1).
    Info,
}

#[derive(Subcommand, Debug)]
pub enum AuthCommand {
    /// The same as `gdrv login`.
    Login(LoginArgs),
    /// The same as `gdrv status` (auth-status/v1).
    Status(StatusArgs),
    /// The same as `gdrv logout`.
    Logout(LogoutArgs),
}

#[derive(Subcommand, Debug)]
pub enum PermissionsCommand {
    /// Everyone who may open an item, and where each grant comes from
    /// (drive-permission-list/v1).
    ///
    /// The address is a folder id, a path under one, or the id of any file
    /// or folder by itself (gdrive:<profile>/<id>). Each grant has its type
    /// (user, group, domain or anyone), its role, the email or domain, and
    /// its source: `direct` (given on the item), `inherited` (it reaches the
    /// item from a folder above, named by `inherited_from`), or `unknown`
    /// when the folder above is out of this sign-in's sight. `link_sharing`
    /// lists the grants of type `anyone` and `domain`. `can_share` says
    /// whether this account may change them; Drive may not list grants to
    /// an account that may not (exit 3, reason `permissions_hidden`).
    /// Nothing is shared or unshared here.
    #[command(after_help = ADDRESS_HELP)]
    List(PermissionsListArgs),
}

/// What a listing shows and whether paths follow shortcuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ListingArg {
    /// Everything, as rclone lists: Google's own documents with no md5, and
    /// shortcuts as their targets (a shortcut to a folder is walked). A
    /// shortcut whose target is gone stops the listing.
    Whole,
    /// Only files with bytes of their own: no Google documents, no
    /// shortcuts.
    Bytes,
}

impl ListingArg {
    pub fn listing(self) -> Listing {
        match self {
            ListingArg::Whole => Listing::Whole,
            ListingArg::Bytes => Listing::Bytes,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ListingArg::Whole => "whole",
            ListingArg::Bytes => "bytes",
        }
    }
}

#[derive(Args, Debug)]
pub struct LsArgs {
    /// gdrive:<profile>/<folder id>[/<path>]
    #[arg(value_name = "ADDRESS")]
    pub address: String,
    /// Every file below the folder, by its path.
    #[arg(short = 'R', long, visible_short_alias = 'r')]
    pub recursive: bool,
    /// What the listing shows.
    #[arg(long, value_enum, value_name = "LISTING", default_value_t = ListingArg::Whole)]
    pub listing: ListingArg,
}

#[derive(Args, Debug)]
pub struct StatArgs {
    /// gdrive:<profile>/<id>[/<path>]
    #[arg(value_name = "ADDRESS")]
    pub address: String,
}

#[derive(Args, Debug)]
pub struct PermissionsListArgs {
    /// gdrive:<profile>/<id>[/<path>]
    #[arg(value_name = "ADDRESS")]
    pub address: String,
}

#[derive(Args, Debug)]
pub struct CatArgs {
    /// gdrive:<profile>/<folder id>/<path of a file>
    #[arg(value_name = "ADDRESS")]
    pub address: String,
}

#[derive(Args, Debug)]
pub struct CpArgs {
    /// A local file or directory, or a Drive address.
    #[arg(value_name = "SOURCE")]
    pub source: String,
    /// A Drive address, or a local path.
    #[arg(value_name = "DESTINATION")]
    pub destination: String,
    /// Replace files already there that hold other bytes.
    #[arg(long)]
    pub force: bool,
    /// For a tree, at most N files in flight at once, 1 to 8. Downloads run
    /// 8 at a time unless set, uploads 4: Drive takes about three writes a
    /// second per user, so uploads never go past 4.
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u8).range(1..=8))]
    pub transfers: Option<u8>,
}

#[derive(Args, Debug)]
pub struct MvArgs {
    /// gdrive:<profile>/<folder id>/<path of a file>
    #[arg(value_name = "SOURCE")]
    pub source: String,
    /// gdrive:<profile>/<same folder id>/<path, or folder/>
    #[arg(value_name = "DESTINATION")]
    pub destination: String,
    /// Replace a file already at the destination (it goes to the trash).
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct RmArgs {
    /// gdrive:<profile>/<folder id>/<path of a file>, one or more.
    #[arg(value_name = "ADDRESS", required = true, num_args = 1..)]
    pub addresses: Vec<String>,
    /// Do not ask (required when there is no terminal to ask on).
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct RmdirArgs {
    /// gdrive:<profile>/<folder id>/<path of an empty folder>, one or more.
    #[arg(value_name = "ADDRESS", required = true, num_args = 1..)]
    pub addresses: Vec<String>,
    /// Do not ask (required when there is no terminal to ask on).
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct MkdirArgs {
    /// gdrive:<profile>/<folder id>/<path of the folder>
    #[arg(value_name = "ADDRESS")]
    pub address: String,
}

#[derive(Args, Debug)]
pub struct HashArgs {
    /// gdrive:<profile>/<folder id>[/<path>]
    #[arg(value_name = "ADDRESS")]
    pub address: String,
    /// What the listing shows.
    #[arg(long, value_enum, value_name = "LISTING", default_value_t = ListingArg::Whole)]
    pub listing: ListingArg,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    /// Print the config file's path.
    Path,
    /// Where gdrv keeps its config, its Drive profiles and its vault
    /// (gdrv-config/v1).
    Show,
    /// gdrv has no settings: exit 2 naming where profiles are made.
    Set { key: String, value: String },
    /// gdrv has no settings: exit 2 naming where profiles are made.
    Unset { key: String },
}
