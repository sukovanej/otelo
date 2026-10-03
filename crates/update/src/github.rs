use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use semver::Version;
use serde::Deserialize;

use crate::digest::verify_sha256_digest;

const GITHUB_API_BASE_URL: &str = "https://api.github.com";
const DOWNLOAD_TIMEOUT: Duration = Duration::from_mins(5);
const MAX_ASSET_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Channel {
    Stable,
    Canary,
}

#[derive(Debug, PartialEq, Eq)]
pub enum UpdateStep {
    UpToDate,
    Install,
    ReturnToStable,
}

pub struct GithubReleases {
    api_base_url: String,
    repository: String,
    agent: ureq::Agent,
}

pub struct Release {
    pub version: Version,
    pub tag: String,
    pub channel: Channel,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
pub struct Asset {
    pub name: String,
    #[serde(rename = "browser_download_url")]
    pub download_url: String,
}

#[derive(Deserialize)]
struct ReleaseResponse {
    tag_name: String,
    name: Option<String>,
    assets: Vec<Asset>,
}

impl GithubReleases {
    #[must_use]
    pub fn of_otelo() -> Self {
        Self::at(
            GITHUB_API_BASE_URL,
            extract_repository_slug(env!("CARGO_PKG_REPOSITORY")),
        )
    }

    #[must_use]
    pub fn at(api_base_url: &str, repository: &str) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(DOWNLOAD_TIMEOUT))
            .user_agent(format!("otelo/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        Self {
            api_base_url: api_base_url.trim_end_matches('/').to_owned(),
            repository: repository.to_owned(),
            agent,
        }
    }

    pub fn fetch_release(&self, channel: Channel) -> Result<Release> {
        let path = match channel {
            Channel::Stable => "releases/latest",
            Channel::Canary => "releases/tags/canary",
        };
        let url = format!("{}/repos/{}/{path}", self.api_base_url, self.repository);
        let response: ReleaseResponse = self
            .agent
            .get(&url)
            .header("Accept", "application/vnd.github+json")
            .call()
            .with_context(|| format!("reading {url}"))?
            .body_mut()
            .read_json()
            .with_context(|| format!("parsing the release at {url}"))?;
        // The tag `canary` moves with each build, so only the release title holds its version.
        let version = match channel {
            Channel::Stable => Version::parse(response.tag_name.trim_start_matches('v'))
                .with_context(|| {
                    format!("the release tag {:?} is not a version", response.tag_name)
                })?,
            Channel::Canary => {
                let title = response.name.unwrap_or_default();
                Version::parse(&title).with_context(|| {
                    format!("the canary release title {title:?} is not a version")
                })?
            }
        };
        Ok(Release {
            version,
            tag: response.tag_name,
            channel,
            assets: response.assets,
        })
    }

    pub fn download_verified_asset(&self, release: &Release, name: &str) -> Result<Vec<u8>> {
        let asset = release.find_asset(name)?;
        let digest_asset = release.find_asset(&format!("{name}.sha256"))?;
        let published_digest_file =
            String::from_utf8(self.download(&digest_asset.download_url)?)
                .with_context(|| format!("{} is not text", digest_asset.name))?;
        let bytes = self.download(&asset.download_url)?;
        verify_sha256_digest(&bytes, &published_digest_file)
            .with_context(|| format!("verifying {name}"))?;
        Ok(bytes)
    }

    fn download(&self, url: &str) -> Result<Vec<u8>> {
        self.agent
            .get(url)
            .call()
            .with_context(|| format!("downloading {url}"))?
            .body_mut()
            .with_config()
            .limit(MAX_ASSET_BYTES)
            .read_to_vec()
            .with_context(|| format!("reading {url}"))
    }
}

impl Release {
    #[must_use]
    pub fn choose_step_from(&self, installed: &Version) -> UpdateStep {
        match self.channel {
            Channel::Canary if self.version == *installed => UpdateStep::UpToDate,
            Channel::Canary => UpdateStep::Install,
            Channel::Stable if self.version > *installed => UpdateStep::Install,
            Channel::Stable if self.version < *installed && !installed.pre.is_empty() => {
                UpdateStep::ReturnToStable
            }
            Channel::Stable => UpdateStep::UpToDate,
        }
    }

    pub fn find_asset(&self, name: &str) -> Result<&Asset> {
        match self.assets.iter().find(|asset| asset.name == name) {
            Some(asset) => Ok(asset),
            None => bail!("release {} publishes no asset named {name}", self.tag),
        }
    }
}

fn extract_repository_slug(repository_url: &str) -> &str {
    let trimmed = repository_url
        .trim_end_matches('/')
        .trim_end_matches(".git");
    trimmed
        .split_once("github.com/")
        .map_or(trimmed, |(_, slug)| slug)
}
