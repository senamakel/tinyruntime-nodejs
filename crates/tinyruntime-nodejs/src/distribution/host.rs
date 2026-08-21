//! Which archive Node.js publishes for this machine.
//!
//! A plain table, because it is only correct by matching what the release
//! actually ships — there is no rule to derive it from. An entry missing here is
//! a host Node.js has no prebuilt binary for, and saying so is better than
//! guessing a filename that will 404 halfway through a download.

use tinyruntime_bus::ArchiveFormat;

use crate::error::{Error, Result};

/// The archive naming and format for one host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostArchive {
    /// Everything after `node-vX.Y.Z-`, e.g. `linux-x64.tar.xz`.
    pub suffix: &'static str,
    /// How the archive is packed.
    pub format: ArchiveFormat,
}

/// The archive for the machine this is running on.
///
/// # Errors
///
/// Returns [`Error::UnsupportedHost`] when Node.js publishes no prebuilt binary
/// for this operating system and architecture.
pub fn host_archive() -> Result<HostArchive> {
    archive_for(std::env::consts::OS, std::env::consts::ARCH)
}

/// The archive for a named operating system and architecture.
///
/// Split from [`host_archive`] so the table can be tested for every platform
/// rather than only for whichever one the tests happen to run on.
///
/// # Errors
///
/// Returns [`Error::UnsupportedHost`] for a combination Node.js does not build.
pub fn archive_for(os: &str, arch: &str) -> Result<HostArchive> {
    let (suffix, format) = match (os, arch) {
        ("macos", "aarch64") => ("darwin-arm64.tar.xz", ArchiveFormat::TarXz),
        ("macos", "x86_64") => ("darwin-x64.tar.xz", ArchiveFormat::TarXz),
        ("linux", "aarch64") => ("linux-arm64.tar.xz", ArchiveFormat::TarXz),
        ("linux", "x86_64") => ("linux-x64.tar.xz", ArchiveFormat::TarXz),
        ("linux", "arm" | "armv7") => ("linux-armv7l.tar.xz", ArchiveFormat::TarXz),
        ("linux", "powerpc64") => ("linux-ppc64le.tar.xz", ArchiveFormat::TarXz),
        ("linux", "s390x") => ("linux-s390x.tar.xz", ArchiveFormat::TarXz),
        // Windows ships zips, and with no `bin/` directory inside them.
        ("windows", "aarch64") => ("win-arm64.zip", ArchiveFormat::Zip),
        ("windows", "x86_64") => ("win-x64.zip", ArchiveFormat::Zip),
        _ => {
            return Err(Error::UnsupportedHost {
                os: os.to_owned(),
                arch: arch.to_owned(),
            });
        }
    };
    Ok(HostArchive { suffix, format })
}
