//! Unit tests for distribution selection.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use tinyruntime_bus::ArchiveFormat;

use super::{archive_for, digest_for, host_archive};
use crate::error::Error;

/// A realistic `SHASUMS256.txt` body, including the near-miss neighbours that
/// make loose matching dangerous.
const SHASUMS: &str = "\
4c2a1b3d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f809  node-v22.11.0-linux-x64.tar.gz
aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899  node-v22.11.0-linux-x64.tar.xz
99887766554433221100ffeeddccbbaa99887766554433221100ffeeddccbbaa  node-v22.11.0-darwin-arm64.tar.xz
";

#[test]
fn the_digest_is_matched_on_the_whole_filename() {
    // `.tar.gz` and `.tar.xz` differ only at the end. A suffix match would hand
    // the router the wrong digest for an archive that downloaded perfectly.
    assert_eq!(
        digest_for(SHASUMS, "node-v22.11.0-linux-x64.tar.xz").as_deref(),
        Some("aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899")
    );
    assert_eq!(
        digest_for(SHASUMS, "node-v22.11.0-linux-x64.tar.gz").as_deref(),
        Some("4c2a1b3d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f809")
    );
}

#[test]
fn an_archive_the_release_does_not_list_has_no_digest() {
    assert!(digest_for(SHASUMS, "node-v22.11.0-win-x64.zip").is_none());
}

#[test]
fn a_malformed_digest_line_is_not_accepted() {
    // A truncated digest would be compared against and never match, producing a
    // confusing "the archive is corrupt" for a file that is fine.
    let truncated = "abc123  node-v22.11.0-linux-x64.tar.xz\n";
    assert!(digest_for(truncated, "node-v22.11.0-linux-x64.tar.xz").is_none());
    assert!(digest_for("garbage-with-no-filename\n", "anything").is_none());
    assert!(digest_for("", "anything").is_none());
}

#[test]
fn every_platform_node_builds_for_is_in_the_table() {
    for (os, arch, suffix, format) in [
        (
            "macos",
            "aarch64",
            "darwin-arm64.tar.xz",
            ArchiveFormat::TarXz,
        ),
        ("macos", "x86_64", "darwin-x64.tar.xz", ArchiveFormat::TarXz),
        (
            "linux",
            "aarch64",
            "linux-arm64.tar.xz",
            ArchiveFormat::TarXz,
        ),
        ("linux", "x86_64", "linux-x64.tar.xz", ArchiveFormat::TarXz),
        ("windows", "x86_64", "win-x64.zip", ArchiveFormat::Zip),
        ("windows", "aarch64", "win-arm64.zip", ArchiveFormat::Zip),
    ] {
        let entry =
            archive_for(os, arch).unwrap_or_else(|_| panic!("{os}/{arch} is not in the table"));
        assert_eq!(entry.suffix, suffix);
        assert_eq!(entry.format, format);
    }
}

#[test]
fn windows_archives_are_zips_and_everything_else_is_a_tarball() {
    // The format decides which extractor the router uses, so a wrong entry here
    // fails after the download rather than before it.
    assert_eq!(
        archive_for("windows", "x86_64").unwrap().format,
        ArchiveFormat::Zip
    );
    assert_eq!(
        archive_for("linux", "x86_64").unwrap().format,
        ArchiveFormat::TarXz
    );
    assert_eq!(
        archive_for("macos", "aarch64").unwrap().format,
        ArchiveFormat::TarXz
    );
}

#[test]
fn a_host_node_does_not_build_for_is_refused_by_name() {
    let error = archive_for("freebsd", "x86_64").expect_err("no prebuilt binary exists");
    let rendered = error.to_string();
    assert!(rendered.contains("freebsd"), "got `{rendered}`");
    assert!(matches!(error, Error::UnsupportedHost { .. }));
}

#[test]
fn this_machine_is_one_node_builds_for() {
    // If this fails, the test suite is running somewhere the module could never
    // install a managed toolchain — worth knowing loudly.
    assert!(
        host_archive().is_ok(),
        "no archive for {}",
        std::env::consts::ARCH
    );
}
