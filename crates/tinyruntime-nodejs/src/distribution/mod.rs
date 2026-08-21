//! Choosing which archive nodejs.org should be asked for.
//!
//! Two pieces of knowledge live here and nowhere else. First, the mapping from
//! this host's operating system and architecture to the archive name Node.js
//! publishes for it — a table that is only correct by matching what the release
//! actually ships. Second, that the digests live in a `SHASUMS256.txt` beside
//! the archives rather than in any per-asset metadata.
//!
//! Fetching that digest file is the one network call this provider makes, and it
//! is what lets the router verify the archive it downloads. A release whose
//! digest file does not list the archive is refused here rather than installed
//! unverified: nodejs.org always publishes one, so its absence means something is
//! wrong with what is being served.

use reqwest::Client;

use tinyruntime_bus::{ArchiveFormat, Distribution, RuntimeSettings};

use crate::error::{Error, Result};
use crate::version;

mod host;

pub use host::{HostArchive, host_archive};

/// Where Node.js publishes its releases.
const DIST_BASE: &str = "https://nodejs.org/dist";

/// Pick the archive to install for this host under `settings`.
///
/// # Errors
///
/// Returns [`Error::UnsupportedHost`] when Node.js publishes no build for this
/// machine, [`Error::InvalidVersion`] when the requested version is not one, and
/// [`Error::DigestUnavailable`] when the release's digest file cannot be read or
/// does not list the archive.
pub async fn select(client: &Client, settings: &RuntimeSettings) -> Result<Distribution> {
    if version::major(&settings.version).is_none() {
        return Err(Error::InvalidVersion(settings.version.clone()));
    }
    let version = version::canonical_version(&settings.version);
    let host = host_archive()?;

    let archive_name = format!("node-{version}-{}", host.suffix);
    let url = format!("{DIST_BASE}/{version}/{archive_name}");
    let digest = fetch_digest(client, &version, &archive_name).await?;

    tracing::info!(
        archive = %archive_name,
        "[tinyruntime-nodejs] selected a distribution for this host"
    );

    Ok(
        Distribution::new(version::bare_version(&version), &archive_name, url, host.format)
            .with_sha256(digest),
    )
}

/// Read `SHASUMS256.txt` for a release and return the digest for one archive.
async fn fetch_digest(client: &Client, version: &str, archive_name: &str) -> Result<String> {
    let url = format!("{DIST_BASE}/{version}/SHASUMS256.txt");
    let body = client
        .get(&url)
        .header(
            reqwest::header::USER_AGENT,
            concat!("tinyruntime-nodejs/", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| Error::DigestUnavailable {
            version: version.to_owned(),
            reason: describe(&error),
        })?
        .text()
        .await
        .map_err(|error| Error::DigestUnavailable {
            version: version.to_owned(),
            reason: describe(&error),
        })?;

    digest_for(&body, archive_name).ok_or_else(|| Error::DigestUnavailable {
        version: version.to_owned(),
        reason: format!("the release does not list `{archive_name}`"),
    })
}

/// Find one archive's digest in a `SHASUMS256.txt` body.
///
/// The format is `<hex>  <filename>` per line. Matching on the whole filename
/// rather than a suffix matters: `node-v22.11.0-linux-x64.tar.gz` and
/// `node-v22.11.0-linux-x64.tar.xz` differ only at the end, and a loose match
/// would hand the router the wrong digest for an archive it downloaded fine.
#[must_use]
pub fn digest_for(shasums: &str, archive_name: &str) -> Option<String> {
    shasums.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let digest = parts.next()?;
        let name = parts.next()?;
        (name == archive_name && digest.len() == 64).then(|| digest.to_owned())
    })
}

/// Describe a request failure without putting the URL in a host-visible message.
fn describe(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "the request timed out".to_owned()
    } else if error.is_connect() {
        "the connection could not be established".to_owned()
    } else if let Some(status) = error.status() {
        format!("nodejs.org answered with status {status}")
    } else {
        "the request failed".to_owned()
    }
}

/// The format this host's archives come in, exposed for the layout tests.
#[must_use]
pub fn host_format() -> Option<ArchiveFormat> {
    host_archive().ok().map(|host| host.format)
}

#[cfg(test)]
mod test;
