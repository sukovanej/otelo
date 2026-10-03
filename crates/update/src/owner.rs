use std::path::{Path, PathBuf};

pub struct Environment {
    pub cargo_home: Option<PathBuf>,
    pub homebrew_prefix: Option<PathBuf>,
}

impl Environment {
    // The executable path arrives canonical, so the prefixes must be canonical too: on macOS
    // `/var` is a link to `/private/var`.
    #[must_use]
    pub fn detect() -> Self {
        let cargo_home = std::env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")));
        Self {
            cargo_home: cargo_home.map(canonicalize_or_keep),
            homebrew_prefix: std::env::var_os("HOMEBREW_PREFIX")
                .map(PathBuf::from)
                .map(canonicalize_or_keep),
        }
    }
}

fn canonicalize_or_keep(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Owner {
    Cargo,
    Homebrew,
    Nix,
    SystemPackage,
    BuildCache,
}

impl Owner {
    #[must_use]
    pub fn describe_refusal(self, executable: &Path) -> String {
        let executable = executable.display();
        match self {
            Self::Cargo => format!(
                "cargo install owns {executable}; install otelo again with cargo to update it"
            ),
            Self::Homebrew => {
                format!("Homebrew owns {executable}; run `brew upgrade otelo` to update it")
            }
            Self::Nix => {
                format!("Nix owns {executable}; update it through your Nix configuration")
            }
            Self::SystemPackage => {
                format!(
                    "a system package owns {executable}; update it through your package manager"
                )
            }
            Self::BuildCache => {
                format!("{executable} is a build in a cache directory; build it again to update it")
            }
        }
    }
}

#[must_use]
pub fn find_owner_of_executable(executable: &Path, environment: &Environment) -> Option<Owner> {
    let is_under = |prefix: &Path| executable.starts_with(prefix);
    if environment
        .cargo_home
        .as_deref()
        .is_some_and(|cargo_home| is_under(&cargo_home.join("bin")))
    {
        return Some(Owner::Cargo);
    }
    if environment.homebrew_prefix.as_deref().is_some_and(is_under)
        || is_under(Path::new("/opt/homebrew"))
        || is_under(Path::new("/home/linuxbrew/.linuxbrew"))
        || executable
            .components()
            .any(|component| component.as_os_str() == "Cellar")
    {
        return Some(Owner::Homebrew);
    }
    if is_under(Path::new("/nix/store")) {
        return Some(Owner::Nix);
    }
    if is_under(Path::new("/usr")) && !is_under(Path::new("/usr/local")) {
        return Some(Owner::SystemPackage);
    }
    // Cargo marks its target directory with this tag, so it catches `cargo build` and `cargo run`.
    if executable
        .ancestors()
        .skip(1)
        .any(|directory| directory.join("CACHEDIR.TAG").is_file())
    {
        return Some(Owner::BuildCache);
    }
    None
}
