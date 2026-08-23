//! What counts as a compatible Node.js version.
//!
//! Matching is on the **major** component only, and that is a deliberate
//! loosening rather than an oversight. Node.js keeps its ABI stable across a
//! major line, and a project pins its own dependencies in `package-lock.json`
//! rather than relying on a patch release of the interpreter. So a host with
//! `v22.8.0` satisfies a request for `v22.11.0`, and that one rule is what stops
//! most machines from ever downloading anything.
//!
//! A caller that genuinely needs an exact interpreter turns off system
//! preference, which skips host detection entirely and installs the pinned
//! version.

/// The major component of a Node.js version string.
///
/// Accepts every spelling that turns up in practice: a configuration value
/// (`v22.11.0`), what `node --version` prints (`v22.11.0\n`), a bare version
/// (`22.11.0`), and a major on its own (`22`).
///
/// Returns `None` when the input is not a version at all.
///
/// # Examples
///
/// ```
/// # use tinyruntime_nodejs::major;
/// assert_eq!(major("v22.11.0"), Some(22));
/// assert_eq!(major(" 22.11.0\n"), Some(22));
/// assert_eq!(major("22"), Some(22));
/// assert_eq!(major("latest"), None);
/// ```
#[must_use]
pub fn major(raw: &str) -> Option<u32> {
    let trimmed = raw.trim();
    let stripped = trimmed.strip_prefix('v').unwrap_or(trimmed);
    stripped.split('.').next()?.parse::<u32>().ok()
}

/// Whether `candidate` satisfies a request for `target`.
///
/// Both sides must be parseable. An unparseable target is a configuration error
/// the caller should hear about rather than a reason to accept anything.
#[must_use]
pub fn satisfies(candidate: &str, target: &str) -> bool {
    match (major(candidate), major(target)) {
        (Some(candidate), Some(target)) => candidate == target,
        _ => false,
    }
}

/// The canonical `vX.Y.Z` spelling nodejs.org uses in its URLs.
///
/// # Examples
///
/// ```
/// # use tinyruntime_nodejs::canonical_version;
/// assert_eq!(canonical_version("22.11.0"), "v22.11.0");
/// assert_eq!(canonical_version(" v22.11.0 "), "v22.11.0");
/// ```
#[must_use]
pub fn canonical_version(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with('v') {
        trimmed.to_owned()
    } else {
        format!("v{trimmed}")
    }
}

/// The version without its leading `v`, as a toolchain reports itself.
#[must_use]
pub fn bare_version(raw: &str) -> String {
    let trimmed = raw.trim();
    trimmed.strip_prefix('v').unwrap_or(trimmed).to_owned()
}

#[cfg(test)]
mod test;
