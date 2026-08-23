// The tinyruntime Node.js worker harness.
//
// One long-lived `node` process that runs many inline jobs, so a host pays for
// one warm interpreter instead of one child per execution.
//
// Protocol (newline-delimited JSON over an authenticated loopback socket):
//   1. Send exactly one handshake: {ready, protocol, language, token}
//   2. For each {id, code, cwd, timeout_ms} reply with
//      {id, ok, stdout, stderr, exit_code, timed_out, elapsed_ms, error}
//
// Two properties are load-bearing and easy to lose.
//
// Each job runs in its own worker_thread. That gives it a fresh module graph and
// fresh globals — so one job cannot see or corrupt the next one's state — and it
// makes termination safe: a runaway job, or one that calls process.exit(), is
// killed with worker.terminate() without taking this host process down with it.
//
// The protocol travels over a socket, never over stdout. A job owns its own
// stdout and stderr, and if replies shared them a job that printed a line
// shaped like a frame could answer its own request or desynchronise the next
// one.

'use strict';

const { Worker, isMainThread, workerData } = require('worker_threads');

const PROTOCOL_VERSION = 1;

// ---------------------------------------------------------------------------
// Worker-thread mode: run one job's code, then let the thread exit.
// ---------------------------------------------------------------------------
if (!isMainThread) {
  const path = require('path');
  const vm = require('vm');
  const { createRequire } = require('module');
  const { pathToFileURL } = require('url');

  async function runUserCode(code, cwd) {
    // Deliberately no process.chdir() here: it throws inside a worker thread.
    // The host changes directory before constructing this worker, so
    // process.cwd() is already the job's directory.
    const dir = cwd || process.cwd();
    const filename = path.join(dir, 'inline.js');
    const req = createRequire(filename);
    const base = pathToFileURL(filename).href;

    // Bare dynamic imports go through the ESM resolver rather than
    // require.resolve, so a package that only publishes an `import` condition
    // resolves the way it would under `node -e`.
    const { default: resolveEsm } = await import(
      'data:text/javascript,export default (specifier, parent) => import.meta.resolve(specifier, parent)'
    );
    const importFromJob = (specifier, _referrer, importAttributes) => {
      const options =
        importAttributes && Object.keys(importAttributes).length > 0
          ? { with: importAttributes }
          : undefined;
      if (
        specifier.startsWith('.') ||
        specifier.startsWith('/') ||
        specifier.startsWith('file:') ||
        specifier.startsWith('data:')
      ) {
        return import(new URL(specifier, base).href, options);
      }
      return import(resolveEsm(specifier, base), options);
    };

    // Mimic `node -e`: a sloppy CommonJS-ish scope with require and __dirname,
    // wrapped in an async IIFE so top-level await works. vm.compileFunction
    // rather than new Function, because only it lets dynamic import() be rooted
    // at the job's directory instead of at this harness file.
    const fn = vm.compileFunction(
      'return (async () => {\n' + code + '\n})();',
      ['require', '__filename', '__dirname', 'module', 'exports'],
      { filename, importModuleDynamically: importFromJob }
    );
    const mod = { exports: {} };
    await fn(req, filename, dir, mod, mod.exports);
  }

  const code = (workerData && workerData.code) || '';
  const cwd = (workerData && workerData.cwd) || null;
  runUserCode(code, cwd).then(
    () => {
      // Resolving lets the thread exit once its loop drains, which flushes the
      // stdout and stderr pipes before the 'exit' event fires.
    },
    (err) => {
      process.stderr.write((err && err.stack ? err.stack : String(err)) + '\n');
      process.exitCode = 1;
    }
  );
  return;
}

// ---------------------------------------------------------------------------
// Host mode: read jobs, run each in a worker thread, reply one per job.
// ---------------------------------------------------------------------------

function collect(stream) {
  return new Promise((resolve) => {
    let buffer = '';
    stream.setEncoding('utf8');
    stream.on('data', (chunk) => {
      buffer += chunk;
    });
    const done = () => resolve(buffer);
    stream.on('end', done);
    stream.on('close', done);
    stream.on('error', done);
  });
}

function harnessFailure(job, started, message) {
  return {
    id: job && job.id,
    ok: false,
    stdout: '',
    stderr: '',
    exit_code: null,
    timed_out: false,
    elapsed_ms: Date.now() - started,
    error: message,
  };
}

