//! Rebuilds when the UI build changes, and warns when there is none, since
//! `src/ui.rs` embeds `packages/app/dist`.

use std::path::Path;

const DIST: &str = "../../packages/app/dist";

fn main() {
    println!("cargo::rerun-if-changed={DIST}");
    if !Path::new(DIST).join("index.html").exists() {
        println!(
            "cargo::warning=packages/app/dist is missing, so siner serves no UI; run `mise run web:build`"
        );
    }
}
