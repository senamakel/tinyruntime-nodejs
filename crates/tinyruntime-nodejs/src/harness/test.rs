//! Unit tests for the shipped harness.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use tinyruntime_bus::WORKER_PROTOCOL_VERSION;

use super::{FILENAME, SOURCE, harness};

#[test]
fn the_harness_runs_under_node_and_speaks_this_protocol() {
    let harness = harness();
    assert_eq!(harness.executable, "node");
    assert_eq!(harness.filename, FILENAME);
    assert_eq!(
        harness.protocol_version, WORKER_PROTOCOL_VERSION,
        "a harness on another protocol is refused at the handshake"
    );
}

#[test]
fn the_flags_the_harness_depends_on_travel_with_it() {
    // Without these the harness still starts and then fails on the first job
    // that uses a dynamic import, which is a much harder failure to read.
    let args = harness().command_args("/cache/pool_worker.js");
    assert!(args.contains(&"--experimental-vm-modules".to_string()));
    assert!(args.contains(&"--experimental-import-meta-resolve".to_string()));
    assert_eq!(
        args.last().map(String::as_str),
        Some("/cache/pool_worker.js"),
        "the script must come after the flags"
    );
}

#[test]
fn the_harness_announces_the_protocol_version_this_build_speaks() {
    // The script's constant and the contract's are two separate declarations of
    // one number; a mismatch fails every handshake at runtime.
    assert!(
        SOURCE.contains(&format!("const PROTOCOL_VERSION = {WORKER_PROTOCOL_VERSION};")),
        "the harness declares a different protocol version than the contract"
    );
}

#[test]
fn the_harness_reads_the_protocol_environment_the_router_sets() {
    // These two names are the entire handshake contract with the router. A
    // rename on either side produces a worker that never connects back.
    assert!(SOURCE.contains("TINYRUNTIME_PROTOCOL_ADDR"));
    assert!(SOURCE.contains("TINYRUNTIME_PROTOCOL_TOKEN"));
}

#[test]
fn the_harness_never_replies_over_stdout() {
    // A job owns stdout. If replies shared it, a job printing a frame-shaped
    // line could answer its own request or desynchronise the next one.
    assert!(
        !SOURCE.contains("process.stdout.write(JSON"),
        "the harness writes protocol frames to stdout"
    );
    assert!(SOURCE.contains("socket.write(JSON.stringify(frame)"));
}

#[test]
fn a_job_that_cannot_enter_its_directory_fails_rather_than_running_elsewhere() {
    // Running in the previous job's directory would escape whatever sandbox the
    // caller set up, which is worse than the job failing.
    assert!(SOURCE.contains("failed to set worker cwd"));
}
