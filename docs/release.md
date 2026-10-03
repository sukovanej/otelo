---
created: 2026-10-03T19:13:23Z
---
# Release

otelo ships as one binary per target on GitHub Releases: macOS and Linux, on aarch64 and x86_64. [[./platforms.md]] says where each part runs.

## Install and update

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/sukovanej/otelo/releases/latest/download/otelo-installer.sh | sh
```

The installer puts `otelo` in `~/.local/bin`.

- `otelo update` replaces the running executable with the newest release.
- `otelo update --canary` replaces it with the newest build of `main`.
- `otelo update` on a canary build goes back to the newest release, even when that release is older.

The update reads the release from the GitHub API, downloads `otelo-<target>.tar.gz`, checks it against the `.sha256` file next to it, and renames the new binary over the old one. A daemon keeps running the old binary until its systemd unit or launchd job restarts it. The update refuses a binary that cargo, Homebrew, Nix, or a system package installed, and a build under `target/`. The `otelo-update` crate holds this code.

## Versions

The version is `version` in `[workspace.package]` of `Cargo.toml`. Every crate takes it, and `otelo --version` prints it.

A canary build of `0.4.0` is `0.4.1-canary.57`: the next patch, then the run number of the workflow. SemVer orders it after `0.4.0` and before `0.4.1`.

## Changelog

`CHANGELOG.md` follows [Keep a Changelog](https://keepachangelog.com). Write each section by hand. cargo-dist takes the notes of the GitHub release from the section of the version, so the release page shows that section.

```md
## [0.0.2](https://github.com/sukovanej/otelo/compare/v0.0.1...v0.0.2) - 2026-10-10

### Added

- What a user reads.
```

## Release

On a branch, after the changelog section:

```sh
mise run release 0.0.2
```

The task stops when `CHANGELOG.md` has no `## [0.0.2]` section. It sets the version, updates `Cargo.lock`, and commits. Merge the commit into `main`, then tag it:

```sh
git tag v0.0.2 && git push origin v0.0.2
```

The tag starts `.github/workflows/release.yml`. It builds the UI and the binary on each target, then publishes the archives, their digests, and `otelo-installer.sh` as a GitHub release.

[cargo-dist](https://axodotdev.github.io/cargo-dist/) generates `release.yml` from `[workspace.metadata.dist]` in `Cargo.toml` and `.github/build-setup.yml`. After a change to either, run `mise x -- dist generate` and commit the result. `mise` installs the cargo-dist version that the metadata names.

## Canary

Each push to `main` starts `.github/workflows/canary.yml`. It builds the same archives with the canary version and replaces the prerelease `canary`, whose title is the version. `otelo update --canary` reads that title. A canary has no installer and no changelog section.
