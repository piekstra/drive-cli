//! gdrv: Google Drive from the command line, for scripts and agents, through
//! Drive's own API (`pk-cli-drive`'s Google backend) with an OAuth client of
//! the owner's. `main.rs` parses and maps errors to exit codes; everything
//! else is here.

pub mod address;
pub mod base64;
pub mod cli;
pub mod context;
pub mod files;
pub mod standard;

use pk_cli_core::CliError;
use pk_cli_drive::google::DriveCommand;

use cli::{AuthCommand, Cli, Command, PermissionsCommand};
use context::drive_cli;

/// Run one parsed command.
pub fn run(cli: &Cli) -> Result<(), CliError> {
    let json = cli.common.json;
    let profile = cli.profile.as_deref();
    let drive =
        |command: &DriveCommand| drive_cli()?.run(command, profile, json, cli.common.interactive());
    match &cli.command {
        Command::Drive(command) => drive(command),
        Command::Auth(AuthCommand::Login(args)) => drive(&DriveCommand::Login(args.clone())),
        Command::Auth(AuthCommand::Status(args)) => drive(&DriveCommand::Status(args.clone())),
        Command::Auth(AuthCommand::Logout(args)) => drive(&DriveCommand::Logout(args.clone())),
        Command::Ls(args) => files::ls(&drive_cli()?, profile, args, json),
        Command::Stat(args) => files::stat(&drive_cli()?, profile, args, json),
        Command::Permissions(PermissionsCommand::List(args)) => {
            files::permissions_list(&drive_cli()?, profile, args, json)
        }
        Command::Cat(args) => files::cat(&drive_cli()?, profile, args, json),
        Command::Cp(args) => files::cp(&drive_cli()?, profile, args, json),
        Command::Mv(args) => files::mv(&drive_cli()?, profile, args, json),
        Command::Rm(args) => {
            files::rm(&drive_cli()?, profile, args, json, cli.common.interactive())
        }
        Command::Rmdir(args) => {
            files::rmdir(&drive_cli()?, profile, args, json, cli.common.interactive())
        }
        Command::Mkdir(args) => files::mkdir(&drive_cli()?, profile, args, json),
        Command::Md5sum(args) => files::md5sum(&drive_cli()?, profile, args, json),
        Command::Duplicates(args) => files::duplicates(&drive_cli()?, profile, args, json),
        Command::Config(command) => standard::config(command, json),
        Command::SelfUpdate(args) => standard::self_update(args, json, cli.common.quiet),
        Command::Completions { shell } => {
            standard::completions(*shell);
            Ok(())
        }
        Command::Info => {
            standard::info();
            Ok(())
        }
    }
}
