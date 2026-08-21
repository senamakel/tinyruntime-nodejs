//! Unit tests for host interpreter detection.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use tinyruntime_bus::RuntimeSettings;

use super::{candidates, detect, locate, probe_version};

#[test]
fn a_preferred_command_is_tried_before_the_default() {
    let ordered = candidates(Some("/opt/node/bin/node"));
    assert_eq!(ordered[0], "/opt/node/bin/node");
    assert_eq!(ordered[1], "node");
}

#[test]
fn the_default_candidate_is_not_repeated() {
    assert_eq!(candidates(Some("node")), vec!["node".to_string()]);
    assert_eq!(candidates(None), vec!["node".to_string()]);
}

#[test]
fn an_absolute_command_that_is_not_there_does_not_resolve() {
    assert!(locate("/nonexistent/path/to/node").is_none());
}

#[cfg(unix)]
#[test]
fn a_bare_command_resolves_through_path() {
    // `sh` is on PATH on every Unix host, which makes this a real lookup rather
    // than one that depends on Node being installed.
    assert!(locate("sh").is_some(), "PATH lookup found nothing at all");
}

#[cfg(unix)]
#[tokio::test]
async fn a_binary_that_does_not_understand_the_flag_is_not_a_toolchain() {
    // `false --version` exits non-zero, which must read as "not usable" rather
    // than as an empty version string.
    if !Path::new("/bin/false").exists() {
        return;
    }
    assert!(probe_version(Path::new("/bin/false")).await.is_none());
}

#[tokio::test]
async fn a_binary_that_is_not_there_is_not_probed_successfully() {
    assert!(probe_version(Path::new("/nonexistent/node")).await.is_none());
}

#[tokio::test]
async fn an_unparseable_requested_version_detects_nothing() {
    // A misconfigured pin must not be answered with whatever is installed.
    let mut settings = RuntimeSettings::new("latest");
    settings.preferred_command = "node".to_string();
    assert!(detect(&settings).await.is_none());
}

#[tokio::test]
async fn a_preferred_command_that_is_not_there_falls_through_rather_than_failing() {
    let mut settings = RuntimeSettings::new("v22.11.0");
    settings.preferred_command = "/nonexistent/node".to_string();
    // Whether this finds a host `node` depends on the machine; what matters is
    // that a missing preferred command is not fatal on its own.
    let _ = detect(&settings).await;
}
