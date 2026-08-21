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
    detect_in(settings, std::env::var_os("PATH").as_ref()).await
}

/// [`detect`] with the search path supplied explicitly.
///
/// Split out so a test can point the probe at a directory it controls. Rewriting
/// the process environment is not an option — `unsafe` is forbidden workspace-wide
/// and a shared `PATH` would make concurrent tests interfere — and a detector
/// that cannot be tested against a known interpreter is one whose candidate
/// ordering nothing checks.
pub async fn detect_in(
    settings: &RuntimeSettings,
    path_var: Option<&std::ffi::OsString>,
) -> Option<RuntimeLayout> {
    if version::major(&settings.version).is_none() {
        tracing::warn!(
            "[tinyruntime-nodejs] the requested version is not a version; skipping host detection"
        );
        return None;
    }

    for candidate in candidates(settings.preferred_command()) {
        let Some(path) = locate(&candidate, path_var) else {
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
fn locate(command: &str, path_var: Option<&std::ffi::OsString>) -> Option<PathBuf> {
    let as_path = Path::new(command);
    if as_path.is_absolute() || as_path.components().count() > 1 {
        return is_executable(as_path).then(|| as_path.to_path_buf());
    }

    let path_var = path_var?;
    for directory in std::env::split_paths(path_var) {
        let candidate = directory.join(command);
        if is_executable(&candidate) {
            return Some(candidate);
        }
        if let Some(found) = windows_executable(&directory, command, cfg!(windows)) {
            return Some(found);
        }
    }
    None
}

/// The `.exe` a bare command names on Windows, if it is there.
///
/// The platform is a parameter rather than a `cfg!`, so the Windows lookup is
/// exercised on the machines that actually run this suite — otherwise it is code
/// nobody tests until it is the only thing between a host and its interpreter.
fn windows_executable(directory: &Path, command: &str, windows: bool) -> Option<PathBuf> {
    if !windows {
        return None;
    }
    let candidate = directory.join(format!("{command}.exe"));
    is_executable(&candidate).then_some(candidate)
}

/// Whether `path` is a file this process could execute.
#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
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
