use std::fs;
use std::path::Path;

/// The web UI generates the types of the API from this file, so a change of
/// the Rust types has to reach it.
#[test]
fn the_web_ui_has_the_spec_of_the_api() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/api/openapi.json");
    let written = fs::read_to_string(&path).unwrap_or_default();
    let spec = format!("{}\n", siner_api::spec().to_pretty_json().unwrap());
    assert!(
        written == spec,
        "packages/api/openapi.json is not the spec of the API; run `mise run api:generate`"
    );
}
