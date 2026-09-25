//! Integration tests for the public crate surface.
//!
//! These link against the crate as a downstream consumer would: they can only
//! use what `src/lib.rs` re-exports. Treat them as the regression suite for the
//! crate's public contract — if a change breaks a test here, it is a breaking
//! change for users.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use tinyruntime_nodejs::{
    CONTRACT_VERSION, DEFAULT_VERSION, Error, Language, NODEJS, distribution, harness, layout,
    major, names, satisfies,
};

#[test]
fn the_provider_serves_the_shared_interface_from_the_contract() {
    assert_eq!(
        names::PROVIDER_INTERFACE,
        tinyruntime_nodejs::PROVIDER_INTERFACE
    );
    assert_eq!(
        names::providers::NODEJS_OBJECT_PATH,
        names::object_path_for(names::providers::NODEJS)
    );
    assert_eq!(names::PROVIDER_METHODS.len(), 5);
    assert_eq!(Language::nodejs().as_str(), NODEJS);
}

#[test]
fn the_contract_is_re_exported_so_consumers_take_one_dependency() {
    let same: tinyruntime_bus::WorkerHarness = harness();
    assert_eq!(same, harness());
    assert!(tinyruntime_nodejs::is_compatible_with_contract());
    let _ = CONTRACT_VERSION;
}

#[test]
fn the_default_version_is_one_this_crate_can_reason_about() {
    assert!(major(DEFAULT_VERSION).is_some());
    assert!(satisfies(DEFAULT_VERSION, DEFAULT_VERSION));
}

#[test]
fn the_host_table_and_the_layout_agree_about_this_platform() {
    // A host the archive table covers must also be one the layout knows the
    // shape of, or an install would succeed and then be unusable.
    if distribution::host_archive().is_ok() {
        let bin = layout::bin_dir(std::path::Path::new("/cache/node-v22.11.0"));
        assert!(bin.starts_with("/cache/node-v22.11.0"));
        assert!(!layout::executable_name("node").is_empty());
    }
}

#[test]
fn an_unsupported_host_is_reported_by_name() {
    let error = distribution::archive_for("plan9", "x86_64").expect_err("no build exists");
    assert!(matches!(error, Error::UnsupportedHost { .. }));
}

#[cfg(feature = "static-link")]
#[test]
fn linked_entry_points_are_available_to_a_host() {
    fn assert_entry_types(
        _: &tinybus::module::abi::TbAbiDescriptor,
        _: tinybus::module::abi::TbModuleInit,
    ) {
    }
    assert_entry_types(
        &tinyruntime_nodejs::linked::TINYBUS_MODULE_ABI_V1,
        tinyruntime_nodejs::linked::tinybus_module_init_v1,
    );
    let manifest = tinyruntime_nodejs::linked::tinybus_module_manifest_v1();
    assert!(manifest.len > 0);
    tinyruntime_nodejs::linked_module().expect("generated manifest is valid");
}
