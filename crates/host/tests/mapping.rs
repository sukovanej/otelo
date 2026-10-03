use std::time::Duration;

use otelo_host::{
    CgroupMemory, Cpu, CpuTicks, Filesystem, HostIdentity, Interface, LaunchdJob, LoadAverage,
    Memory, Pid, Process, ProcessUsage, Services, Snapshot, SnapshotMapper, Swap, Unit,
};
use otelo_indexed_storage::{Batch, Points, StorageSize, Temporality};
use serde_json::{Value, json};

const TICK_AT: i64 = 1_790_769_600_000_000_000;

fn droplet() -> HostIdentity {
    HostIdentity {
        name: Some("mudro-prod".into()),
        id: Some("0f5c2d8e4b1a4c6f9d3e7a8b9c0d1e2f".into()),
        arch: "amd64",
        os_type: "linux",
    }
}

fn idle_machine() -> Snapshot {
    Snapshot {
        cpu: Cpu::TicksByMode(CpuTicks::default()),
        load_average: LoadAverage {
            one_minute: 0.5,
            five_minutes: 0.25,
            fifteen_minutes: 0.125,
        },
        memory: Memory::UsedAndFree {
            total_bytes: 1000,
            used_bytes: 600,
            free_bytes: 100,
        },
        swap: Swap {
            used_bytes: 0,
            free_bytes: 0,
        },
        filesystems: Vec::new(),
        interfaces: Vec::new(),
        services: Services::Unavailable,
        otelo_process: ProcessUsage {
            cpu_time: Duration::from_millis(1500),
            resident_bytes: 40_000_000,
        },
    }
}

struct PointRow {
    service: String,
    name: String,
    kind: &'static str,
    unit: String,
    labels: Value,
    value: f64,
}

fn collect_point_rows(batch: &Batch) -> Vec<PointRow> {
    let mut rows = Vec::new();
    for records in batch {
        for metric in &records.metrics {
            let (kind, points) = match &metric.points {
                Points::Gauge(points) => ("gauge", points),
                Points::UpDown(points) => ("updown", points),
                Points::Counter(Temporality::Cumulative, points) => ("counter", points),
                other => panic!("the collector sends no {other:?}"),
            };
            for point in points {
                assert_eq!(point.recorded_at, TICK_AT, "{}", metric.name);
                rows.push(PointRow {
                    service: records.resource.service.clone(),
                    name: metric.name.clone(),
                    kind,
                    unit: metric.unit.clone(),
                    labels: serde_json::from_str(&metric.labels.to_json()).unwrap(),
                    value: point.value,
                });
            }
        }
    }
    rows
}

fn map_to_point_rows(snapshot_mapper: &mut SnapshotMapper, snapshot: &Snapshot) -> Vec<PointRow> {
    collect_point_rows(&snapshot_mapper.map_snapshot_to_batch(TICK_AT, snapshot, None))
}

fn values_by_label(rows: &[PointRow], name: &str, label: &str) -> Vec<(String, f64)> {
    rows.iter()
        .filter(|row| row.name == name)
        .map(|row| (row.labels[label].as_str().unwrap().to_owned(), row.value))
        .collect()
}

fn values_of_service(rows: &[PointRow], service: &str) -> Vec<(String, f64)> {
    rows.iter()
        .filter(|row| row.service == service && row.name.starts_with("process."))
        .map(|row| (row.name.clone(), row.value))
        .collect()
}

fn labelled(pairs: &[(&str, f64)]) -> Vec<(String, f64)> {
    pairs
        .iter()
        .map(|&(label, value)| (label.to_owned(), value))
        .collect()
}

