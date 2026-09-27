mod serve;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "Deploy, run, and observe the apps on one server")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the daemon
    Serve(serve::Args),
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Serve(args) => serve::main(args),
    }
}
