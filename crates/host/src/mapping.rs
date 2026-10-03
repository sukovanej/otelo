use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

use otelo_indexed_storage::{
    Attributes, Batch, Metric, NumberPoint, Points, Records, StorageSize, Temporality,
};

use crate::identity::HostIdentity;
use crate::snapshot::{
    Cpu, CpuTicks, Filesystem, Interface, LaunchdJob, Memory, Pid, Process, ProcessUsage, Services,
    Snapshot, Unit,
};

const OTELO_SERVICE_NAME: &str = "otelo";

const BYTES_UNIT: &str = "By";
const SECONDS_UNIT: &str = "s";
const RATIO_UNIT: &str = "1";
// The OpenTelemetry Collector gives the load averages this unit.
const THREADS_UNIT: &str = "{thread}";

// A filesystem of these types has no disk behind it. squashfs is how Ubuntu mounts each snap.
const FILESYSTEM_TYPES_WITHOUT_DISK: [&str; 8] = [
    "tmpfs", "devtmpfs", "overlay", "squashfs", "devfs", "autofs", "proc", "sysfs",
];

const LOOPBACK_INTERFACE_NAMES: [&str; 2] = ["lo", "lo0"];

pub struct SnapshotMapper {
    host: HostIdentity,
    previous_cpu_ticks: Option<CpuTicks>,
    process_tree_cpu_by_job_label: HashMap<String, ProcessTreeCpu>,
}

// A sum over the live processes of a tree would fall when a child exits, and a reader takes a
// counter that fell for one that started again. This total only grows.
#[derive(Default)]
struct ProcessTreeCpu {
    total_cpu_time: Duration,
    cpu_time_seen_by_pid: HashMap<Pid, Duration>,
}

impl SnapshotMapper {
    #[must_use]
    pub fn new(host: HostIdentity) -> Self {
        Self {
            host,
            previous_cpu_ticks: None,
            process_tree_cpu_by_job_label: HashMap::new(),
        }
    }

    pub fn map_snapshot_to_batch(
        &mut self,
        recorded_at: i64,
        snapshot: &Snapshot,
        storage_size: Option<StorageSize>,
    ) -> Batch {
        let mut otelo_metrics = TickMetrics::new(recorded_at);
        self.push_cpu_utilization(&mut otelo_metrics, snapshot.cpu);
        push_load_average(&mut otelo_metrics, snapshot);
        push_memory(&mut otelo_metrics, snapshot.memory);
        push_swap(&mut otelo_metrics, snapshot);
        push_filesystems(&mut otelo_metrics, &snapshot.filesystems);
        push_interfaces(&mut otelo_metrics, &snapshot.interfaces);
        push_process_usage(&mut otelo_metrics, snapshot.otelo_process);
        if let Some(storage_size) = storage_size {
            push_storage_size(&mut otelo_metrics, storage_size);
        }
        let mut batch = vec![self.records_of_service(OTELO_SERVICE_NAME, otelo_metrics)];
        for (service, tick_metrics) in self.map_services(recorded_at, &snapshot.services) {
            batch.push(self.records_of_service(&service, tick_metrics));
        }
        batch
    }

    fn records_of_service(&self, service: &str, tick_metrics: TickMetrics) -> Records {
        Records {
            resource: self.host.resource_of_service(service),
            logs: Vec::new(),
            spans: Vec::new(),
            metrics: tick_metrics.metrics,
        }
    }

    // The shares need two readings, so the first snapshot gives none.
    fn push_cpu_utilization(&mut self, metrics: &mut TickMetrics, cpu: Cpu) {
        match cpu {
            Cpu::Utilization(share) => {
                metrics.push_gauge("system.cpu.utilization", RATIO_UNIT, &[], share);
            }
            Cpu::TicksByMode(ticks) => {
                let Some(previous_ticks) = self.previous_cpu_ticks.replace(ticks) else {
                    return;
                };
                let ticks_since = |now: u64, before: u64| now.saturating_sub(before);
                let ticks_by_mode = [
                    ("user", ticks_since(ticks.user, previous_ticks.user)),
                    ("nice", ticks_since(ticks.nice, previous_ticks.nice)),
                    ("system", ticks_since(ticks.system, previous_ticks.system)),
                    (
                        "interrupt",
                        ticks_since(ticks.irq, previous_ticks.irq)
                            + ticks_since(ticks.softirq, previous_ticks.softirq),
                    ),
                    ("iowait", ticks_since(ticks.iowait, previous_ticks.iowait)),
                    ("steal", ticks_since(ticks.steal, previous_ticks.steal)),
                    ("idle", ticks_since(ticks.idle, previous_ticks.idle)),
                ];
                let all_ticks: u64 = ticks_by_mode.iter().map(|(_, ticks)| ticks).sum();
                if all_ticks == 0 {
                    return;
                }
                #[expect(clippy::cast_precision_loss, reason = "the ticks of 15 seconds")]
                for (mode, mode_ticks) in ticks_by_mode {
                    metrics.push_gauge(
                        "system.cpu.utilization",
                        RATIO_UNIT,
                        &[("cpu.mode", mode)],
                        mode_ticks as f64 / all_ticks as f64,
                    );
                }
            }
        }
    }

