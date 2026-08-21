//! End-to-end tests for the shipped worker harness, against a real `node`.
//!
//! The harness is the one part of this crate that is not Rust, so nothing else
//! in the suite can check that it actually speaks the protocol. These tests
//! stand in for the router: they listen on loopback, launch the harness the way
//! the router would, complete the handshake, and run jobs through it.
//!
//! They skip when the machine has no `node`, so the suite stays hermetic on a
//! runner without one rather than failing for the wrong reason.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::{Child, Command};

use tinyruntime_nodejs::{WORKER_PROTOCOL_VERSION, harness};

/// How long any single step may take before the test gives up.
const STEP_TIMEOUT: Duration = Duration::from_secs(30);

/// Read and discard a child stream, so the job writing to it never blocks.
fn drain(stream: impl tokio::io::AsyncRead + Send + Unpin + 'static) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(stream).lines();
        while let Ok(Some(_)) = lines.next_line().await {}
    });
}

/// A harness process under test, plus the protocol connection to it.
struct Harness {
    _child: Child,
    writer: tokio::io::WriteHalf<TcpStream>,
    lines: Lines<BufReader<tokio::io::ReadHalf<TcpStream>>>,
    _scratch: tempfile::TempDir,
    cwd: PathBuf,
}

impl Harness {
    /// Launch the harness the way the router would, or `None` without `node`.
    async fn launch() -> Option<Self> {
        let usable = Command::new("node")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .is_ok_and(|status| status.success());
        if !usable {
            eprintln!("skipped: this machine has no usable `node`");
            return None;
        }

        let scratch = tempfile::tempdir().unwrap();
        let harness = harness();
        let script = scratch.path().join(&harness.filename);
        std::fs::write(&script, &harness.source).unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let token = "test-secret-token";

        let mut child = Command::new("node")
            .args(harness.command_args(&script.to_string_lossy()))
            .env("TINYRUNTIME_PROTOCOL_ADDR", address.to_string())
            .env("TINYRUNTIME_PROTOCOL_TOKEN", token)
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();

        let (stream, _) = tokio::time::timeout(STEP_TIMEOUT, listener.accept())
            .await
            .expect("the harness connected back")
            .unwrap();
        let (reader, writer) = tokio::io::split(stream);
        let mut lines = BufReader::new(reader).lines();

        let handshake: serde_json::Value = serde_json::from_str(
            &tokio::time::timeout(STEP_TIMEOUT, lines.next_line())
                .await
                .expect("the handshake arrived")
                .unwrap()
                .expect("the harness sent a handshake"),
        )
        .unwrap();

        assert_eq!(handshake["ready"], serde_json::json!(true));
        assert_eq!(
            handshake["protocol"],
            serde_json::json!(WORKER_PROTOCOL_VERSION)
        );
        assert_eq!(handshake["language"], serde_json::json!("nodejs"));
        assert_eq!(
            handshake["token"],
            serde_json::json!(token),
            "the harness must echo the secret it was given"
        );

        // Drain the child's own file descriptors, exactly as the router does.
        // This is not tidiness: a job that writes to fd 1 gets EPIPE if nothing
        // is reading, and blocks outright once the pipe fills.
        if let Some(stdout) = child.stdout.take() {
            drain(stdout);
        }
        if let Some(stderr) = child.stderr.take() {
            drain(stderr);
        }
        let cwd = scratch.path().to_path_buf();
        Some(Self {
            _child: child,
            writer,
            lines,
            _scratch: scratch,
            cwd,
        })
    }

    /// Run one job and return its reply.
    async fn run(&mut self, id: &str, code: &str, timeout_ms: Option<u64>) -> serde_json::Value {
        let mut request = serde_json::json!({
            "id": id,
            "code": code,
            "cwd": self.cwd.to_string_lossy(),
        });
        if let Some(timeout_ms) = timeout_ms {
            request["timeout_ms"] = serde_json::json!(timeout_ms);
        }

        let mut line = serde_json::to_string(&request).unwrap();
        line.push('\n');
        self.writer.write_all(line.as_bytes()).await.unwrap();
        self.writer.flush().await.unwrap();

        let reply = tokio::time::timeout(STEP_TIMEOUT, self.lines.next_line())
            .await
            .expect("the harness replied")
            .unwrap()
            .expect("the harness sent a reply");
        let reply: serde_json::Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(
            reply["id"],
            serde_json::json!(id),
            "reply was for another job"
        );
        reply
    }
}

#[tokio::test]
async fn the_harness_runs_a_job_and_reports_its_output() {
    let Some(mut harness) = Harness::launch().await else {
        return;
    };
    let reply = harness.run("1", "console.log(6 * 7)", None).await;
    assert_eq!(reply["ok"], serde_json::json!(true));
    assert_eq!(reply["stdout"], serde_json::json!("42\n"));
    assert_eq!(reply["exit_code"], serde_json::json!(0));
}

#[tokio::test]
async fn one_warm_worker_serves_many_jobs() {
    // The entire reason the pool exists. If the harness exited after a job, this
    // would fail on the second one.
    let Some(mut harness) = Harness::launch().await else {
        return;
    };
    for index in 0..3 {
        let reply = harness
            .run(
                &index.to_string(),
                &format!("console.log({index} + 1)"),
                None,
            )
            .await;
        assert_eq!(
            reply["stdout"],
            serde_json::json!(format!("{}\n", index + 1))
        );
    }
}

