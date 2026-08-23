# Roadmap

What is deliberately not built yet, and what would have to be true before it is.

## Shipped

- Host interpreter detection, bounded so a wedged binary costs a probe rather
  than a hang.
- Distribution selection for every platform nodejs.org publishes, with the
  digest read from the release's own `SHASUMS256.txt`.
- Install layout for both shapes, including reaching `npm` through its launcher.
- The warm-worker harness: a `worker_thread` per job, `node -e` semantics for
  dynamic imports, soft deadlines, and a protocol a job cannot forge.

## Next

**Package installation.** `npm` is reported in the layout but nothing calls it.
Whether installing dependencies belongs behind a provider member or stays a host
concern is an open question in the contract, not here.

**Corepack and pnpm/yarn.** The managed toolchain ships `corepack`, which is the
supported way to reach the other package managers. Reporting it in the layout is
a one-line change; the question is whether anything wants it yet.

**A version index.** `SelectDistribution` needs an exact version and cannot
resolve `"lts"` or `"latest"`. Node publishes `index.json` beside the releases,
so this is straightforward — it has just not been needed.

## Not planned

**Downloading or installing anything here.** That is the router's half, and
duplicating it would give every language its own subtly different pipeline.

**Falling back to an incompatible interpreter.** A request that names a major
line and finds another one gets nothing rather than a surprise.