    fn map_services(
        &mut self,
        recorded_at: i64,
        services: &Services,
    ) -> Vec<(String, TickMetrics)> {
        match services {
            Services::Unavailable => Vec::new(),
            Services::Cgroups {
                units,
                otelo_unit_name,
            } => units
                .iter()
                .filter(|unit| otelo_unit_name.as_deref() != Some(unit.name.as_str()))
                .map(|unit| (unit.name.clone(), metrics_of_unit(recorded_at, unit)))
                .collect(),
            Services::ProcessTrees {
                jobs,
                processes,
                otelo_pid,
            } => self.map_process_trees(recorded_at, jobs, processes, *otelo_pid),
        }
    }

    fn map_process_trees(
        &mut self,
        recorded_at: i64,
        jobs: &[LaunchdJob],
        processes: &[Process],
        otelo_pid: Pid,
    ) -> Vec<(String, TickMetrics)> {
        let process_trees = ProcessTrees::from_processes(processes);
        let mut metrics_by_job_label = Vec::new();
        let mut running_job_labels = HashSet::new();
        for job in jobs {
            let tree_processes = process_trees.collect_processes_under(job.main_pid);
            if tree_processes.is_empty()
                || tree_processes
                    .iter()
                    .any(|process| process.pid == otelo_pid)
            {
                continue;
            }
            running_job_labels.insert(job.label.as_str());
            let tree_cpu_entry = self.process_tree_cpu_by_job_label.entry(job.label.clone());
            let is_first_reading =
                matches!(tree_cpu_entry, std::collections::hash_map::Entry::Vacant(_));
            let tree_cpu = tree_cpu_entry.or_default();
            let mut cpu_time_seen_by_pid = HashMap::new();
            for process in &tree_processes {
                let cpu_time = process.usage.cpu_time;
                // A process the last reading did not have started after it, and so did one
                // whose time fell: another process took its PID.
                let gained_cpu_time = tree_cpu
                    .cpu_time_seen_by_pid
                    .get(&process.pid)
                    .and_then(|&seen_cpu_time| cpu_time.checked_sub(seen_cpu_time))
                    .unwrap_or(cpu_time);
                if !is_first_reading {
                    tree_cpu.total_cpu_time += gained_cpu_time;
                }
                cpu_time_seen_by_pid.insert(process.pid, cpu_time);
            }
            tree_cpu.cpu_time_seen_by_pid = cpu_time_seen_by_pid;
            let mut metrics = TickMetrics::new(recorded_at);
            push_process_usage(
                &mut metrics,
                ProcessUsage {
                    cpu_time: tree_cpu.total_cpu_time,
                    resident_bytes: tree_processes
                        .iter()
                        .map(|process| process.usage.resident_bytes)
                        .sum(),
                },
            );
            metrics_by_job_label.push((job.label.clone(), metrics));
        }
        self.process_tree_cpu_by_job_label
            .retain(|job_label, _| running_job_labels.contains(job_label.as_str()));
        metrics_by_job_label
    }
}

struct ProcessTrees<'a> {
    processes_by_pid: HashMap<Pid, &'a Process>,
    child_pids_by_pid: HashMap<Pid, Vec<Pid>>,
}

impl<'a> ProcessTrees<'a> {
    fn from_processes(processes: &'a [Process]) -> Self {
        let mut child_pids_by_pid: HashMap<Pid, Vec<Pid>> = HashMap::new();
        for process in processes {
            if let Some(parent_pid) = process.parent_pid {
                child_pids_by_pid
                    .entry(parent_pid)
                    .or_default()
                    .push(process.pid);
            }
        }
        Self {
            processes_by_pid: processes
                .iter()
                .map(|process| (process.pid, process))
                .collect(),
            child_pids_by_pid,
        }
    }

