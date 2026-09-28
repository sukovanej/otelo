//! The daemon and the CLI of siner. `main.rs` reads the command line and
//! calls these modules.

pub mod api;
pub mod client;
pub mod own;
pub mod query;
pub mod serve;
pub mod state;
pub mod table;
pub mod ui;
