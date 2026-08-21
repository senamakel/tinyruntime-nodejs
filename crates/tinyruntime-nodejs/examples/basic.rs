//! What this provider knows about Node.js, printed.
//!
//! Every answer here is one the router would otherwise have to hard-code. None
//! of it downloads or installs anything — that is the router's half.

use tinyruntime_nodejs::{DEFAULT_VERSION, RuntimeSettings, distribution, harness, satisfies};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("default version: {DEFAULT_VERSION}");

    match distribution::host_archive() {
        Ok(host) => println!(
            "this host installs: node-{DEFAULT_VERSION}-{} ({})",
            host.suffix,
            host.format.extension()
        ),
        Err(error) => println!("this host cannot install a managed toolchain: {error}"),
    }

    // The rule that decides whether anything gets downloaded at all.
    for candidate in ["v22.8.0", "v22.20.1", "v20.11.0", "v24.0.0"] {
        let verdict = if satisfies(candidate, DEFAULT_VERSION) {
            "reused"
        } else {
            "rejected"
        };
        println!("  a host {candidate} would be {verdict}");
    }

    match tinyruntime_nodejs::system::detect(&RuntimeSettings::new(DEFAULT_VERSION)).await {
        Some(layout) => println!("found a host toolchain: {} at {}", layout.version, layout.bin_dir),
        None => println!("no compatible host toolchain; the router would install one"),
    }

    let harness = harness();
    println!(
        "warm worker: {} ({} bytes) under `{}` with {:?}",
        harness.filename,
        harness.source.len(),
        harness.executable,
        harness.args_before_script
    );
    Ok(())
}
