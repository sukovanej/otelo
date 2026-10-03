use std::path::PathBuf;

use anyhow::{Context, bail};
use otelo_state::StateFile;

use crate::serve::DEFAULT_DATA_DIR;

#[derive(clap::Args)]
pub struct InitArgs {
    /// Directory for the state and the telemetry, made when it is missing
    #[arg(long = "data", value_name = "DATA", default_value = DEFAULT_DATA_DIR)]
    data_dir: PathBuf,

    /// Replace the password, and end every session
    #[arg(long)]
    new_password: bool,
}

pub fn init_data_directory(args: &InitArgs) -> anyhow::Result<()> {
    std::fs::create_dir_all(&args.data_dir)
        .with_context(|| format!("make the data directory {}", args.data_dir.display()))?;
    let state = StateFile::open(&args.data_dir)?;
    if state.has_password()? && !args.new_password {
        bail!(
            "{} has a password already; `otelo init --new-password` replaces it and ends every session",
            args.data_dir.display()
        );
    }
    let password = state.replace_password()?;
    println!("{password}");
    eprintln!(
        "That is the password of the UI and of `otelo login`. otelo keeps only its hash, so it \
         prints the password this once."
    );
    Ok(())
}
