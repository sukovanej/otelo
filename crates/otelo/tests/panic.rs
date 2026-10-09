use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tracing::field::{Field, Visit};
use tracing_subscriber::Registry;
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

type EventFields = BTreeMap<String, String>;

#[derive(Clone, Default)]
struct LoggedEvents(Arc<Mutex<Vec<(tracing::Level, EventFields)>>>);

struct FieldVisitor<'a>(&'a mut EventFields);

impl Visit for FieldVisitor<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().to_owned(), value.to_owned());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.0.insert(field.name().to_owned(), value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.insert(field.name().to_owned(), format!("{value:?}"));
    }
}

impl<S: tracing::Subscriber> Layer<S> for LoggedEvents {
    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        let mut fields = EventFields::new();
        event.record(&mut FieldVisitor(&mut fields));
        self.0
            .lock()
            .unwrap()
            .push((*event.metadata().level(), fields));
    }
}

// The hook and the subscriber are global, so this binary holds one test.
#[test]
fn a_panic_of_a_thread_is_logged_as_an_error_with_where_it_happened() {
    let logged_events = LoggedEvents::default();
    tracing::subscriber::set_global_default(Registry::default().with(logged_events.clone()))
        .unwrap();
    otelo::own::log_panics_as_errors();

    let panicked_line = line!() + 3;
    let joined = std::thread::Builder::new()
        .name("telemetry-indexer".into())
        .spawn(|| panic!("the telemetry file is gone"))
        .unwrap()
        .join();
    assert!(joined.is_err());

    let events = logged_events.0.lock().unwrap().clone();
    let [(level, fields)] = events.as_slice() else {
        panic!("{events:?}");
    };
    assert_eq!(*level, tracing::Level::ERROR);
    assert_eq!(fields["message"], "panicked");
    assert_eq!(fields["exception.message"], "the telemetry file is gone");
    assert_eq!(fields["thread.name"], "telemetry-indexer");
    assert_eq!(fields["code.file.path"], file!());
    assert_eq!(fields["code.line.number"], panicked_line.to_string());
    assert!(
        fields["exception.stacktrace"].contains("panic"),
        "{}",
        fields["exception.stacktrace"]
    );
}
