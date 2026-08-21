//! Where a Node.js toolchain keeps its executables.
//!
//! Two shapes, decided by the platform rather than by anything in the archive:
//!
//! * Unix: `<install>/bin/{node,npm,npx}`.
//! * Windows: `<install>/{node.exe,npm.cmd,npx.cmd}` — the official zip has no
//!   `bin/` directory at all.
//!
//! `npm` in particular has to be addressed through its launcher script. The Unix
//! distributions ship `bin/npm` as a symlink into a JavaScript file under
//! `lib/`, and invoking that file directly is not the supported contract.

use std::path::{Path, PathBuf};

use tinyruntime_bus::RuntimeLayout;

use crate::version;

/// The directory a child's `PATH` should start with.
///
/// Also where the toolchain's own tools live, which is why a job that shells out
/// to `npx` reaches the same install that is running it.
#[must_use]
pub fn bin_dir(install_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        install_dir.to_path_buf()
    } else {
        install_dir.join("bin")
    }
}

/// The platform's filename for a toolchain tool.
#[must_use]
pub fn executable_name(tool: &str) -> String {
    if !cfg!(windows) {
        return tool.to_owned();
    }
    match tool {
        // `node` is a real executable; the package tools are batch shims.
        "node" => "node.exe".to_owned(),
        other => format!("{other}.cmd"),
    }
}

/// Describe the toolchain in `install_dir`, if there is a usable one.
///
/// Returns `None` when `node` is missing or reports a version that does not
/// satisfy `target_version`. Both are ordinary answers: the router is scanning a
/// cache that may hold several versions and several kinds of leftover, and
/// "this directory is not the toolchain you asked for" is the common case rather
/// than a failure.
pub async fn describe(install_dir: &Path, target_version: &str) -> Option<RuntimeLayout> {
    let bin_dir = bin_dir(install_dir);
    let node = bin_dir.join(executable_name("node"));
    if !node.is_file() {
        return None;
    }

    let reported = crate::system::probe_version(&node).await?;
    if !version::satisfies(&reported, target_version) {
        tracing::debug!(
            reported = %reported,
            "[tinyruntime-nodejs] a cached install is a different major line"
        );
        return None;
    }

    Some(from_bin_dir(&bin_dir, &version::bare_version(&reported)))
}

/// Build a layout from a directory known to hold the toolchain's executables.
///
/// Only tools that are actually present are recorded. A toolchain missing `npm`
/// is unusual but usable, and claiming a path that is not there would turn a
/// clear "this install has no npm" into a spawn failure much later.
#[must_use]
pub fn from_bin_dir(bin_dir: &Path, version: &str) -> RuntimeLayout {
    let mut layout = RuntimeLayout::new(version, bin_dir.to_string_lossy().into_owned());
    for tool in TOOLS {
        let path = bin_dir.join(executable_name(tool));
        if path.is_file() {
            layout = layout.with_executable(*tool, path.to_string_lossy().into_owned());
        }
    }
    layout
}

/// The logical executables this provider reports.
pub const TOOLS: &[&str] = &["node", "npm", "npx"];

#[cfg(test)]
mod test;