function runJob(job) {
  return new Promise((resolve) => {
    const started = Date.now();

    // The job's directory is set on this process before the worker is
    // constructed: a worker thread inherits the cwd at creation and cannot
    // change it itself. Failing here rather than running anyway matters — a job
    // that silently ran in the previous job's directory would escape whatever
    // sandbox the caller set up.
    const priorCwd = process.cwd();
    if (job.cwd) {
      try {
        process.chdir(job.cwd);
      } catch (e) {
        resolve(
          harnessFailure(
            job,
            started,
            'failed to set worker cwd: ' + (e && e.stack ? e.stack : String(e))
          )
        );
        return;
      }
    }

    let worker;
    try {
      worker = new Worker(__filename, {
        workerData: { id: job.id, code: job.code || '', cwd: job.cwd || null },
        stdout: true,
        stderr: true,
        // Carry this process's flags through, so the worker's dynamic-import
        // hook stays enabled.
        execArgv: process.execArgv,
      });
    } catch (e) {
      try {
        process.chdir(priorCwd);
      } catch (_e) {
        /* best effort */
      }
      resolve(
        harnessFailure(
          job,
          started,
          'failed to start worker thread: ' + (e && e.stack ? e.stack : String(e))
        )
      );
      return;
    }

    const stdoutPromise = collect(worker.stdout);
    const stderrPromise = collect(worker.stderr);
    let exitCode = 0;
    let timedOut = false;
    let extraStderr = '';

    let timer = null;
    if (job.timeout_ms && job.timeout_ms > 0) {
      timer = setTimeout(() => {
        timedOut = true;
        worker.terminate();
      }, job.timeout_ms);
    }

    worker.on('error', (e) => {
      extraStderr += (e && e.stack ? e.stack : String(e)) + '\n';
      if (!exitCode) exitCode = 1;
    });

    worker.on('exit', async (code) => {
      if (timer) clearTimeout(timer);
      // Restore only now: a worker reads its cwd as it initialises, so this
      // process has to stay in the job's directory for the worker's whole life.
      // Jobs are serialised, so the next one starts from the restored directory.
      try {
        process.chdir(priorCwd);
      } catch (_e) {
        /* best effort */
      }
      if (code && !exitCode) exitCode = code;
      const stdout = await stdoutPromise;
      const stderr = (await stderrPromise) + extraStderr;
      resolve({
        id: job.id,
        ok: true,
        stdout,
        stderr,
        exit_code: timedOut ? null : exitCode,
        timed_out: timedOut,
        elapsed_ms: Date.now() - started,
        error: null,
      });
    });
  });
}

function serve(socket) {
  const send = (frame) => socket.write(JSON.stringify(frame) + '\n');

  send({
    ready: true,
    protocol: PROTOCOL_VERSION,
    language: 'nodejs',
    token: process.env.TINYRUNTIME_PROTOCOL_TOKEN || null,
  });

  const readline = require('readline');
  const lines = readline.createInterface({ input: socket });
  // One job at a time, chained, which both matches what the pool sends and
  // keeps replies in request order.
  let chain = Promise.resolve();

  lines.on('line', (line) => {
    const trimmed = line.trim();
    if (!trimmed) return;
    let job;
    try {
      job = JSON.parse(trimmed);
    } catch (_e) {
      return; // an unparseable line is not a job
    }
    chain = chain
      .then(() => runJob(job))
      .then(send)
      .catch((e) => send(harnessFailure(job, Date.now(), String((e && e.stack) || e))));
  });

  lines.on('close', () => {
    // Drain whatever was already accepted before exiting, so a closed
    // connection does not drop work the pool believes is running.
    Promise.resolve(chain).finally(() => process.exit(0));
  });
}

const address = process.env.TINYRUNTIME_PROTOCOL_ADDR;
if (!address) {
  process.stderr.write('tinyruntime: no protocol address was supplied\n');
  process.exit(1);
}

const net = require('net');
const separator = address.lastIndexOf(':');
const socket = net.createConnection({
  host: address.slice(0, separator),
  port: Number(address.slice(separator + 1)),
});
socket.once('connect', () => serve(socket));
socket.once('error', (e) => {
  process.stderr.write('tinyruntime: protocol connection failed: ' + String(e) + '\n');
  process.exit(1);
});
