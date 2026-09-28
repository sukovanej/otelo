//! The daemon and the CLI of siner. `main.rs` reads the command line and
//! calls these modules. The HTTP API is `siner-api`, and the state file
//! `siner-state`.

pub mod cli;
pub mod own;
pub mod serve;
pub mod ui;
