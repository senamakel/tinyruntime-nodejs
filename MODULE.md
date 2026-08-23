# tinyruntime-nodejs TinyBus Module

This package contains the native `tinyruntime-nodejs` module for TinyBus module
ABI v1. Install only the archive matching the host operating system and
architecture.

The module claims `ai.tinyhumans.runtime.nodejs.Provider`, implements the shared
provider interface `ai.tinyhumans.runtime.Provider` at
`/ai/tinyhumans/runtime/nodejs/Provider`, and provides `Describe`, `DetectSystem`,
`SelectDistribution`, `Layout`, and `Harness`. Every payload type and every name
is published as the `tinyruntime-bus` crate, so a host names them from a library
rather than by string literal.

## It is not useful on its own

This is a provider. It answers questions about Node.js and performs no
installation or execution of its own. Load it alongside the `tinyruntime`
module, which routes the `nodejs` language to the bus name above.

Load order does not matter: the router contacts providers per call, not at
setup, so this module may be loaded before or after it.

## Configuration

None. The module takes no configuration — everything it needs arrives with each
request, so a host that changes a version pin or a cache directory does not
reload anything.

## What it does to the machine

Nothing persistent. It runs `node --version` to probe candidate interpreters,
and it reads `SHASUMS256.txt` from nodejs.org when the router asks which archive
to install. It writes no files and starts no long-lived process.

## Installing

The archive contains one `.so`, `.dylib`, or `.dll` plus `modules.toml`. Keep
those files together when copying them into a TinyBus module directory. The
allowlist binds the native library filename to its SHA-256 digest so TinyBus can
reject a missing, renamed, or modified artifact before initialization.

The GitHub release also publishes `checksum.toml` as a separate asset. TinyBus
checks that manifest before downloading and extracting the selected platform
archive. Install directly from a tagged release with:

```sh
tinybus modules load-github \
  https://github.com/tinyhumansai/tinyruntime-nodejs/releases/tag/v0.1.0 \
  tinyruntime-nodejs-0.1.0-ubuntu-24.04-x86_64.tar.gz \
  <archive-sha256>
```

TinyBus modules are trusted in-process code. Install release artifacts only
from a trusted source and restart the host after replacing a loaded module.
