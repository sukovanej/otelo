use std::path::Path;

const UI_DIST_DIR: &str = "../../packages/app/dist";

fn main() {
    println!("cargo::rerun-if-changed={UI_DIST_DIR}");
    if !Path::new(UI_DIST_DIR).join("index.html").exists() {
        println!(
            "cargo::warning=packages/app/dist is missing, so otelo serves no UI; run `mise run web:build`"
        );
    }
}
