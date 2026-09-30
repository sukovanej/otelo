mod identity;
mod launchd;
mod linux;
mod mapping;
mod reader;
mod snapshot;

use otelo_storage::{Batch, StorageSize};

pub use identity::{HostIdentity, find_platform_uuid_in_ioreg_output};
pub use launchd::find_running_jobs_in_launchctl_list;
pub use linux::LinuxFiles;
pub use mapping::Mapping;
pub use reader::MachineReader;
pub use snapshot::{
    CgroupMemory, Cpu, CpuTicks, Filesystem, Interface, LaunchdJob, LoadAverage, Memory, Pid,
    Process, ProcessUsage, Services, Snapshot, Swap, Unit,
};

pub struct Collector {
    reader: MachineReader,
    mapping: Mapping,
}

impl Collector {
    pub fn of_host(host: HostIdentity) -> anyhow::Result<Self> {
        Ok(Self {
            reader: MachineReader::new()?,
            mapping: Mapping::new(host),
        })
    }

    // Reads files and runs commands, so an async caller has to give it a thread that may block.
    pub fn collect_batch(
        &mut self,
        recorded_at: i64,
        storage_size: Option<StorageSize>,
    ) -> anyhow::Result<Batch> {
        let snapshot = self.reader.read_snapshot()?;
        Ok(self
            .mapping
            .map_snapshot_to_batch(recorded_at, &snapshot, storage_size))
    }
}
