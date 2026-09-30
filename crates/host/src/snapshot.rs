use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub cpu: Cpu,
    pub load_average: LoadAverage,
    pub memory: Memory,
    pub swap: Swap,
    pub filesystems: Vec<Filesystem>,
    pub interfaces: Vec<Interface>,
    pub services: Services,
    pub otelo_process: ProcessUsage,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cpu {
    TicksByMode(CpuTicks),
    Utilization(f64),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CpuTicks {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoadAverage {
    pub one_minute: f64,
    pub five_minutes: f64,
    pub fifteen_minutes: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Memory {
    Meminfo {
        total_bytes: u64,
        free_bytes: u64,
        cached_bytes: u64,
        reclaimable_slab_bytes: u64,
        buffers_bytes: u64,
    },
    UsedAndFree {
        total_bytes: u64,
        used_bytes: u64,
        free_bytes: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Swap {
    pub used_bytes: u64,
    pub free_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filesystem {
    pub device: String,
    pub mount_point: PathBuf,
    pub filesystem_type: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interface {
    pub name: String,
    pub received_bytes: u64,
    pub transmitted_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Services {
    Cgroups {
        units: Vec<Unit>,
        otelo_unit_name: Option<String>,
    },
    ProcessTrees {
        jobs: Vec<LaunchdJob>,
        processes: Vec<Process>,
        otelo_pid: Pid,
    },
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unit {
    pub name: String,
    pub cpu_time: Duration,
    pub memory: Option<CgroupMemory>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CgroupMemory {
    pub anonymous_bytes: u64,
    pub charged_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchdJob {
    pub label: String,
    pub main_pid: Pid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Process {
    pub pid: Pid,
    pub parent_pid: Option<Pid>,
    pub usage: ProcessUsage,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProcessUsage {
    pub cpu_time: Duration,
    pub resident_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pid(pub u32);
