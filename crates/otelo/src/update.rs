use anyhow::{Context as _, bail};
use otelo_update::{Channel, Environment, GithubReleases, UpdateStep, find_owner_of_executable};
use semver::Version;

#[derive(clap::Args)]
pub struct UpdateArgs {
    /// Install the newest build of main instead of the newest release
    #[arg(long)]
    canary: bool,
}

pub fn update_executable(args: &UpdateArgs) -> anyhow::Result<()> {
    let channel = if args.canary {
        Channel::Canary
    } else {
        Channel::Stable
    };
    let executable = std::env::current_exe()
        .and_then(|executable| executable.canonicalize())
        .context("locating this executable")?;
    if let Some(owner) = find_owner_of_executable(&executable, &Environment::detect()) {
        bail!("{}", owner.describe_refusal(&executable));
    }
    let target = otelo_update::detect_release_target().with_context(|| {
        format!(
            "no release is built for {}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })?;
    let installed =
        Version::parse(env!("CARGO_PKG_VERSION")).context("parsing the built-in version")?;

    let github = GithubReleases::of_otelo();
    let release = github.fetch_release(channel).context(match channel {
        Channel::Stable => "looking up the newest release",
        Channel::Canary => "looking up the canary build",
    })?;
    match release.choose_step_from(&installed) {
        UpdateStep::UpToDate => {
            match channel {
                Channel::Stable => println!("otelo {installed} is the newest release"),
                Channel::Canary => println!("otelo {installed} is the newest canary build"),
            }
            return Ok(());
        }
        UpdateStep::Install => println!("otelo {installed} -> {}", release.version),
        UpdateStep::ReturnToStable => println!(
            "otelo {installed} -> {}: from the canary build back to the newest release",
            release.version
        ),
    }

    let archive_name = otelo_update::build_archive_name(target);
    println!("downloading {archive_name}");
    let archive = github.download_verified_asset(&release, &archive_name)?;
    println!("verified {archive_name}");
    otelo_update::replace_executable_from_archive(&archive, &executable)?;
    println!("updated {} to {}", executable.display(), release.version);
    println!("a running daemon keeps its version until it restarts");
    Ok(())
}
