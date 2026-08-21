//! The warm-worker harness this provider ships to the router.
//!
//! The script is compiled into the module and handed over on request rather than
//! installed anywhere. That is what keeps the two in step: an upgraded provider
//! ships an upgraded harness, and there is no version of it on disk that can be
//! newer or older than the module that launched it.
//!
//! Two Node flags travel with it, and both are required rather than defensive.
//! `--experimental-vm-modules` is what lets the harness root a job's dynamic
//! `import()` at the job's own directory instead of at the harness file, and
//! `--experimental-import-meta-resolve` is what lets a bare import resolve the
//! way it would under `node -e`.

use tinyruntime_bus::WorkerHarness;

/// The harness source, compiled in.
const SOURCE: &str = include_str!("pool_worker.js");

/// The filename the router writes the harness under.
const FILENAME: &str = "pool_worker.js";

/// The harness for this provider's warm workers.
#[must_use]
pub fn harness() -> WorkerHarness {
    WorkerHarness::new(FILENAME, SOURCE, "node")
        .with_flag("--experimental-vm-modules")
        .with_flag("--experimental-import-meta-resolve")
}

#[cfg(test)]
mod test;
