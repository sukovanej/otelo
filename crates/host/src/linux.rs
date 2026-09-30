use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs, io};

use anyhow::Context;

use crate::snapshot::{CgroupMemory, CpuTicks, Memory, Services, Unit};

const BYTES_PER_KIBIBYTE: u64 = 1024;

// systemd keeps every system service in a cgroup under this slice.
const SYSTEM_SLICE_NAME: &str = "system.slice";

pub struct LinuxFiles {
    proc: PathBuf,
    cgroup: PathBuf,
}

impl LinuxFiles {
    #[must_use]
    pub fn at_system_paths() -> Self {
        Self::rooted_at("/proc".into(), "/sys/fs/cgroup".into())
    }

    #[must_use]
    pub const fn rooted_at(proc: PathBuf, cgroup: PathBuf) -> Self {
        Self { proc, cgroup }
    }

    pub fn read_cpu_ticks(&self) -> anyhow::Result<CpuTicks> {
        let path = self.proc.join("stat");
        let stat = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let total = stat
            .lines()
            .find_map(|line| line.strip_prefix("cpu "))
            .with_context(|| format!("{} has no line of all CPUs", path.display()))?;
        let mut ticks = total
            .split_ascii_whitespace()
            .map(|ticks| ticks.parse::<u64>().ok());
        let mut next = || {
            ticks
                .next()
                .flatten()
                .with_context(|| format!("the CPU line of {} is short", path.display()))
        };
        Ok(CpuTicks {
            user: next()?,
            nice: next()?,
            system: next()?,
            idle: next()?,
            iowait: next()?,
            irq: next()?,
            softirq: next()?,
            steal: next()?,
        })
    }

    pub fn read_memory(&self) -> anyhow::Result<Memory> {
        let path = self.proc.join("meminfo");
        let meminfo =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let bytes_of = |field: &str| {
            meminfo
                .lines()
                .find_map(|line| line.strip_prefix(field)?.strip_prefix(':'))
                .and_then(|value| value.trim().strip_suffix("kB"))
                .and_then(|kibibytes| kibibytes.trim().parse::<u64>().ok())
                .map(|kibibytes| kibibytes * BYTES_PER_KIBIBYTE)
                .with_context(|| format!("{} has no {field} in kB", path.display()))
        };
        Ok(Memory::Meminfo {
            total_bytes: bytes_of("MemTotal")?,
            free_bytes: bytes_of("MemFree")?,
            cached_bytes: bytes_of("Cached")?,
            reclaimable_slab_bytes: bytes_of("SReclaimable")?,
            buffers_bytes: bytes_of("Buffers")?,
        })
    }

    pub fn read_services(&self) -> anyhow::Result<Services> {
        // Only the unified hierarchy of cgroup v2 has this file at its root.
        if !self.cgroup.join("cgroup.controllers").is_file() {
            return Ok(Services::Unavailable);
        }
        let mut units = Vec::new();
        let slice = self.cgroup.join(SYSTEM_SLICE_NAME);
        if slice.is_dir() {
            collect_units_of_slice(&slice, &mut units)
                .with_context(|| format!("read the units under {}", slice.display()))?;
        }
        units.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Services::Cgroups {
            units,
            otelo_unit: self.read_unit_of_this_process(),
        })
    }

    fn read_unit_of_this_process(&self) -> Option<String> {
        let cgroups = fs::read_to_string(self.proc.join("self/cgroup")).ok()?;
        let path = cgroups.lines().find_map(|line| line.strip_prefix("0::"))?;
        path.split('/')
            .find_map(|part| part.strip_suffix(".service"))
            .map(str::to_owned)
    }
}

// A template unit such as `getty@tty1.service` lives in a slice of its own under the slice.
fn collect_units_of_slice(slice: &Path, units: &mut Vec<Unit>) -> io::Result<()> {
    for entry in fs::read_dir(slice)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if name.strip_suffix(".slice").is_some() {
            collect_units_of_slice(&entry.path(), units)?;
        } else if let Some(name) = name.strip_suffix(".service") {
            units.extend(read_unit_if_running(name, &entry.path()));
        }
    }
    Ok(())
}

// A unit can stop while it is read, and its files go with it.
fn read_unit_if_running(name: &str, cgroup: &Path) -> Option<Unit> {
    let events = fs::read_to_string(cgroup.join("cgroup.events")).ok()?;
    if find_number_of_field(&events, "populated")? != 1 {
        return None;
    }
    let cpu = fs::read_to_string(cgroup.join("cpu.stat")).ok()?;
    Some(Unit {
        name: name.to_owned(),
        cpu_time: Duration::from_micros(find_number_of_field(&cpu, "usage_usec")?),
        memory: read_memory_if_accounted(cgroup),
    })
}

// A cgroup without the memory controller has neither file.
fn read_memory_if_accounted(cgroup: &Path) -> Option<CgroupMemory> {
    let stat = fs::read_to_string(cgroup.join("memory.stat")).ok()?;
    let current = fs::read_to_string(cgroup.join("memory.current")).ok()?;
    Some(CgroupMemory {
        anonymous_bytes: find_number_of_field(&stat, "anon")?,
        charged_bytes: current.trim().parse().ok()?,
    })
}

fn find_number_of_field(lines: &str, field: &str) -> Option<u64> {
    lines.lines().find_map(|line| {
        let (name, value) = line.split_once(' ')?;
        (name == field).then(|| value.trim().parse().ok())?
    })
}
