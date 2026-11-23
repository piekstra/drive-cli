use clap::Parser;
use drive_cli::cli::Cli;

fn main() {
    // Rust ignores SIGPIPE, so a closed pipe (`gdrv ls … | head`) would
    // panic on the next print. The default ends the process quietly, as
    // other Unix tools do.
    #[cfg(unix)]
    // SAFETY: called first, on the one thread, before any I/O.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    let cli = Cli::parse();
    if let Err(e) = drive_cli::run(&cli) {
        std::process::exit(pk_cli_core::output::fail(&e, cli.common.json));
    }
}
