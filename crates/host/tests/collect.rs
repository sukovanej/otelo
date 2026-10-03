use otelo_host::{Collector, HostIdentity};
use otelo_indexed_storage::{Batch, Points, StorageSize};

fn newest_value(batch: &Batch, service: &str, name: &str) -> Option<f64> {
    batch
        .iter()
        .filter(|records| records.resource.service == service)
        .flat_map(|records| &records.metrics)
        .filter(|metric| metric.name == name)
        .find_map(|metric| match &metric.points {
            Points::Gauge(points) | Points::UpDown(points) | Points::Counter(_, points) => {
                points.last().map(|point| point.value)
            }
            Points::Histogram(..) => None,
        })
}

#[test]
fn reads_the_machine_the_test_runs_on() {
    let host = HostIdentity::read_from_this_machine();
    assert!(host.attributes().iter().any(|(key, _)| *key == "os.type"));
    let mut collector = Collector::new(host).unwrap();
    let storage_size = StorageSize {
        telemetry_bytes: 1,
        state_bytes: 2,
    };
    let first_batch = collector.collect_batch(1, Some(storage_size)).unwrap();
    let second_batch = collector.collect_batch(2, Some(storage_size)).unwrap();
    for batch in [&first_batch, &second_batch] {
        assert_eq!(batch[0].resource.service, "otelo");
        assert!(newest_value(batch, "otelo", "system.memory.limit").unwrap() > 0.0);
        assert!(newest_value(batch, "otelo", "process.memory.usage").unwrap() > 0.0);
        assert!(newest_value(batch, "otelo", "system.cpu.load_average.1m").is_some());
        assert!(newest_value(batch, "otelo", "otelo.storage.size").is_some());
    }
    let cpu_time = |batch| newest_value(batch, "otelo", "process.cpu.time").unwrap();
    assert!(cpu_time(&second_batch) >= cpu_time(&first_batch));
}
