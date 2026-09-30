use std::fs;
use std::path::Path;

#[test]
fn the_web_ui_has_the_spec_of_the_api() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/api/openapi.json");
    let written_spec = fs::read_to_string(&path).unwrap_or_default();
    let built_spec = format!(
        "{}\n",
        otelo_api::build_openapi_spec().to_pretty_json().unwrap()
    );
    assert!(
        written_spec == built_spec,
        "packages/api/openapi.json is not the spec of the API; run `mise run api:generate`"
    );
}