#[test]
fn the_cpu_shares_are_the_change_between_two_readings() {
    let mut snapshot_mapper = SnapshotMapper::new(droplet());
    let first_ticks = CpuTicks {
        user: 1000,
        nice: 10,
        system: 500,
        idle: 9000,
        iowait: 40,
        irq: 5,
        softirq: 20,
        steal: 70,
    };
    let second_ticks = CpuTicks {
        user: first_ticks.user + 30,
        nice: first_ticks.nice,
        system: first_ticks.system + 10,
        idle: first_ticks.idle + 50,
        iowait: first_ticks.iowait + 5,
        irq: first_ticks.irq + 1,
        softirq: first_ticks.softirq + 1,
        steal: first_ticks.steal + 3,
    };
    let snapshot_with_ticks = |ticks| Snapshot {
        cpu: Cpu::TicksByMode(ticks),
        ..idle_machine()
    };
    let rows = map_to_point_rows(&mut snapshot_mapper, &snapshot_with_ticks(first_ticks));
    assert!(rows.iter().all(|row| row.name != "system.cpu.utilization"));

    let rows = map_to_point_rows(&mut snapshot_mapper, &snapshot_with_ticks(second_ticks));
    assert_eq!(
        values_by_label(&rows, "system.cpu.utilization", "cpu.mode"),
        labelled(&[
            ("user", 0.3),
            ("nice", 0.0),
            ("system", 0.1),
            ("interrupt", 0.02),
            ("iowait", 0.05),
            ("steal", 0.03),
            ("idle", 0.5),
        ])
    );
    let utilization_row = rows
        .iter()
        .find(|row| row.name == "system.cpu.utilization")
        .unwrap();
    assert_eq!(
        (utilization_row.kind, utilization_row.unit.as_str()),
        ("gauge", "1")
    );

    // No tick passed between two readings, so there is nothing to share out.
    let rows = map_to_point_rows(&mut snapshot_mapper, &snapshot_with_ticks(second_ticks));
    assert!(rows.iter().all(|row| row.name != "system.cpu.utilization"));
}

#[test]
fn a_machine_without_cpu_modes_sends_one_share() {
    let mut snapshot_mapper = SnapshotMapper::new(droplet());
    let rows = map_to_point_rows(
        &mut snapshot_mapper,
        &Snapshot {
            cpu: Cpu::Utilization(0.25),
            ..idle_machine()
        },
    );
    let shares: Vec<&PointRow> = rows
        .iter()
        .filter(|row| row.name == "system.cpu.utilization")
        .collect();
    assert_eq!(shares.len(), 1);
    assert_eq!(shares[0].labels, json!({}));
    assert!((shares[0].value - 0.25).abs() < f64::EPSILON);
}

#[test]
fn the_memory_of_linux_has_four_states_that_add_up_to_the_limit() {
    let mut snapshot_mapper = SnapshotMapper::new(droplet());
    let rows = map_to_point_rows(
        &mut snapshot_mapper,
        &Snapshot {
            memory: Memory::Meminfo {
                total_bytes: 1000,
                free_bytes: 100,
                cached_bytes: 200,
                reclaimable_slab_bytes: 50,
                buffers_bytes: 30,
            },
            ..idle_machine()
        },
    );
    assert_eq!(
        values_by_label(&rows, "system.memory.usage", "system.memory.state"),
        labelled(&[
            ("used", 620.0),
            ("free", 100.0),
            ("cached", 250.0),
            ("buffers", 30.0),
        ])
    );
    let limit = rows
        .iter()
        .find(|row| row.name == "system.memory.limit")
        .unwrap();
    assert_eq!(
        (limit.kind, limit.unit.as_str(), limit.value.to_bits()),
        ("updown", "By", 1000_f64.to_bits())
    );
}

#[test]
fn the_memory_of_macos_has_the_states_it_reports() {
    let mut snapshot_mapper = SnapshotMapper::new(droplet());
    let rows = map_to_point_rows(&mut snapshot_mapper, &idle_machine());
    assert_eq!(
        values_by_label(&rows, "system.memory.usage", "system.memory.state"),
        labelled(&[("used", 600.0), ("free", 100.0)])
    );
}

