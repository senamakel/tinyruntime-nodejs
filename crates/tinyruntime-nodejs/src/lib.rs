//! The Node.js runtime provider for tinyruntime.
//!
//! # What this crate is
//!
//! One half of a deliberate split. `tinyruntime` — the router — owns everything
//! that is the same for every language: downloading an archive, verifying its
//! digest, unpacking it, promoting it into a cache atomically, reusing it on the
//! next start, and keeping a bounded set of warm interpreter processes in front
//! of it. This crate owns everything that is true only of Node.js:
//!
//! - [`version`] — that compatibility is a major-line question, which is why a
//!   host with `v22.8.0` satisfies a request for `v22.11.0` and never downloads.
//! - [`system`] — how to find a host interpreter and ask it what it is.
//! - [`distribution`] — which archive nodejs.org publishes for this machine, and
//!   that its digest lives in a `SHASUMS256.txt` beside it.
//! - [`layout`] — that Unix keeps its tools under `bin/`, Windows keeps them at
//!   the root, and `npm` must be reached through its launcher.
//! - [`harness`](mod@harness) — what a warm Node worker is: a `worker_thread` per job, for a
//!   fresh module graph and a safe kill.
//!
//! It downloads nothing, installs nothing, and starts no worker. Every answer it
//! gives is a description the router acts on.
//!
//! # Using it
//!
//! Load it alongside `tinyruntime`, which routes `nodejs` to the well-known name
//! this module claims. A host then asks the router to run JavaScript and never
//! addresses this module directly.
//!
//! ```
//! use tinyruntime_nodejs::{DEFAULT_VERSION, satisfies};
//!
//! // The rule that keeps most machines from downloading anything.
//! assert!(satisfies("v22.8.0", DEFAULT_VERSION));
//! assert!(!satisfies("v20.11.0", DEFAULT_VERSION));
//! ```

pub mod distribution;
pub mod error;
pub mod harness;
pub mod layout;
pub mod system;
pub mod version;

mod tinybus_module;

/// Entry points for hosts that link this module into their executable.
#[cfg(feature = "static-link")]
pub mod linked {
    pub use crate::tinybus_module::exports::{
        TINYBUS_MODULE_ABI_V1, linked_module, tinybus_module_init_v1, tinybus_module_manifest_v1,
    };
}

#[cfg(feature = "static-link")]
pub use linked::linked_module;

pub use error::{Error, Result};
pub use harness::harness;
pub use tinybus_module::DEFAULT_VERSION;
pub use version::{bare_version, canonical_version, major, satisfies};

/// Whether this provider's contract version can bind to the one it was built
/// against.
///
/// Always true for a build whose vendored contract matches, and the check the
/// router makes before routing anything here. Exposed so a host can assert it
/// without reconstructing the comparison.
#[must_use]
pub fn is_compatible_with_contract() -> bool {
    tinyruntime_bus::is_compatible(CONTRACT_VERSION)
}
pub use tinyruntime_bus::NODEJS;

// The wire contract, re-exported whole, so a consumer of this crate names the
// very types the module serves rather than copies of them.
pub use tinyruntime_bus::{
    ArchiveFormat, CONTRACT_VERSION, Distribution, Language, LayoutRequest, LayoutResponse,
    PROVIDER_INTERFACE, PROVIDER_METHODS, ProviderDescriptor, RuntimeLayout, RuntimeSettings,
    WORKER_PROTOCOL_VERSION, WorkerHarness, names, object_path_for,
};
