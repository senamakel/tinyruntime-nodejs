//! Unit tests for the crate-wide error type.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::Error;

#[test]
fn messages_are_lowercase_and_unpunctuated() {
    let errors = [
        Error::UnsupportedHost {
            os: "freebsd".to_string(),
            arch: "x86_64".to_string(),
        },
        Error::InvalidVersion("latest".to_string()),
        Error::DigestUnavailable {
            version: "v22.11.0".to_string(),
            reason: "the request timed out".to_string(),
        },
    ];
    for error in errors {
        let rendered = error.to_string();
        assert!(
            !rendered.ends_with('.'),
            "`{rendered}` ends with punctuation"
        );
        let first = rendered.chars().next().expect("a non-empty message");
        assert!(!first.is_uppercase(), "`{rendered}` starts with a capital");
    }
}

#[test]
fn an_unsupported_host_names_the_machine_it_refused() {
    let rendered = Error::UnsupportedHost {
        os: "freebsd".to_string(),
        arch: "riscv64".to_string(),
    }
    .to_string();
    assert!(rendered.contains("freebsd/riscv64"), "got `{rendered}`");
}

#[test]
fn an_invalid_version_quotes_what_was_asked_for() {
    let rendered = Error::InvalidVersion("latest".to_string()).to_string();
    assert!(rendered.contains("`latest`"), "got `{rendered}`");
}
