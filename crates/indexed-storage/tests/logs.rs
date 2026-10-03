use otelo_indexed_storage::LogSource;

#[test]
fn a_log_source_is_stored_and_sent_under_its_name() {
    assert_eq!(LogSource::Otlp.name(), "otlp");
    assert_eq!(LogSource::from_name("otlp"), Some(LogSource::Otlp));
    assert_eq!(serde_json::to_value(LogSource::Otlp).unwrap(), "otlp");
    assert_eq!(LogSource::from_name("journald"), None);
}
