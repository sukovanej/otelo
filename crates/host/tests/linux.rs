use std::fs;
use std::path::Path;
use std::time::Duration;

use otelo_host::{CgroupMemory, CpuTicks, LinuxFiles, Memory, Services, Unit};

// In the format of /proc/stat on a machine with one CPU.
const PROC_STAT: &str = "cpu  10132153 290696 3084719 46828483 16683 0 25195 175628 0 0
cpu0 10132153 290696 3084719 46828483 16683 0 25195 175628 0 0
intr 1462898 0 0 0 0 0 0 0 0 0
ctxt 115315
btime 1769904000
processes 53421
procs_running 1
procs_blocked 0
softirq 5026 0 1560 3 418 1342 0 1 0 0 1702
";

// In the format of /proc/meminfo, with the fields of a machine with 1 GB.
const PROC_MEMINFO: &str = "MemTotal:         980204 kB
MemFree:           85332 kB
MemAvailable:     520148 kB
Buffers:           31916 kB
Cached:           496224 kB
SwapCached:            0 kB
Active:           312504 kB
Shmem:              1268 kB
KReclaimable:      58432 kB
Slab:             104876 kB
SReclaimable:      58432 kB
SUnreclaim:        46444 kB
SwapTotal:             0 kB
SwapFree:              0 kB
";

const UNIT_CPU_STAT: &str = "usage_usec 93211250
user_usec 61200400
system_usec 32010850
nr_periods 0
nr_throttled 0
throttled_usec 0
";

const UNIT_MEMORY_STAT: &str = "anon 45211648
file 120397824
kernel 5742592
shmem 0
file_mapped 23781376
";

fn write_file(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

struct Machine {
    directory: tempfile::TempDir,
}

impl Machine {
    fn with_cgroup_v2() -> Self {
        let machine = Self {
            directory: tempfile::tempdir().unwrap(),
        };
        machine.write_file("proc/stat", PROC_STAT);
        machine.write_file("proc/meminfo", PROC_MEMINFO);
        machine.write_file("proc/self/cgroup", "0::/system.slice/otelo.service\n");
        machine.write_file("cgroup/cgroup.controllers", "cpuset cpu io memory pids\n");
        machine
    }

    fn write_file(&self, path: &str, text: &str) {
        write_file(&self.directory.path().join(path), text);
    }

    fn write_unit(&self, path: &str, is_populated: bool) {
        let unit_cgroup_directory = format!("cgroup/system.slice/{path}");
        self.write_file(
            &format!("{unit_cgroup_directory}/cgroup.events"),
            &format!("populated {}\nfrozen 0\n", u8::from(is_populated)),
        );
        self.write_file(&format!("{unit_cgroup_directory}/cpu.stat"), UNIT_CPU_STAT);
        self.write_file(
            &format!("{unit_cgroup_directory}/memory.stat"),
            UNIT_MEMORY_STAT,
        );
        self.write_file(
            &format!("{unit_cgroup_directory}/memory.current"),
            "171352064\n",
        );
    }

    fn linux_files(&self) -> LinuxFiles {
        LinuxFiles::rooted_at(
            self.directory.path().join("proc"),
            self.directory.path().join("cgroup"),
        )
    }
}

const fn running_unit(name: String) -> Unit {
    Unit {
        name,
        cpu_time: Duration::from_micros(93_211_250),
        memory: Some(CgroupMemory {
            anonymous_bytes: 45_211_648,
            charged_bytes: 171_352_064,
        }),
    }
}

#[test]
fn reads_the_ticks_of_all_cpus_by_mode() {
    let machine = Machine::with_cgroup_v2();
    assert_eq!(
        machine.linux_files().read_cpu_ticks().unwrap(),
        CpuTicks {
            user: 10_132_153,
            nice: 290_696,
            system: 3_084_719,
            idle: 46_828_483,
            iowait: 16_683,
            irq: 0,
            softirq: 25_195,
            steal: 175_628,
        }
    );
    machine.write_file("proc/stat", "cpu  1 2 3\n");
    assert!(machine.linux_files().read_cpu_ticks().is_err());
}

#[test]
fn reads_the_memory_in_bytes() {
    let machine = Machine::with_cgroup_v2();
    assert_eq!(
        machine.linux_files().read_memory().unwrap(),
        Memory::Meminfo {
            total_bytes: 980_204 * 1024,
            free_bytes: 85_332 * 1024,
            cached_bytes: 496_224 * 1024,
            reclaimable_slab_bytes: 58_432 * 1024,
            buffers_bytes: 31_916 * 1024,
        }
    );
}

#[test]
fn lists_the_units_that_have_processes() {
    let machine = Machine::with_cgroup_v2();
    machine.write_unit("mudro.service", true);
    machine.write_unit("otelo.service", true);
    machine.write_unit("fstrim.service", false);
    // A template unit has a slice of its own.
    machine.write_unit("system-getty.slice/getty@tty1.service", true);
    // A container is a scope, not a service.
    machine.write_unit("docker-4f2a.scope", true);
    // This unit has no memory controller.
    machine.write_unit("caddy.service", true);
    for file in ["memory.stat", "memory.current"] {
        let path = format!("cgroup/system.slice/caddy.service/{file}");
        fs::remove_file(machine.directory.path().join(path)).unwrap();
    }

    assert_eq!(
        machine.linux_files().read_services().unwrap(),
        Services::Cgroups {
            units: vec![
                Unit {
                    memory: None,
                    ..running_unit("caddy".into())
                },
                running_unit("getty@tty1".into()),
                running_unit("mudro".into()),
                running_unit("otelo".into()),
            ],
            otelo_unit_name: Some("otelo".into()),
        }
    );
}

#[test]
fn a_process_outside_a_unit_has_no_unit_of_its_own() {
    let machine = Machine::with_cgroup_v2();
    machine.write_file(
        "proc/self/cgroup",
        "0::/user.slice/user-1000.slice/session-3.scope\n",
    );
    machine.write_unit("mudro.service", true);
    assert_eq!(
        machine.linux_files().read_services().unwrap(),
        Services::Cgroups {
            units: vec![running_unit("mudro".into())],
            otelo_unit_name: None,
        }
    );
}

#[test]
fn a_machine_without_cgroup_v2_has_no_services() {
    let machine = Machine::with_cgroup_v2();
    machine.write_unit("mudro.service", true);
    fs::remove_file(machine.directory.path().join("cgroup/cgroup.controllers")).unwrap();
    assert_eq!(
        machine.linux_files().read_services().unwrap(),
        Services::Unavailable
    );
}