#[test]
fn a_machine_without_swap_has_no_series_of_it() {
    let mut snapshot_mapper = SnapshotMapper::new(droplet());
    let rows = map_to_point_rows(&mut snapshot_mapper, &idle_machine());
    assert!(rows.iter().all(|row| row.name != "system.paging.usage"));

    let rows = map_to_point_rows(
        &mut snapshot_mapper,
        &Snapshot {
            swap: Swap {
                used_bytes: 0,
                free_bytes: 2048,
            },
            ..idle_machine()
        },
    );
    assert_eq!(
        values_by_label(&rows, "system.paging.usage", "system.paging.state"),
        labelled(&[("used", 0.0), ("free", 2048.0)])
    );
}

#[test]
fn the_load_averages_are_gauges_of_threads() {
    let mut snapshot_mapper = SnapshotMapper::new(droplet());
    let rows = map_to_point_rows(&mut snapshot_mapper, &idle_machine());
    let loads: Vec<(&str, &str, &str, u64)> = rows
        .iter()
        .filter(|row| row.name.starts_with("system.cpu.load_average"))
        .map(|row| {
            (
                row.name.as_str(),
                row.kind,
                row.unit.as_str(),
                row.value.to_bits(),
            )
        })
        .collect();
    assert_eq!(
        loads,
        [
            (
                "system.cpu.load_average.1m",
                "gauge",
                "{thread}",
                0.5_f64.to_bits()
            ),
            (
                "system.cpu.load_average.5m",
                "gauge",
                "{thread}",
                0.25_f64.to_bits()
            ),
            (
                "system.cpu.load_average.15m",
                "gauge",
                "{thread}",
                0.125_f64.to_bits()
            ),
        ]
    );
}

fn filesystem(
    device: &str,
    mount_point: &str,
    filesystem_type: &str,
    total_bytes: u64,
) -> Filesystem {
    Filesystem {
        device: device.into(),
        mount_point: mount_point.into(),
        filesystem_type: filesystem_type.into(),
        total_bytes,
        available_bytes: total_bytes / 4,
    }
}

#[test]
fn a_disk_counts_once_under_its_shortest_mount_point() {
    let mut snapshot_mapper = SnapshotMapper::new(droplet());
    let rows = map_to_point_rows(
        &mut snapshot_mapper,
        &Snapshot {
            filesystems: vec![
                filesystem("/dev/vda1", "/var/lib/docker/bind", "ext4", 8000),
                filesystem("/dev/vda1", "/", "ext4", 8000),
                filesystem("/dev/vda15", "/boot/efi", "vfat", 400),
                filesystem("/dev/loop3", "/snap/core22/1621", "squashfs", 100),
                filesystem("tmpfs", "/run", "tmpfs", 100),
                filesystem("/dev/sr0", "/media/empty", "ext4", 0),
            ],
            ..idle_machine()
        },
    );
    let usage: Vec<(Value, f64)> = rows
        .iter()
        .filter(|row| row.name == "system.filesystem.usage")
        .map(|row| (row.labels.clone(), row.value))
        .collect();
    let labels = |device: &str, mount_point: &str, filesystem_type: &str, state: &str| {
        json!({
            "system.device": device,
            "system.filesystem.mountpoint": mount_point,
            "system.filesystem.type": filesystem_type,
            "system.filesystem.state": state,
        })
    };
    assert_eq!(
        usage,
        [
            (labels("/dev/vda1", "/", "ext4", "used"), 6000.0),
            (labels("/dev/vda1", "/", "ext4", "free"), 2000.0),
            (labels("/dev/vda15", "/boot/efi", "vfat", "used"), 300.0),
            (labels("/dev/vda15", "/boot/efi", "vfat", "free"), 100.0),
        ]
    );
}

