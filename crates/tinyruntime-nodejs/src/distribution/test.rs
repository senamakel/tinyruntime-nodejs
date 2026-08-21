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

// ---------------------------------------------------------------------------
// Against a release index the test serves
//
// Reaching nodejs.org here would tie the suite to the network and to a release
// staying published. A loopback server gives the same code path with neither.
// ---------------------------------------------------------------------------

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

use reqwest::Client;
use tinyruntime_bus::RuntimeSettings;

/// Serve `SHASUMS256.txt` for one release, then stop.
///
/// Returns the base URL and the server thread.
fn serve_shasums(body: String) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback is available");
    let base = format!("http://{}", listener.local_addr().expect("an address"));

    let handle = std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let Ok(clone) = stream.try_clone() else {
            return;
        };
        let mut reader = BufReader::new(clone);
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap_or(0) > 0 {
            if line == "\r\n" {
                break;
            }
            line.clear();
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    });

    (base, handle)
}

/// The archive name this host would ask for at `version`.
fn expected_archive(version: &str) -> String {
    format!(
        "node-{version}-{}",
        host_archive().expect("this host is supported").suffix
    )
}

#[tokio::test]
async fn a_distribution_carries_the_digest_the_release_published() {
    let version = "v22.11.0";
    let archive = expected_archive(version);
    let digest = "ab".repeat(32);
    let (base, server) = serve_shasums(format!("{digest}  {archive}\n"));

    let chosen = super::select_from(&Client::new(), &base, &RuntimeSettings::new(version))
        .await
        .expect("the distribution is selected");

    assert_eq!(chosen.version, "22.11.0", "the leading v is stripped");
    assert_eq!(chosen.archive_name, archive);
    assert_eq!(chosen.expected_sha256.as_deref(), Some(digest.as_str()));
    assert!(chosen.url.ends_with(&archive), "got {}", chosen.url);
    assert_eq!(chosen.format, host_archive().unwrap().format);
    server.join().expect("the server finished");
}

#[tokio::test]
async fn a_release_that_does_not_list_this_hosts_archive_is_refused() {
    // Installing unverified is not an option: nodejs.org always publishes a
    // digest, so its absence means something is wrong with what is being served.
    let (base, server) = serve_shasums("cd".repeat(32) + "  node-v22.11.0-some-other-host.tar.xz\n");

    let error = super::select_from(&Client::new(), &base, &RuntimeSettings::new("v22.11.0"))
        .await
        .expect_err("an unlisted archive is refused");

    assert!(matches!(error, Error::DigestUnavailable { .. }), "got {error:?}");
    assert!(error.to_string().contains("22.11.0"), "got `{error}`");
    server.join().expect("the server finished");
}

#[tokio::test]
async fn a_version_that_is_not_one_is_refused_before_any_request() {
    // The server is never started, so reaching it would hang rather than fail.
    let error = super::select_from(&Client::new(), "http://127.0.0.1:1", &RuntimeSettings::new("latest"))
        .await
        .expect_err("`latest` is not a version");
    assert!(matches!(error, Error::InvalidVersion(_)), "got {error:?}");
}

#[tokio::test]
async fn an_unreachable_release_index_is_reported_without_the_url() {
    // These messages reach a host's UI; a URL can carry a token.
    let error = super::select_from(
        &Client::new(),
        "http://127.0.0.1:1",
        &RuntimeSettings::new("v22.11.0"),
    )
    .await
    .expect_err("an unreachable index fails");

    let Error::DigestUnavailable { reason, .. } = &error else {
        panic!("got {error:?}");
    };
    assert!(!reason.contains("127.0.0.1"), "got `{reason}`");
    assert!(reason.contains("connection"), "got `{reason}`");
}

#[test]
fn the_host_format_is_reported_for_this_machine() {
    assert_eq!(super::host_format(), host_archive().ok().map(|host| host.format));
}
