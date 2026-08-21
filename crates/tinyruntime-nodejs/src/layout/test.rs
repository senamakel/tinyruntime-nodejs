//! Unit tests for the Node.js install layout.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;

use super::{bin_dir, executable_name, from_bin_dir};

/// Build the platform's bin directory with the named tools in it.
fn fabricate(root: &Path, tools: &[&str]) -> std::path::PathBuf {
    let bin = bin_dir(root);
    fs::create_dir_all(&bin).unwrap();
    for tool in tools {
        fs::write(bin.join(executable_name(tool)), b"").unwrap();
    }
    bin
}

#[cfg(unix)]
#[test]
fn unix_keeps_its_tools_under_bin() {
    let root = Path::new("/cache/node-v22.11.0");
    assert_eq!(bin_dir(root), root.join("bin"));
    assert_eq!(executable_name("node"), "node");
    assert_eq!(executable_name("npm"), "npm");
}

#[cfg(windows)]
#[test]
fn windows_keeps_its_tools_at_the_install_root() {
    // The official Windows zip ships no `bin/` directory at all.
    let root = Path::new("C:/cache/node-v22.11.0");
    assert_eq!(bin_dir(root), root);
    assert_eq!(executable_name("node"), "node.exe");
    assert_eq!(
        executable_name("npm"),
        "npm.cmd",
        "npm must be reached through its launcher, not its script"
    );
}

#[test]
fn a_layout_records_every_tool_that_is_present() {
    let scratch = tempfile::tempdir().unwrap();
    let bin = fabricate(scratch.path(), &["node", "npm", "npx"]);

    let layout = from_bin_dir(&bin, "22.11.0");
    assert_eq!(layout.version, "22.11.0");
    for tool in super::TOOLS {
        assert!(layout.executable(tool).is_some(), "{tool} was not recorded");
    }
}

#[test]
fn a_tool_that_is_absent_is_not_claimed() {
    // Claiming a path that is not there turns a clear "this install has no npm"
    // into a confusing spawn failure much later.
    let scratch = tempfile::tempdir().unwrap();
    let bin = fabricate(scratch.path(), &["node"]);

    let layout = from_bin_dir(&bin, "22.11.0");
    assert!(layout.executable("node").is_some());
    assert!(layout.executable("npm").is_none());
}

#[tokio::test]
async fn a_directory_with_no_node_is_not_a_toolchain() {
    let scratch = tempfile::tempdir().unwrap();
    fs::create_dir_all(bin_dir(scratch.path())).unwrap();
    assert!(super::describe(scratch.path(), "v22.11.0").await.is_none());
}

#[tokio::test]
async fn an_empty_directory_is_not_a_toolchain() {
    let scratch = tempfile::tempdir().unwrap();
    assert!(super::describe(scratch.path(), "v22.11.0").await.is_none());
}

/// Write an executable standing in for `node`, printing `version`.
#[cfg(unix)]
fn fake_node(bin: &Path, version: &str) {
    use std::os::unix::fs::PermissionsExt;

    let path = bin.join(super::executable_name("node"));
    fs::write(&path, format!("#!/bin/sh\necho '{version}'\n")).expect("the script writes");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .expect("the script is executable");
}

#[cfg(unix)]
#[tokio::test]
async fn an_install_on_the_requested_major_line_is_described() {
    let scratch = tempfile::tempdir().unwrap();
    let bin = fabricate(scratch.path(), &["npm", "npx"]);
    fake_node(&bin, "v22.9.0");

    let layout = super::describe(scratch.path(), "v22.11.0")
        .await
        .expect("a compatible install is described");

    assert_eq!(layout.version, "22.9.0");
    assert_eq!(layout.bin_dir, bin.to_string_lossy());
    for tool in super::TOOLS {
        assert!(layout.executable(tool).is_some(), "{tool} was not recorded");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn an_install_on_another_major_line_is_not_described() {
    // The router scans a cache that may hold several versions. Reporting the
    // wrong one would install nothing and run the wrong interpreter.
    let scratch = tempfile::tempdir().unwrap();
    let bin = fabricate(scratch.path(), &[]);
    fake_node(&bin, "v20.11.0");

    assert!(super::describe(scratch.path(), "v22.11.0").await.is_none());
}

#[cfg(unix)]
#[tokio::test]
async fn an_install_whose_interpreter_does_not_answer_is_not_described() {
    use std::os::unix::fs::PermissionsExt;

    let scratch = tempfile::tempdir().unwrap();
    let bin = fabricate(scratch.path(), &[]);
    let path = bin.join(super::executable_name("node"));
    fs::write(&path, "#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();

    assert!(super::describe(scratch.path(), "v22.11.0").await.is_none());
}
