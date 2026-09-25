# tinyruntime-nodejs

The Node.js provider for [`tinyruntime`](https://github.com/tinyhumansai/tinyruntime).

## What this is

One half of a deliberate split.

`tinyruntime` — the router — owns everything that is the same for every
language: downloading an archive, verifying its digest, unpacking it, promoting
it into a cache atomically, reusing it on the next start, and keeping a bounded
set of warm interpreter processes in front of it.

This module owns everything that is true only of Node.js. It answers five
questions and does nothing else:

| Member | What it answers |
| --- | --- |
| `Describe` | what this provider is and what it targets by default |
| `DetectSystem` | whether the host already has a usable interpreter |
| `SelectDistribution` | which archive to install for this machine |
| `Layout` | where the binaries are inside an unpacked install |
| `Harness` | what a warm Node worker is |

It downloads nothing, installs nothing, and starts no worker. Every answer it
gives is a description the router acts on.

## The Node.js knowledge, in four parts

**Compatibility is a major-line question.** Node keeps its ABI stable across a
major line, and projects pin their own dependencies in `package-lock.json`
rather than the interpreter's patch level. So a host with `v22.8.0` satisfies a
request for `v22.11.0` — which is what stops most machines from downloading
anything at all. A caller that needs an exact interpreter turns off system
preference instead.

**The digests live beside the archives.** nodejs.org publishes one
`SHASUMS256.txt` per release rather than per-asset metadata, so selecting a
distribution means reading it. A release that does not list the archive is
refused here rather than installed unverified.

**The layout is platform-shaped, not archive-shaped.** Unix keeps its tools under
`bin/`; the official Windows zip has no `bin/` directory at all. And `npm` has to
be reached through its launcher — the Unix distributions ship `bin/npm` as a
symlink into a JavaScript file under `lib/`, and invoking that file directly is
not the supported contract.

**A warm worker runs each job in a `worker_thread`.** That gives every job a
fresh module graph and fresh globals, and it makes termination safe: a runaway
job, or one that calls `process.exit()`, is killed without taking the long-lived
worker down with it. The protocol runs over an authenticated loopback socket
rather than the job's stdout, so a job printing a frame-shaped line cannot answer
its own request.

## Using it

Load it alongside `tinyruntime`, which routes `nodejs` to the well-known name
this module claims (`ai.tinyhumans.runtime.nodejs.Provider`). A host then asks
the router to run JavaScript and never addresses this module directly.

```rust
use tinyruntime_nodejs::{DEFAULT_VERSION, satisfies};

// The rule that keeps most machines from downloading anything.
assert!(satisfies("v22.8.0", DEFAULT_VERSION));
assert!(!satisfies("v20.11.0", DEFAULT_VERSION));
```

## Supported hosts

Whatever nodejs.org publishes a prebuilt binary for: macOS and Linux on x86-64
and ARM64, Linux on ARMv7, ppc64le, and s390x, and Windows on x86-64 and ARM64.
Anything else is refused by name rather than guessed at.

## Linking into a host

The default build exports the TinyBus C ABI for dynamic loading. The pinned
TinyBus gitlink (433d9ed, PR #29) supplies `module_export_static!` and its
`linked_module()` helper. To link the module into a Rust host, enable its
`static-link` feature (or the `linked` alias) and pass the public
`linked_module()` result to the TinyBus linked-module host API. This uses the
same declaration and manifest as the dynamic build, with Rust-addressable
symbols that can coexist with other linked modules.

## Building

```sh
git submodule update --init --recursive
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --all-targets --all-features
cargo test --all-features
```

The harness suite in `tests/harness_protocol.rs` launches a real `node` and
drives the protocol end to end. It skips when the machine has none, so the suite
stays hermetic on a runner without Node.

See [`AGENTS.md`](AGENTS.md) for the working agreement, and
[`MODULE.md`](MODULE.md) for installing a release artifact.

## License

GPL-3.0-only. See [`LICENSE`](LICENSE).