#[test]
fn the_volumes_of_an_apfs_container_count_once() {
    let mut snapshot_mapper = SnapshotMapper::new(droplet());
    let rows = map_to_point_rows(
        &mut snapshot_mapper,
        &Snapshot {
            filesystems: vec![
                filesystem("Data", "/System/Volumes/Data", "apfs", 494_384_795_648),
                filesystem("Macintosh HD", "/", "apfs", 494_384_795_648),
                filesystem("Backup", "/Volumes/Backup", "apfs", 1_000_000_000_000),
            ],
            ..idle_machine()
        },
    );
    let devices: Vec<(String, f64)> =
        values_by_label(&rows, "system.filesystem.usage", "system.device")
            .into_iter()
            .step_by(2)
            .collect();
    assert_eq!(
        devices,
        labelled(&[
            ("Macintosh HD", 370_788_596_736.0),
            ("Backup", 750_000_000_000.0),
        ])
    );
}

#[test]
fn leaves_out_the_loopback_and_the_idle_interfaces() {
    let mut snapshot_mapper = SnapshotMapper::new(droplet());
    let interface = |name: &str, received_bytes, transmitted_bytes| Interface {
        name: name.into(),
        received_bytes,
        transmitted_bytes,
    };
    let rows = map_to_point_rows(
        &mut snapshot_mapper,
        &Snapshot {
            interfaces: vec![
                interface("lo", 900, 900),
                interface("utun3", 0, 0),
                interface("eth0", 5000, 1200),
            ],
            ..idle_machine()
        },
    );
    let network_io_rows: Vec<&PointRow> = rows
        .iter()
        .filter(|row| row.name == "system.network.io")
        .collect();
    assert_eq!(network_io_rows.len(), 2);
    assert!(
        network_io_rows
            .iter()
            .all(|row| row.kind == "counter" && row.unit == "By")
    );
    assert_eq!(
        network_io_rows[0].labels,
        json!({"network.interface.name": "eth0", "network.io.direction": "receive"})
    );
    assert_eq!(
        values_by_label(&rows, "system.network.io", "network.io.direction"),
        labelled(&[("receive", 5000.0), ("transmit", 1200.0)])
    );
}

#[test]
fn a_unit_is_a_service_with_the_attributes_of_the_host() {
    let mut snapshot_mapper = SnapshotMapper::new(droplet());
    let unit = |name: &str, memory| Unit {
        name: name.into(),
        cpu_time: Duration::from_millis(93_250),
        memory,
    };
    let memory = Some(CgroupMemory {
        anonymous_bytes: 45_000_000,
        charged_bytes: 171_000_000,
    });
    let batch = snapshot_mapper.map_snapshot_to_batch(
        TICK_AT,
        &Snapshot {
            services: Services::Cgroups {
                units: vec![
                    unit("caddy", None),
                    unit("mudro", memory),
                    unit("otelo", memory),
                ],
                otelo_unit_name: Some("otelo".into()),
            },
            ..idle_machine()
        },
        None,
    );
    let services: Vec<&str> = batch
        .iter()
        .map(|records| records.resource.service.as_str())
        .collect();
    assert_eq!(services, ["otelo", "caddy", "mudro"]);
    assert_eq!(
        serde_json::from_str::<Value>(&batch[2].resource.attributes.to_json()).unwrap(),
        json!({
            "service.name": "mudro",
            "host.name": "mudro-prod",
            "host.id": "0f5c2d8e4b1a4c6f9d3e7a8b9c0d1e2f",
            "host.arch": "amd64",
            "os.type": "linux",
        })
    );

    let rows = collect_point_rows(&batch);
    assert_eq!(
        values_of_service(&rows, "caddy"),
        labelled(&[("process.cpu.time", 93.25)])
    );
    assert_eq!(
        values_of_service(&rows, "mudro"),
        labelled(&[
            ("process.cpu.time", 93.25),
            ("process.memory.usage", 45_000_000.0),
            ("process.cgroup.memory.usage", 171_000_000.0),
        ])
    );
    // otelo counts once, from its own process and not from its unit.
    assert_eq!(
        values_of_service(&rows, "otelo"),
        labelled(&[
            ("process.cpu.time", 1.5),
            ("process.memory.usage", 40_000_000.0),
        ])
    );
}

