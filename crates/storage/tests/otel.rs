use otelo_storage::{Severity, SpanId, SpanKind, SpanStatus, TraceContext, TraceId};

#[test]
fn parses_trace_ids_and_severities() {
    let id = TraceId::parse_hex("4bf92f3577b34da6a3ce929d0e0e4736").unwrap();
    assert_eq!(id.0[..3], [0x4b, 0xf9, 0x2f]);
    assert_eq!(id.0[15], 0x36);
    assert!(TraceId::parse_hex("4bf9").is_err());
    assert!(TraceId::parse_hex("zzf92f3577b34da6a3ce929d0e0e4736").is_err());
    assert_eq!(
        Severity::parse_level_or_number("WARN").unwrap(),
        Severity::WARN
    );
    assert_eq!(Severity::parse_level_or_number("17").unwrap().number(), 17);
    assert!(Severity::parse_level_or_number("loud").is_err());
    assert_eq!(Severity::from_number(18).level(), "ERROR");
}

#[test]
fn a_level_covers_four_severity_numbers() {
    assert_eq!(Severity::WARN.level_number_range(), 13..=16);
    assert_eq!(Severity::from_number(18).level_number_range(), 17..=20);
    assert_eq!(Severity::FATAL.level_number_range(), 21..=24);
    assert_eq!(Severity::UNSPECIFIED.level_number_range(), 0..=0);
}

#[test]
fn span_kinds_and_statuses_are_found_by_name() {
    assert_eq!(SpanKind::from_name("server"), Some(SpanKind::Server));
    assert_eq!(
        SpanKind::from_name("consumer").map(SpanKind::number),
        Some(5)
    );
    assert_eq!(SpanKind::from_name("Server"), None);
    assert_eq!(SpanStatus::from_name("error"), Some(SpanStatus::Error));
    assert_eq!(
        SpanStatus::from_name("unset").map(SpanStatus::number),
        Some(0)
    );
    assert_eq!(SpanStatus::from_name("failed"), None);
}

#[test]
fn a_span_id_without_a_trace_id_is_no_trace_context() {
    let trace_id = TraceId([0xab; 16]);
    let span_id = SpanId([0xcd; 8]);
    assert_eq!(
        TraceContext::from_otlp_bytes(&trace_id.0, &span_id.0),
        TraceContext::Span { trace_id, span_id }
    );
    assert_eq!(
        TraceContext::from_otlp_bytes(&trace_id.0, &[]),
        TraceContext::Trace(trace_id)
    );
    assert_eq!(
        TraceContext::from_otlp_bytes(&[0; 16], &span_id.0),
        TraceContext::None
    );
    assert_eq!(TraceContext::Trace(trace_id).span_id(), None);
    assert_eq!(
        TraceContext::Span { trace_id, span_id }.trace_id(),
        Some(trace_id)
    );
}
