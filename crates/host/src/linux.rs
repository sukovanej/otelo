use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs, io};

use anyhow::Context;

use crate::snapshot::{CgroupMemory, CpuTicks, Memory, Services, Unit};

const BYTES_PER_KIBIBYTE: u64 = 1024;

// systemd keeps every system service in a cgroup under this slice.
const SYSTEM_SLICE_NAME: &str = "system.slice";

pub struct LinuxFiles {
    proc_directory: PathBuf,
    cgroup_directory: PathBuf,
}

impl LinuxFiles {
    #[must_use]
    pub fn at_system_paths() -> Self {
        Self::rooted_at("/proc".into(), "/sys/fs/cgroup".into())
    }

    #[must_use]
    pub const fn rooted_at(proc_directory: PathBuf, cgroup_directory: PathBuf) -> Self {
        Self {
            proc_directory,
            cgroup_directory,
        }
    }

    pub fn read_cpu_ticks(&self) -> anyhow::Result<CpuTicks> {
        let path = self.proc_directory.join("stat");
        let proc_stat =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let all_cpus_line = proc_stat
            .lines()
            .find_map(|line| line.strip_prefix("cpu "))
            .with_context(|| format!("{} has no line of all CPUs", path.display()))?;
        let mut tick_counts = all_cpus_line
            .split_ascii_whitespace()
            .map(|tick_count| tick_count.parse::<u64>().ok());
        let mut read_next_tick_count = || {
            tick_counts
                .next()
                .flatten()
                .with_context(|| format!("the CPU line of {} is short", path.display()))
        };
        Ok(CpuTicks {
            user: read_next_tick_count()?,
            nice: read_next_tick_count()?,
            system: read_next_tick_count()?,
            idle: read_next_tick_count()?,
            iowait: read_next_tick_count()?,
            irq: read_next_tick_count()?,
            softirq: read_next_tick_count()?,
            steal: read_next_tick_count()?,
        })
    }

    pub fn read_memory(&self) -> anyhow::Result<Memory> {
        let path = self.proc_directory.join("meminfo");
        let meminfo =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let read_field_in_bytes = |field: &str| {
            meminfo
                .lines()
                .find_map(|line| line.strip_prefix(field)?.strip_prefix(':'))
                .and_then(|value| value.trim().strip_suffix("kB"))
                .and_then(|kibibytes| kibibytes.trim().parse::<u64>().ok())
                .map(|kibibytes| kibibytes * BYTES_PER_KIBIBYTE)
                .with_context(|| format!("{} has no {field} in kB", path.display()))
        };
        Ok(Memory::Meminfo {
            total_bytes: read_field_in_bytes("MemTotal")?,
            free_bytes: read_field_in_bytes("MemFree")?,
            cached_bytes: read_field_in_bytes("Cached")?,
            reclaimable_slab_bytes: read_field_in_bytes("SReclaimable")?,
            buffers_bytes: read_field_in_bytes("Buffers")?,
        })
    }

    pub fn read_services(&self) -> anyhow::Result<Services> {
        // Only the unified hierarchy of cgroup v2 has this file at its root.
        if !self.cgroup_directory.join("cgroup.controllers").is_file() {
            return Ok(Services::Unavailable);
        }
        let mut units = Vec::new();
        let system_slice_directory = self.cgroup_directory.join(SYSTEM_SLICE_NAME);
        if system_slice_directory.is_dir() {
            collect_units_of_slice(&system_slice_directory, &mut units).with_context(|| {
                format!("read the units under {}", system_slice_directory.display())
            })?;
        }
        units.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Services::Cgroups {
            units,
            otelo_unit_name: self.read_unit_name_of_this_process(),
        })
    }

    fn read_unit_name_of_this_process(&self) -> Option<String> {
        let own_cgroups = fs::read_to_string(self.proc_directory.join("self/cgroup")).ok()?;
        let cgroup_path = own_cgroups
            .lines()
            .find_map(|line| line.strip_prefix("0::"))?;
        cgroup_path
            .split('/')
            .find_map(|path_segment| path_segment.strip_suffix(".service"))
            .map(str::to_owned)
    }
}

// A template unit such as `getty@tty1.service` lives in a slice of its own under the slice.
fn collect_units_of_slice(slice_directory: &Path, units: &mut Vec<Unit>) -> io::Result<()> {
    for entry in fs::read_dir(slice_directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let Ok(directory_name) = entry.file_name().into_string() else {
            continue;
        };
        if directory_name.strip_suffix(".slice").is_some() {
            collect_units_of_slice(&entry.path(), units)?;
        } else if let Some(unit_name) = directory_name.strip_suffix(".service") {
            units.extend(read_unit_if_running(unit_name, &entry.path()));
        }
    }
    Ok(())
}

// A unit can stop while it is read, and its files go with it.
fn read_unit_if_running(unit_name: &str, cgroup_directory: &Path) -> Option<Unit> {
    let cgroup_events = fs::read_to_string(cgroup_directory.join("cgroup.events")).ok()?;
    if find_number_of_field(&cgroup_events, "populated")? != 1 {
        return None;
    }
    let cpu_stat = fs::read_to_string(cgroup_directory.join("cpu.stat")).ok()?;
    Some(Unit {
        name: unit_name.to_owned(),
        cpu_time: Duration::from_micros(find_number_of_field(&cpu_stat, "usage_usec")?),
        memory: read_memory_if_accounted(cgroup_directory),
    })
}

// A cgroup without the memory controller has neither file.
fn read_memory_if_accounted(cgroup_directory: &Path) -> Option<CgroupMemory> {
    let memory_stat = fs::read_to_string(cgroup_directory.join("memory.stat")).ok()?;
    let memory_current = fs::read_to_string(cgroup_directory.join("memory.current")).ok()?;
    Some(CgroupMemory {
        anonymous_bytes: find_number_of_field(&memory_stat, "anon")?,
        charged_bytes: memory_current.trim().parse().ok()?,
    })
}

fn find_number_of_field(file_contents: &str, field: &str) -> Option<u64> {
    file_contents.lines().find_map(|line| {
        let (name, value) = line.split_once(' ')?;
        (name == field).then(|| value.trim().parse().ok())?
    })
}
