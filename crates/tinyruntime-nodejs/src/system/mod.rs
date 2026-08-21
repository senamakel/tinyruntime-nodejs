//! Finding a Node.js toolchain the host already has.
//!
//! This is the cheapest step in the whole system and the one that pays for
//! itself most often: a developer machine almost always has a compatible `node`
//! already, and one `--version` probe is the difference between using it and
//! downloading a hundred megabytes to sit beside it.
//!
//! The probe runs the interpreter, which means it can hang — a wedged binary, a
//! network filesystem, an antivirus scanner deciding to inspect it. It is
//! therefore bounded, and a probe that does not answer in time is treated as no
//! interpreter at all rather than waited on.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tinyruntime_bus::{RuntimeLayout, RuntimeSettings};

use crate::{layout, version};

/// How long a `node --version` probe may take before it is abandoned.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Look for a host toolchain satisfying `settings`.
///
/// Returns `None` when nothing on `PATH` matches, which is the signal for the
/// router to install a managed toolchain instead.
pub async fn detect(settings: &RuntimeSettings) -> Option<RuntimeLayout> {
    if version::major(&settings.version).is_none() {
        tracing::warn!(
            "[tinyruntime-nodejs] the requested version is not a version; skipping host detection"
        );
        return None;
    }

    for candidate in candidates(settings.preferred_command()) {
        let Some(path) = locate(&candidate) else {
            continue;
        };
        let Some(reported) = probe_version(&path).await else {
            tracing::debug!("[tinyruntime-nodejs] a candidate did not answer `--version`");
            continue;
        };
        if !version::satisfies(&reported, &settings.version) {
            tracing::debug!(
                reported = %reported,
                "[tinyruntime-nodejs] a host interpreter is on a different major line"
            );
            continue;
        }

        let Some(bin_dir) = path.parent() else {
            continue;
        };
        tracing::info!(
            reported = %reported,
            "[tinyruntime-nodejs] reusing a compatible host interpreter"
        );
        return Some(layout::from_bin_dir(
            bin_dir,
            &version::bare_version(&reported),
        ));
    }
    None
}

/// The commands to try, in order.
///
/// A caller's preferred command comes first and may be an absolute path, which
/// is how a host points at an interpreter that is deliberately not on `PATH`.
fn candidates(preferred: Option<&str>) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(preferred) = preferred {
        candidates.push(preferred.to_owned());
    }
    if !candidates.iter().any(|candidate| candidate == "node") {
        candidates.push("node".to_owned());
    }
    candidates
}

/// Resolve a command to an executable file, searching `PATH` for a bare name.
fn locate(command: &str) -> Option<PathBuf> {
    let as_path = Path::new(command);
    if as_path.is_absolute() || as_path.components().count() > 1 {
        return is_executable(as_path).then(|| as_path.to_path_buf());
    }

    let path_var = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path_var) {
        let candidate = directory.join(command);
        if is_executable(&candidate) {
            return Some(candidate);
        }
        if cfg!(windows) {
            let with_extension = directory.join(format!("{command}.exe"));
            if is_executable(&with_extension) {
                return Some(with_extension);
            }
        }
    }
    None
}

/// Whether `path` is a file this process could execute.
#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Whether `path` is a file. Windows has no execute bit to consult.
#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// Ask an interpreter what version it is, within a bounded time.
///
/// Returns `None` for anything other than a clean, prompt answer — a binary that
/// hangs, exits non-zero, or prints nothing is not a toolchain worth reusing.
pub async fn probe_version(binary: &Path) -> Option<String> {
    let mut command = tokio::process::Command::new(binary);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    no_console_window(&mut command);

    let output = tokio::time::timeout(PROBE_TIMEOUT, command.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let reported = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if reported.is_empty() {
        None
    } else {
        Some(reported)
    }
}

/// Suppress the console window Windows would flash for each probe.
#[cfg(windows)]
fn no_console_window(command: &mut tokio::process::Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

/// No-op off Windows.
#[cfg(not(windows))]
fn no_console_window(_command: &mut tokio::process::Command) {}

#[cfg(test)]
mod test;
