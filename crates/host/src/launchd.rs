use std::process::Command;

use anyhow::{Context, ensure};

use crate::snapshot::{LaunchdJob, Pid};

// The jobs of macOS itself and the apps a user opened would be hundreds of services nobody
// deployed.
const LABEL_PREFIXES_LEFT_OUT: [&str; 2] = ["com.apple.", "application."];

pub fn list_running_jobs() -> anyhow::Result<Vec<LaunchdJob>> {
    let output = Command::new("launchctl")
        .arg("list")
        .output()
        .context("run launchctl list")?;
    ensure!(
        output.status.success(),
        "launchctl list ended with {}",
        output.status
    );
    Ok(find_running_jobs_in_launchctl_list(
        &String::from_utf8_lossy(&output.stdout),
    ))
}

// A line is the PID or `-`, the last exit status, and the label, with tabs between them.
#[must_use]
pub fn find_running_jobs_in_launchctl_list(output: &str) -> Vec<LaunchdJob> {
    output
        .lines()
        .filter_map(|line| {
            let mut columns = line.split('\t');
            let pid = columns.next()?.trim().parse().ok()?;
            let label = columns.nth(1)?.trim();
            let is_left_out = LABEL_PREFIXES_LEFT_OUT
                .iter()
                .any(|prefix| label.starts_with(prefix));
            (!label.is_empty() && !is_left_out).then(|| LaunchdJob {
                label: label.to_owned(),
                main_pid: Pid(pid),
            })
        })
        .collect()
}
