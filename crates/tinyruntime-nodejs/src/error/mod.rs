//! The crate-wide error type and result alias.
//!
//! A provider's failures are narrow by design: it answers questions and does not
//! install anything, so the things that can go wrong are a host it cannot serve,
//! a version that is not one, and a release index it could not read.
//!
//! Messages are lowercase and carry no credential, payload, or absolute path.
//! They travel to the router, which puts them in front of a person.

/// Everything this provider can fail with.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Node.js publishes no prebuilt binary for this machine.
    #[error("node.js publishes no build for {os}/{arch}")]
    UnsupportedHost {
        /// The operating system, as Rust names it.
        os: String,
        /// The architecture, as Rust names it.
        arch: String,
    },

    /// The requested version is not a version.
    #[error("`{0}` is not a node.js version")]
    InvalidVersion(String),

    /// The release's digest file could not be read, or does not list the archive.
    ///
    /// Refusing rather than installing unverified is deliberate: nodejs.org
    /// always publishes digests, so their absence means something is wrong with
    /// what is being served rather than with the expectation.
    #[error("the digests for node.js {version} could not be read: {reason}")]
    DigestUnavailable {
        /// The release whose digests were being read.
        version: String,
        /// Why they could not be, sanitised for display.
        reason: String,
    },
}

/// The crate's result alias.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod test;
