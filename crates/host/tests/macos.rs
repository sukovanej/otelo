use otelo_host::{
    LaunchdJob, Pid, find_platform_uuid_in_ioreg_output, find_running_jobs_in_launchctl_list,
};

// The start of what `ioreg -rd1 -c IOPlatformExpertDevice` prints, with another UUID and serial.
const IOREG_OUTPUT: &str = r##"+-o J713AP  <class IOPlatformExpertDevice, id 0x100000276, registered, matched, active, busy 0 (36247 ms), retain 36>
    {
      "IOPolledInterface" = "AppleARMWatchdogTimerHibernateHandler is not serializable"
      "#address-cells" = <02000000>
      "manufacturer" = <"Apple Inc.">
      "compatible" = <"J713AP","Mac16,12","AppleARM">
      "IOPlatformSerialNumber" = "C02XXXXXXXXX"
      "model" = <"Mac16,12">
      "IOPlatformUUID" = "564D3A1B-0C2E-4F7A-9B1D-2E8A6F3C9D10"
      "device_type" = <"bootrom">
    }
"##;

// In the format of `launchctl list`: tabs between the PID, the last exit status, and the label.
const LAUNCHCTL_LIST: &str = "PID\tStatus\tLabel
-\t0\tcom.apple.SafariHistoryServiceAgent
601\t0\tcom.apple.trustd.agent
-\t78\tcom.example.backup
4321\t0\tcom.example.mudro
987\t-9\thomebrew.mxcl.postgresql@16
65382\t0\tapplication.com.google.Chrome.126963.55014095
";

#[test]
fn finds_the_platform_uuid_in_the_output_of_ioreg() {
    assert_eq!(
        find_platform_uuid_in_ioreg_output(IOREG_OUTPUT).as_deref(),
        Some("564D3A1B-0C2E-4F7A-9B1D-2E8A6F3C9D10")
    );
    assert_eq!(find_platform_uuid_in_ioreg_output("no device"), None);
}

#[test]
fn keeps_the_running_jobs_that_are_not_part_of_macos_or_an_app() {
    assert_eq!(
        find_running_jobs_in_launchctl_list(LAUNCHCTL_LIST),
        [
            LaunchdJob {
                label: "com.example.mudro".into(),
                main_pid: Pid(4321),
            },
            LaunchdJob {
                label: "homebrew.mxcl.postgresql@16".into(),
                main_pid: Pid(987),
            },
        ]
    );
}
