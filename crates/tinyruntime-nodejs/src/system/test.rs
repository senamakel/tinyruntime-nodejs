//! Unit tests for host interpreter detection.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use tinyruntime_bus::RuntimeSettings;

use super::{candidates, detect, detect_in, locate, probe_version};

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
    assert!(locate("/nonexistent/path/to/node", None).is_none());
}

#[test]
fn a_bare_command_with_no_search_path_does_not_resolve() {
    assert!(locate("node", None).is_none());
}

#[cfg(unix)]
#[test]
fn a_bare_command_resolves_through_the_search_path() {
    let path = std::env::var_os("PATH").expect("a host has a PATH");
    assert!(
        locate("sh", Some(&path)).is_some(),
        "PATH lookup found nothing at all"
    );
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
    assert!(
        probe_version(Path::new("/nonexistent/node"))
            .await
            .is_none()
    );
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

/// Write an executable standing in for `node`, printing `version`.
#[cfg(unix)]
fn fake_node(directory: &std::path::Path, name: &str, version: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = directory.join(name);
    std::fs::write(&path, format!("#!/bin/sh\necho '{version}'\n")).expect("the script writes");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("the script is executable");
    path
}

#[cfg(unix)]
#[tokio::test]
async fn a_matching_interpreter_on_the_search_path_is_reused() {
    // The step that keeps most machines from downloading anything, exercised
    // against an interpreter the test controls rather than whatever is installed.
    let scratch = tempfile::tempdir().expect("scratch directory");
    fake_node(scratch.path(), "node", "v22.8.0");

    let path = std::ffi::OsString::from(scratch.path());
    let layout = detect_in(&RuntimeSettings::new("v22.11.0"), Some(&path))
        .await
        .expect("a compatible interpreter is reused");

    assert_eq!(layout.version, "22.8.0", "the leading v should be stripped");
    assert_eq!(layout.bin_dir, scratch.path().to_string_lossy());
    assert!(layout.executable("node").is_some());
}

#[cfg(unix)]
#[tokio::test]
async fn an_interpreter_on_another_major_line_is_not_reused() {
    let scratch = tempfile::tempdir().expect("scratch directory");
    fake_node(scratch.path(), "node", "v20.11.0");

    let path = std::ffi::OsString::from(scratch.path());
    assert!(
        detect_in(&RuntimeSettings::new("v22.11.0"), Some(&path))
            .await
            .is_none(),
        "a different major line was accepted"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_preferred_command_is_used_before_the_default_one() {
    // On a machine with several interpreters, the caller's choice must win.
    let scratch = tempfile::tempdir().expect("scratch directory");
    fake_node(scratch.path(), "node", "v20.0.0");
    let preferred = fake_node(scratch.path(), "node-preferred", "v22.9.0");

    let mut settings = RuntimeSettings::new("v22.11.0");
    settings.preferred_command = preferred.to_string_lossy().into_owned();

    let layout = detect_in(&settings, Some(&std::ffi::OsString::from(scratch.path())))
        .await
        .expect("the preferred interpreter is used");
    assert_eq!(layout.version, "22.9.0");
}

#[cfg(unix)]
#[tokio::test]
async fn an_interpreter_that_prints_nothing_useful_is_skipped() {
    let scratch = tempfile::tempdir().expect("scratch directory");
    fake_node(scratch.path(), "node", "not-a-version");

    let path = std::ffi::OsString::from(scratch.path());
    assert!(
        detect_in(&RuntimeSettings::new("v22.11.0"), Some(&path))
            .await
            .is_none()
    );
}