const fn process(pid: u32, parent_pid: u32, cpu_millis: u64, resident_bytes: u64) -> Process {
    Process {
        pid: Pid(pid),
        parent_pid: Some(Pid(parent_pid)),
        usage: ProcessUsage {
            cpu_time: Duration::from_millis(cpu_millis),
            resident_bytes,
        },
    }
}

fn launchd_machine(processes: Vec<Process>) -> Snapshot {
    let job = |label: &str, main_pid| LaunchdJob {
        label: label.into(),
        main_pid: Pid(main_pid),
    };
    Snapshot {
        services: Services::ProcessTrees {
            jobs: vec![
                job("com.example.mudro", 10),
                job("com.example.otelo", 50),
                job("com.example.stopped", 70),
            ],
            processes,
            otelo_pid: Pid(51),
        },
        ..idle_machine()
    }
}

#[test]
fn the_cpu_time_of_a_process_tree_never_falls_when_a_child_exits() {
    let mut snapshot_mapper = SnapshotMapper::new(droplet());
    let otelo_processes = [process(50, 1, 100, 10), process(51, 50, 100, 10)];
    let mut map_tick_with_mudro = |mudro_processes: &[Process]| {
        let processes = [
            mudro_processes,
            otelo_processes.as_slice(),
            &[process(99, 1, 9000, 9000)],
        ]
        .concat();
        let rows = map_to_point_rows(&mut snapshot_mapper, &launchd_machine(processes));
        let services: Vec<String> = rows
            .iter()
            .filter(|row| row.name == "process.cpu.time")
            .map(|row| row.service.clone())
            .collect();
        // The job that holds otelo and the job without a process are left out.
        assert_eq!(services, ["otelo", "com.example.mudro"]);
        values_of_service(&rows, "com.example.mudro")
    };

    // The first reading only sets where the counting starts.
    assert_eq!(
        map_tick_with_mudro(&[
            process(10, 1, 5000, 100),
            process(11, 10, 2000, 200),
            process(12, 11, 8000, 300),
        ]),
        labelled(&[("process.cpu.time", 0.0), ("process.memory.usage", 600.0)])
    );
    // The grandchild 12 exited with its 8 seconds. The others gained 1 and 2 seconds.
    assert_eq!(
        map_tick_with_mudro(&[process(10, 1, 6000, 100), process(11, 10, 4000, 250)]),
        labelled(&[("process.cpu.time", 3.0), ("process.memory.usage", 350.0)])
    );
    // A child that started since the last reading counts with all its time.
    assert_eq!(
        map_tick_with_mudro(&[
            process(10, 1, 6000, 100),
            process(11, 10, 4500, 250),
            process(13, 10, 250, 50),
        ]),
        labelled(&[("process.cpu.time", 3.75), ("process.memory.usage", 400.0)])
    );
}

#[test]
fn the_size_of_the_storage_is_a_level_by_kind_of_file() {
    let mut snapshot_mapper = SnapshotMapper::new(droplet());
    let storage_size = StorageSize {
        telemetry_bytes: 52_000_000,
        rollup_bytes: 4_000_000,
        state_bytes: 8192,
    };
    let rows = collect_point_rows(&snapshot_mapper.map_snapshot_to_batch(
        TICK_AT,
        &idle_machine(),
        Some(storage_size),
    ));
    assert_eq!(
        values_by_label(&rows, "otelo.storage.size", "otelo.storage.file"),
        labelled(&[
            ("telemetry", 52_000_000.0),
            ("rollup", 4_000_000.0),
            ("state", 8192.0),
        ])
    );
    let size_row = rows
        .iter()
        .find(|row| row.name == "otelo.storage.size")
        .unwrap();
    assert_eq!(
        (
            size_row.service.as_str(),
            size_row.kind,
            size_row.unit.as_str()
        ),
        ("otelo", "updown", "By")
    );

    let rows = map_to_point_rows(&mut snapshot_mapper, &idle_machine());
    assert!(rows.iter().all(|row| row.name != "otelo.storage.size"));
}
