use std::time::Duration;

use anyhow::Context;
use sysinfo::{Disks, Networks, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::launchd;
use crate::linux::LinuxFiles;
use crate::snapshot::{
    Cpu, Filesystem, Interface, LoadAverage, Memory, Pid, Process, ProcessUsage, Services,
    Snapshot, Swap,
};

const PERCENT_PER_WHOLE: f64 = 100.0;

pub struct MachineReader {
    system: System,
    disks: Disks,
    networks: Networks,
    linux_files: LinuxFiles,
    otelo_pid: sysinfo::Pid,
    has_warned_about_services: bool,
}

impl MachineReader {
    pub fn new() -> anyhow::Result<Self> {
        let mut system = System::new();
        // A share of the CPU is the change between two readings, and this is the first.
        system.refresh_cpu_usage();
        Ok(Self {
            system,
            disks: Disks::new(),
            networks: Networks::new(),
            linux_files: LinuxFiles::at_system_paths(),
            otelo_pid: sysinfo::get_current_pid()
                .map_err(|error| anyhow::anyhow!("find the PID of this process: {error}"))?,
            has_warned_about_services: false,
        })
    }

    pub fn read_snapshot(&mut self) -> anyhow::Result<Snapshot> {
        self.system.refresh_memory();
        let (cpu, memory) = if cfg!(target_os = "linux") {
            (
                Cpu::TicksByMode(self.linux_files.read_cpu_ticks()?),
                self.linux_files.read_memory()?,
            )
        } else {
            self.system.refresh_cpu_usage();
            (
                Cpu::Utilization(f64::from(self.system.global_cpu_usage()) / PERCENT_PER_WHOLE),
                Memory::UsedAndFree {
                    total_bytes: self.system.total_memory(),
                    used_bytes: self.system.used_memory(),
                    free_bytes: self.system.free_memory(),
                },
            )
        };
        let services = self.read_services();
        let otelo_process = self
            .read_usage_of_this_process()
            .context("read the CPU time and the memory of this process")?;
        let load_average = System::load_average();
        Ok(Snapshot {
            cpu,
            load_average: LoadAverage {
                one_minute: load_average.one,
                five_minutes: load_average.five,
                fifteen_minutes: load_average.fifteen,
            },
            memory,
            swap: Swap {
                used_bytes: self.system.used_swap(),
                free_bytes: self.system.free_swap(),
            },
            filesystems: self.read_filesystems(),
            interfaces: self.read_interfaces(),
            services,
            otelo_process,
        })
    }

    // A machine without a way to list its services still sends the rest.
    fn read_services(&mut self) -> Services {
        let services = if cfg!(target_os = "linux") {
            self.linux_files.read_services()
        } else if cfg!(target_os = "macos") {
            self.read_process_trees()
        } else {
            Ok(Services::Unavailable)
        };
        match services {
            Ok(Services::Unavailable) => {
                self.warn_once_about_services("this machine has no cgroup v2 and no launchd");
                Services::Unavailable
            }
            Ok(services) => services,
            Err(error) => {
                self.warn_once_about_services(&format!("{error:#}"));
                Services::Unavailable
            }
        }
    }

    fn warn_once_about_services(&mut self, reason: &str) {
        if !self.has_warned_about_services {
            tracing::warn!("no metrics of the services: {reason}");
            self.has_warned_about_services = true;
        }
    }

    fn read_process_trees(&mut self) -> anyhow::Result<Services> {
        let jobs = launchd::list_running_jobs()?;
        self.refresh_processes(ProcessesToUpdate::All);
        let processes = self
            .system
            .processes()
            .values()
            .map(|process| Process {
                pid: Pid(process.pid().as_u32()),
                parent_pid: process.parent().map(|parent_pid| Pid(parent_pid.as_u32())),
                usage: read_usage_of_process(process),
            })
            .collect();
        Ok(Services::ProcessTrees {
            jobs,
            processes,
            otelo_pid: Pid(self.otelo_pid.as_u32()),
        })
    }

    fn read_usage_of_this_process(&mut self) -> Option<ProcessUsage> {
        self.refresh_processes(ProcessesToUpdate::Some(&[self.otelo_pid]));
        self.system
            .process(self.otelo_pid)
            .map(read_usage_of_process)
    }

    fn refresh_processes(&mut self, processes_to_update: ProcessesToUpdate) {
        // On Linux a thread is a process of its own, and its memory is the memory of its process.
        let refresh_kind = ProcessRefreshKind::nothing()
            .with_cpu()
            .with_memory()
            .without_tasks();
        self.system
            .refresh_processes_specifics(processes_to_update, true, refresh_kind);
    }

    fn read_filesystems(&mut self) -> Vec<Filesystem> {
        self.disks.refresh(true);
        self.disks
            .list()
            .iter()
            .map(|disk| Filesystem {
                device: disk.name().to_string_lossy().into_owned(),
                mount_point: disk.mount_point().to_owned(),
                filesystem_type: disk.file_system().to_string_lossy().into_owned(),
                total_bytes: disk.total_space(),
                available_bytes: disk.available_space(),
            })
            .collect()
    }

    fn read_interfaces(&mut self) -> Vec<Interface> {
        self.networks.refresh(true);
        self.networks
            .list()
            .iter()
            .map(|(name, interface_data)| Interface {
                name: name.clone(),
                received_bytes: interface_data.total_received(),
                transmitted_bytes: interface_data.total_transmitted(),
            })
            .collect()
    }
}

fn read_usage_of_process(process: &sysinfo::Process) -> ProcessUsage {
    ProcessUsage {
        cpu_time: Duration::from_millis(process.accumulated_cpu_time()),
        resident_bytes: process.memory(),
    }
}
