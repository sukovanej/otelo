use std::fs;
use std::process::Command;

use otelo_storage::{Attributes, Resource};

// OpenTelemetry names these files and this command as the sources of `host.id`.
const MACHINE_ID_PATHS: [&str; 2] = ["/etc/machine-id", "/var/lib/dbus/machine-id"];
const IOREG_PATH: &str = "/usr/sbin/ioreg";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostIdentity {
    pub name: Option<String>,
    pub id: Option<String>,
    pub arch: &'static str,
    pub os_type: &'static str,
}

impl HostIdentity {
    #[must_use]
    pub fn read_from_this_machine() -> Self {
        Self {
            name: sysinfo::System::host_name(),
            id: if cfg!(target_os = "macos") {
                read_platform_uuid()
            } else {
                read_machine_id()
            },
            arch: match std::env::consts::ARCH {
                "x86_64" => "amd64",
                "aarch64" => "arm64",
                "arm" => "arm32",
                arch => arch,
            },
            os_type: match std::env::consts::OS {
                "macos" => "darwin",
                os => os,
            },
        }
    }

    #[must_use]
    pub fn attributes(&self) -> Vec<(&'static str, String)> {
        [
            ("host.name", self.name.clone()),
            ("host.id", self.id.clone()),
            ("host.arch", Some(self.arch.to_owned())),
            ("os.type", Some(self.os_type.to_owned())),
        ]
        .into_iter()
        .filter_map(|(key, value)| Some((key, value?)))
        .collect()
    }

    #[must_use]
    pub fn resource_of_service(&self, service: &str) -> Resource {
        let mut attributes = Attributes::new();
        attributes.insert("service.name", service);
        for (key, value) in self.attributes() {
            attributes.insert(key, value);
        }
        Resource {
            service: service.into(),
            attributes,
        }
    }
}

fn read_machine_id() -> Option<String> {
    MACHINE_ID_PATHS
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .map(|machine_id| machine_id.trim().to_owned())
        .find(|machine_id| !machine_id.is_empty())
}

fn read_platform_uuid() -> Option<String> {
    let output = Command::new(IOREG_PATH)
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .ok()?;
    find_platform_uuid_in_ioreg_output(&String::from_utf8_lossy(&output.stdout))
}

#[must_use]
pub fn find_platform_uuid_in_ioreg_output(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        key.contains("\"IOPlatformUUID\"")
            .then(|| value.trim().trim_matches('"').to_owned())
            .filter(|uuid| !uuid.is_empty())
    })
}
