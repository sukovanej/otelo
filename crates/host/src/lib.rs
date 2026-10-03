mod identity;
mod launchd;
mod linux;
mod mapping;
mod reader;
mod snapshot;

use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use otelo_indexed_storage::StorageSize;

pub use identity::{HostIdentity, find_platform_uuid_in_ioreg_output};
pub use launchd::find_running_jobs_in_launchctl_list;
pub use linux::LinuxFiles;
pub use mapping::SnapshotMapper;
pub use reader::MachineReader;
pub use snapshot::{
    CgroupMemory, Cpu, CpuTicks, Filesystem, Interface, LaunchdJob, LoadAverage, Memory, Pid,
    Process, ProcessUsage, Services, Snapshot, Swap, Unit,
};

pub struct Collector {
    machine_reader: MachineReader,
    snapshot_mapper: SnapshotMapper,
}

impl Collector {
    pub fn new(host: HostIdentity) -> anyhow::Result<Self> {
        Ok(Self {
            machine_reader: MachineReader::new()?,
            snapshot_mapper: SnapshotMapper::new(host),
        })
    }

    // Reads files and runs commands, so an async caller has to give it a thread that may block.
    pub fn collect_request(
        &mut self,
        recorded_at: i64,
        storage_size: Option<StorageSize>,
    ) -> anyhow::Result<ExportMetricsServiceRequest> {
        let snapshot = self.machine_reader.read_snapshot()?;
        Ok(self
            .snapshot_mapper
            .map_snapshot_to_request(recorded_at, &snapshot, storage_size))
    }
}