    fn collect_processes_under(&self, root_pid: Pid) -> Vec<&'a Process> {
        let mut tree_processes = Vec::new();
        let mut visited_pids = HashSet::new();
        let mut pids_to_visit = vec![root_pid];
        while let Some(pid) = pids_to_visit.pop() {
            if !visited_pids.insert(pid) {
                continue;
            }
            if let Some(process) = self.processes_by_pid.get(&pid) {
                tree_processes.push(*process);
                pids_to_visit.extend(self.child_pids_by_pid.get(&pid).into_iter().flatten());
            }
        }
        tree_processes
    }
}

struct TickMetrics {
    recorded_at: i64,
    metrics: Vec<Metric>,
}

impl TickMetrics {
    const fn new(recorded_at: i64) -> Self {
        Self {
            recorded_at,
            metrics: Vec::new(),
        }
    }

    fn push_point(
        &mut self,
        name: &str,
        unit: &str,
        attribute_pairs: &[(&str, &str)],
        value: f64,
        points_of_kind: fn(Vec<NumberPoint>) -> Points,
    ) {
        let mut attributes = Attributes::new();
        for &(key, value) in attribute_pairs {
            attributes.insert(key, value);
        }
        self.metrics.push(Metric {
            name: name.into(),
            unit: unit.into(),
            attributes,
            points: points_of_kind(vec![NumberPoint {
                recorded_at: self.recorded_at,
                value,
            }]),
        });
    }

    fn push_gauge(&mut self, name: &str, unit: &str, attribute_pairs: &[(&str, &str)], value: f64) {
        self.push_point(name, unit, attribute_pairs, value, Points::Gauge);
    }

    #[expect(clippy::cast_precision_loss, reason = "a level far below 2^53")]
    fn push_level(&mut self, name: &str, unit: &str, attribute_pairs: &[(&str, &str)], value: u64) {
        self.push_point(name, unit, attribute_pairs, value as f64, Points::UpDown);
    }

    fn push_counter(
        &mut self,
        name: &str,
        unit: &str,
        attribute_pairs: &[(&str, &str)],
        total: f64,
    ) {
        self.push_point(name, unit, attribute_pairs, total, |points| {
            Points::Counter(Temporality::Cumulative, points)
        });
    }
}

fn push_load_average(metrics: &mut TickMetrics, snapshot: &Snapshot) {
    let load_average = snapshot.load_average;
    for (name, threads) in [
        ("system.cpu.load_average.1m", load_average.one_minute),
        ("system.cpu.load_average.5m", load_average.five_minutes),
        ("system.cpu.load_average.15m", load_average.fifteen_minutes),
    ] {
        metrics.push_gauge(name, THREADS_UNIT, &[], threads);
    }
}

fn push_memory(metrics: &mut TickMetrics, memory: Memory) {
    let (limit_bytes, bytes_by_state) = match memory {
        Memory::Meminfo {
            total_bytes,
            free_bytes,
            cached_bytes,
            reclaimable_slab_bytes,
            buffers_bytes,
        } => {
            // The OpenTelemetry Collector counts the slab the kernel can give back as cache.
            let cache_bytes = cached_bytes + reclaimable_slab_bytes;
            let used_bytes = total_bytes
                .saturating_sub(free_bytes)
                .saturating_sub(cache_bytes)
                .saturating_sub(buffers_bytes);
            (
                total_bytes,
                vec![
                    ("used", used_bytes),
                    ("free", free_bytes),
                    ("cached", cache_bytes),
                    ("buffers", buffers_bytes),
                ],
            )
        }
        Memory::UsedAndFree {
            total_bytes,
            used_bytes,
            free_bytes,
        } => (
            total_bytes,
            vec![("used", used_bytes), ("free", free_bytes)],
        ),
    };
    for (state, bytes) in bytes_by_state {
        metrics.push_level(
            "system.memory.usage",
            BYTES_UNIT,
            &[("system.memory.state", state)],
            bytes,
        );
    }
    metrics.push_level("system.memory.limit", BYTES_UNIT, &[], limit_bytes);
}

fn push_swap(metrics: &mut TickMetrics, snapshot: &Snapshot) {
    let swap = snapshot.swap;
    if swap.used_bytes + swap.free_bytes == 0 {
        return;
    }
    for (state, bytes) in [("used", swap.used_bytes), ("free", swap.free_bytes)] {
        metrics.push_level(
            "system.paging.usage",
            BYTES_UNIT,
            &[("system.paging.state", state)],
            bytes,
        );
    }
}