#[tokio::test]
async fn a_job_that_throws_reports_a_non_zero_exit_without_killing_the_worker() {
    let Some(mut harness) = Harness::launch().await else {
        return;
    };
    let thrown = harness.run("1", "throw new Error('boom')", None).await;
    assert_eq!(
        thrown["ok"],
        serde_json::json!(true),
        "the harness ran it; the job failed"
    );
    assert_ne!(thrown["exit_code"], serde_json::json!(0));
    assert!(thrown["stderr"].as_str().unwrap().contains("boom"));

    // The worker must survive a thrown job, or one bad job would cost a respawn.
    let after = harness.run("2", "console.log('still here')", None).await;
    assert_eq!(after["stdout"], serde_json::json!("still here\n"));
}

#[tokio::test]
async fn a_job_that_exits_the_process_does_not_take_the_worker_down() {
    // Each job runs in its own worker_thread precisely so `process.exit()` kills
    // the thread rather than the long-lived host process.
    let Some(mut harness) = Harness::launch().await else {
        return;
    };
    let exited = harness.run("1", "process.exit(3)", None).await;
    assert_eq!(exited["exit_code"], serde_json::json!(3));

    let after = harness.run("2", "console.log('survived')", None).await;
    assert_eq!(after["stdout"], serde_json::json!("survived\n"));
}

#[tokio::test]
async fn relative_paths_resolve_against_the_job_directory() {
    // A shared warm worker runs wherever the last job left it unless the harness
    // enters each job's directory. Getting this wrong escapes the caller's sandbox.
    let Some(mut harness) = Harness::launch().await else {
        return;
    };
    std::fs::write(harness.cwd.join("probe.txt"), b"RELATIVE_OK").unwrap();

    let reply = harness
        .run(
            "1",
            "process.stdout.write(require('fs').readFileSync('./probe.txt', 'utf8'))",
            None,
        )
        .await;
    assert_eq!(reply["stdout"], serde_json::json!("RELATIVE_OK"));
}

#[tokio::test]
async fn a_job_whose_directory_is_missing_fails_rather_than_running_elsewhere() {
    let Some(mut harness) = Harness::launch().await else {
        return;
    };
    let sentinel = harness.cwd.join("must-not-exist.txt");
    harness.cwd = harness.cwd.join("deleted-sandbox");

    let reply = harness
        .run(
            "1",
            "require('fs').writeFileSync('must-not-exist.txt', 'escaped')",
            None,
        )
        .await;
    assert_eq!(reply["ok"], serde_json::json!(false), "the job ran anyway");
    assert!(
        reply["error"]
            .as_str()
            .unwrap()
            .contains("failed to set worker cwd")
    );
    assert!(!sentinel.exists(), "the job escaped its sandbox");
}

#[tokio::test]
async fn a_job_that_never_finishes_is_aborted_at_its_deadline() {
    let Some(mut harness) = Harness::launch().await else {
        return;
    };
    // A bare unresolved promise would not do: with nothing else pending, the
    // worker's event loop drains and the thread exits on its own, exactly as
    // `node -e` does. A live timer is what actually keeps a job running.
    let reply = harness
        .run(
            "1",
            "await new Promise((resolve) => setTimeout(resolve, 60000))",
            Some(1_000),
        )
        .await;
    assert_eq!(reply["timed_out"], serde_json::json!(true));

    // And the worker is still usable afterwards.
    let after = harness.run("2", "console.log('alive')", None).await;
    assert_eq!(after["stdout"], serde_json::json!("alive\n"));
}

#[tokio::test]
async fn a_job_cannot_forge_a_reply_over_its_own_stdout() {
    // The reason the protocol has its own socket: a job writing a frame-shaped
    // line to fd 1 must not be able to answer its own request.
    let Some(mut harness) = Harness::launch().await else {
        return;
    };
    let reply = harness
        .run(
            "1",
            r"require('fs').writeSync(1, JSON.stringify({ id: '1', ok: true, stdout: 'FORGED' }) + '\n');
console.log('REAL')",
            None,
        )
        .await;

    // The forged frame went to the process's real file descriptor, which the
    // router drains and logs and never parses. What matters is that it did not
    // become this job's reply: the reply is the harness's own, and the job's
    // captured output is only what it wrote through the worker's own stream.
    assert_eq!(reply["ok"], serde_json::json!(true), "reply was {reply}");
    assert_eq!(
        reply["stdout"],
        serde_json::json!("REAL\n"),
        "a job's fd-level write reached the protocol reply: {reply}"
    );
    assert_eq!(reply["exit_code"], serde_json::json!(0));
}

#[tokio::test]
async fn top_level_await_and_dynamic_import_work_like_node_dash_e() {
    // Both depend on the flags the harness ships with; without them this fails
    // at runtime rather than at launch.
    let Some(mut harness) = Harness::launch().await else {
        return;
    };
    let reply = harness
        .run(
            "1",
            "const fs = await import('fs'); console.log(typeof fs.readFileSync)",
            None,
        )
        .await;
    assert_eq!(reply["stdout"], serde_json::json!("function\n"));
}

#[tokio::test]
async fn a_job_reads_end_of_file_on_standard_input() {
    // Standard input must never be the protocol stream, or a job that read it
    // would consume the next request.
    let Some(mut harness) = Harness::launch().await else {
        return;
    };
    let reply = harness
        .run(
            "1",
            "console.log(JSON.stringify(require('fs').readFileSync(0, 'utf8')))",
            Some(5_000),
        )
        .await;
    assert_eq!(reply["stdout"], serde_json::json!("\"\"\n"));
}
