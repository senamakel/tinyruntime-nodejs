//! Unit tests for Node.js version handling.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{bare_version, canonical_version, major, satisfies};

#[test]
fn every_spelling_that_turns_up_in_practice_parses() {
    assert_eq!(major("v22.11.0"), Some(22));
    assert_eq!(major("22.11.0"), Some(22));
    assert_eq!(
        major("v22.11.0\n"),
        Some(22),
        "this is what `node --version` prints"
    );
    assert_eq!(major("  22  "), Some(22));
}

#[test]
fn something_that_is_not_a_version_does_not_parse() {
    assert_eq!(major("latest"), None);
    assert_eq!(major(""), None);
    assert_eq!(major("vlatest"), None);
}

#[test]
fn a_newer_patch_on_the_same_major_satisfies_the_request() {
    // The rule that stops most machines from downloading anything: Node keeps
    // its ABI stable across a major line, and projects pin their own
    // dependencies rather than the interpreter's patch level.
    assert!(satisfies("v22.8.0", "v22.11.0"));
    assert!(satisfies("v22.20.1", "v22.11.0"));
}

#[test]
fn a_different_major_does_not_satisfy_the_request() {
    assert!(!satisfies("v20.11.0", "v22.11.0"));
    assert!(!satisfies("v24.0.0", "v22.11.0"));
}

#[test]
fn an_unparseable_target_accepts_nothing() {
    // A misconfigured pin must not silently accept whatever is installed.
    assert!(!satisfies("v22.11.0", "latest"));
    assert!(!satisfies("nonsense", "v22.11.0"));
}

#[test]
fn canonical_and_bare_spellings_round_trip() {
    assert_eq!(canonical_version("22.11.0"), "v22.11.0");
    assert_eq!(canonical_version("v22.11.0"), "v22.11.0");
    assert_eq!(bare_version("v22.11.0"), "22.11.0");
    assert_eq!(bare_version("22.11.0"), "22.11.0");
    assert_eq!(
        bare_version(canonical_version("22.11.0").as_str()),
        "22.11.0"
    );
}