fn push_filesystems(metrics: &mut TickMetrics, filesystems: &[Filesystem]) {
    // The volumes of one APFS container share its space, and every one of them reports the
    // size and the free space of the container.
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    enum Disk<'a> {
        Device(&'a str),
        ApfsContainer {
            total_bytes: u64,
            available_bytes: u64,
        },
    }
    let mut shortest_mounted_by_disk: BTreeMap<Disk, &Filesystem> = BTreeMap::new();
    for filesystem in filesystems {
        let filesystem_type = filesystem.filesystem_type.as_str();
        if filesystem.total_bytes == 0 || FILESYSTEM_TYPES_WITHOUT_DISK.contains(&filesystem_type) {
            continue;
        }
        let disk = if filesystem_type == "apfs" {
            Disk::ApfsContainer {
                total_bytes: filesystem.total_bytes,
                available_bytes: filesystem.available_bytes,
            }
        } else {
            Disk::Device(&filesystem.device)
        };
        let mount_point_sort_key = |filesystem: &Filesystem| {
            (
                filesystem.mount_point.as_os_str().len(),
                filesystem.mount_point.clone(),
            )
        };
        shortest_mounted_by_disk
            .entry(disk)
            .and_modify(|shortest_mounted| {
                if mount_point_sort_key(filesystem) < mount_point_sort_key(shortest_mounted) {
                    *shortest_mounted = filesystem;
                }
            })
            .or_insert(filesystem);
    }
    let mut shown_filesystems: Vec<&Filesystem> = shortest_mounted_by_disk.into_values().collect();
    shown_filesystems.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
    for filesystem in shown_filesystems {
        let mount_point = filesystem.mount_point.to_string_lossy();
        let used_bytes = filesystem
            .total_bytes
            .saturating_sub(filesystem.available_bytes);
        for (state, bytes) in [("used", used_bytes), ("free", filesystem.available_bytes)] {
            metrics.push_level(
                "system.filesystem.usage",
                BYTES_UNIT,
                &[
                    ("system.device", &filesystem.device),
                    ("system.filesystem.mountpoint", &mount_point),
                    ("system.filesystem.type", &filesystem.filesystem_type),
                    ("system.filesystem.state", state),
                ],
                bytes,
            );
        }
    }
}

fn push_interfaces(metrics: &mut TickMetrics, interfaces: &[Interface]) {
    let mut shown_interfaces: Vec<&Interface> = interfaces
        .iter()
        .filter(|interface| !LOOPBACK_INTERFACE_NAMES.contains(&interface.name.as_str()))
        .filter(|interface| interface.received_bytes + interface.transmitted_bytes > 0)
        .collect();
    shown_interfaces.sort_by(|a, b| a.name.cmp(&b.name));
    for interface in shown_interfaces {
        #[expect(clippy::cast_precision_loss, reason = "a byte count far below 2^53")]
        for (direction, bytes) in [
            ("receive", interface.received_bytes),
            ("transmit", interface.transmitted_bytes),
        ] {
            metrics.push_counter(
                "system.network.io",
                BYTES_UNIT,
                &[
                    ("network.interface.name", &interface.name),
                    ("network.io.direction", direction),
                ],
                bytes as f64,
            );
        }
    }
}

fn push_process_usage(metrics: &mut TickMetrics, usage: ProcessUsage) {
    metrics.push_counter(
        "process.cpu.time",
        SECONDS_UNIT,
        &[],
        usage.cpu_time.as_secs_f64(),
    );
    metrics.push_level(
        "process.memory.usage",
        BYTES_UNIT,
        &[],
        usage.resident_bytes,
    );
}

fn metrics_of_unit(recorded_at: i64, unit: &Unit) -> TickMetrics {
    let mut metrics = TickMetrics::new(recorded_at);
    metrics.push_counter(
        "process.cpu.time",
        SECONDS_UNIT,
        &[],
        unit.cpu_time.as_secs_f64(),
    );
    if let Some(memory) = unit.memory {
        metrics.push_level(
            "process.memory.usage",
            BYTES_UNIT,
            &[],
            memory.anonymous_bytes,
        );
        metrics.push_level(
            "process.cgroup.memory.usage",
            BYTES_UNIT,
            &[],
            memory.charged_bytes,
        );
    }
    metrics
}

fn push_storage_size(metrics: &mut TickMetrics, storage_size: StorageSize) {
    for (file_kind, bytes) in [
        ("telemetry", storage_size.telemetry_bytes),
        ("state", storage_size.state_bytes),
    ] {
        metrics.push_level(
            "otelo.storage.size",
            BYTES_UNIT,
            &[("otelo.storage.file", file_kind)],
            bytes,
        );
    }
}
