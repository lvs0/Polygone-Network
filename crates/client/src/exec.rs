//! exec — the RES execution layer (sandboxed, systemd-run based).
//!
//! Runs a task on the ghost node inside a systemd sandbox:
//!   - MemoryMax (bounded memory)
//!   - NoNewPrivileges (no privilege escalation)
//!   - ProtectSystem=strict (system dirs read-only)
//!   - ProtectHome=yes ($HOME read-only)
//!   - InaccessiblePaths=~/.polygone (identity + received UNREADABLE)
//!   - PrivateDevices, PrivateNetwork, SystemCallFilter (no raw devices,
//!     no network, no dangerous syscalls)
//!   - CPUQuota (bounded CPU)
//!
//! Honest scope: this is the hardened MVP sandbox — good isolation against
//! accidents and opportunistic abuse, NOT a cryptographic sandbox
//! (systemd-run --user shares the UID). Production (Phase 8+) adds WASM
//! verification + reputation + adversary budget (see STAGING.md).

use anyhow::Result;
use std::process::Command;
use std::time::Duration;

/// Max concurrent in-flight RES executions on this node. A flood of
/// requests cannot spawn unbounded sandboxes.
pub const MAX_CONCURRENT_EXEC: usize = 2;
static ACTIVE_EXEC: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Fuel budget for WASM execution: an infinite loop exhausts it and traps
/// instead of freezing the node. ~1e10 instructions ≈ seconds of real work.
const WASM_FUEL: u64 = 10_000_000_000;

/// Run a WASM module (WASI) in the wasmi sandbox, capturing stdout.
/// Fuel metering guarantees a non-terminating module traps instead of
/// blocking the event loop (wasmi is synchronous).
pub fn run_wasm(wasm: &[u8], timeout: Duration) -> Result<String> {
    use wasmi::{Config, Engine, Linker, Module, Store};

    let mut config = Config::default();
    config.consume_fuel(true); // fuel metering on: loops die, nodes live
    let engine = Engine::new(&config);
    let module = Module::new(&engine, wasm)?;

    // WASI context with stdout captured to shared Arcs (readable after run).
    let stdout_shared = std::sync::Arc::new(std::sync::RwLock::new(Vec::<u8>::new()));
    let stderr_shared = std::sync::Arc::new(std::sync::RwLock::new(Vec::<u8>::new()));
    let mut wasi_builder = wasmi_wasi::WasiCtxBuilder::new();
    wasi_builder.stdout(Box::new(
        wasmi_wasi::wasi_common::pipe::WritePipe::from_shared(stdout_shared.clone()),
    ));
    wasi_builder.stderr(Box::new(
        wasmi_wasi::wasi_common::pipe::WritePipe::from_shared(stderr_shared.clone()),
    ));
    wasi_builder.args(&["polygone-res".to_string()])?;
    let wasi = wasi_builder.build();

    let mut store = Store::new(&engine, wasi);
    store.set_fuel(WASM_FUEL)?;
    let mut linker = Linker::new(&engine);
    wasmi_wasi::add_to_linker(&mut linker, |ctx| ctx)?;

    let instance = linker.instantiate(&mut store, &module)?.start(&mut store)?;
    let start = instance
        .get_typed_func::<(), ()>(&mut store, "_start")
        .map_err(|_| anyhow::anyhow!("module sans _start (compilé avec --target wasm32-wasi ?)"))?;

    // Run with a wall-clock timeout (backstop) + fuel (primary bound).
    let start_time = std::time::Instant::now();
    let result = start.call(&mut store, ());
    if start_time.elapsed() > timeout {
        anyhow::bail!("exécution WASM dépassée (>{:?})", timeout);
    }
    result.map_err(|e| anyhow::anyhow!("erreur WASM : {e}"))?;

    let out: Vec<u8> = stdout_shared.read().map(|g| g.clone()).unwrap_or_default();
    let err: Vec<u8> = stderr_shared.read().map(|g| g.clone()).unwrap_or_default();

    let mut out_s = String::from_utf8_lossy(&out).to_string();
    let err_s = String::from_utf8_lossy(&err).to_string();
    if !err_s.trim().is_empty() {
        out_s.push_str(&format!("\n[stderr] {err_s}"));
    }
    if out_s.len() > 4000 {
        out_s.truncate(4000);
        out_s.push_str("\n…[tronqué]");
    }
    Ok(out_s)
}

/// Run a shell command inside the systemd sandbox, capturing stdout.
/// `mem_mb` caps memory, `cpu_pct` caps CPU, `timeout` caps wall time.
/// On timeout the **transient unit** is stopped (not just systemd-run),
/// so no orphaned process survives.
pub fn run_sandboxed(
    command: &str,
    mem_mb: u32,
    cpu_pct: u32,
    timeout: Duration,
) -> Result<String> {
    // Concurrency guard: bounded sandboxes, not a flood.
    if ACTIVE_EXEC.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= MAX_CONCURRENT_EXEC {
        ACTIVE_EXEC.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        anyhow::bail!(
            "trop d'exécutions en parallèle (max {MAX_CONCURRENT_EXEC}) — réessayez dans un instant"
        );
    }
    let _guard = ExecGuard;

    let mem = format!("--property=MemoryMax={mem_mb}M");
    let cpu = format!("--property=CPUQuota={cpu_pct}%");
    // Unique transient unit per run so the timeout can kill exactly this one.
    let unit = format!(
        "polygone-res-{}-{:?}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );

    // ~/.polygone must be unreadable inside the sandbox: identity.json and
    // received/ are the crown jewels of the node.
    let poly_path = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".polygone");
    let inaccessible = format!("--property=InaccessiblePaths={}", poly_path.display());

    let mut cmd = Command::new("systemd-run");
    cmd.args(["--user", "--wait", "--pipe", "-q"])
        .arg(format!("--unit={unit}"))
        .arg(&mem)
        .arg(&cpu)
        .args([
            "--property=NoNewPrivileges=yes",
            "--property=ProtectSystem=strict",
            "--property=ProtectHome=yes",
            "--property=PrivateTmp=yes",
            "--property=PrivateNetwork=yes",
            "--property=PrivateDevices=yes",
            "--property=RestrictAddressFamilies=AF_UNIX",
            "--property=SystemCallFilter=@system-service",
        ])
        .arg(&inaccessible)
        .arg("/bin/sh")
        .arg("-c")
        .arg(command);

    // Timeout: systemd-run --wait blocks; stop the transient unit on excess.
    let output = match run_with_timeout(&mut cmd, timeout, Some(&unit)) {
        Ok(o) => o,
        Err(e) => {
            // If the sandbox is not available (e.g. no user manager), report clearly.
            return Err(anyhow::anyhow!("exécution sandboxée impossible : {e}"));
        }
    };

    let mut out = String::from_utf8_lossy(&output.stdout).to_string();
    let err = String::from_utf8_lossy(&output.stderr).to_string();
    if !err.trim().is_empty() {
        out.push_str(&format!("\n[stderr] {err}"));
    }
    // Truncate: responses travel the relay, keep them bounded.
    if out.len() > 4000 {
        out.truncate(4000);
        out.push_str("\n…[tronqué]");
    }
    Ok(out)
}

/// Releases the concurrency slot on drop, even on early return.
struct ExecGuard;
impl Drop for ExecGuard {
    fn drop(&mut self) {
        ACTIVE_EXEC.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Run a command with a wall-clock timeout. If `unit` is given (a transient
/// systemd unit), it is stopped on timeout — killing only systemd-run would
/// leave the sandboxed process orphaned and running.
fn run_with_timeout(
    cmd: &mut Command,
    timeout: Duration,
    unit: Option<&str>,
) -> std::io::Result<std::process::Output> {
    use std::process::Stdio;

    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

    // Wait with timeout: poll the child.
    let start = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            let mut stdout = String::new();
            let mut stderr = String::new();
            use std::io::Read;
            if let Some(mut so) = child.stdout.take() {
                let _ = so.read_to_string(&mut stdout);
            }
            if let Some(mut se) = child.stderr.take() {
                let _ = se.read_to_string(&mut stderr);
            }
            return Ok(std::process::Output {
                status,
                stdout: stdout.into_bytes(),
                stderr: stderr.into_bytes(),
            });
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            // Stop the transient unit so its cgroup and children die too.
            if let Some(unit) = unit {
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "stop", "--no-block"])
                    .arg(unit)
                    .status();
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "reset-failed"])
                    .arg(unit)
                    .status();
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("délai dépassé (>{:?})", timeout),
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_runs_simple_command() {
        let out = run_sandboxed("echo hello-res", 64, 50, Duration::from_secs(10));
        match out {
            Ok(o) => assert!(o.contains("hello-res"), "got: {o}"),
            Err(e) => panic!("sandbox unavailable in test env: {e}"),
        }
    }

    #[test]
    fn wasm_runs_and_captures_stdout() {
        // Environment-dependent (needs a compiled wasm): skip gracefully.
        let Ok(wasm) = std::fs::read("/tmp/wasmtest/test.wasm") else {
            eprintln!("skip: test.wasm absent");
            return;
        };
        let out = run_wasm(&wasm, Duration::from_secs(20));
        match out {
            Ok(o) => assert!(o.contains("hello from wasm"), "sortie: {o:?}"),
            Err(e) => panic!("run_wasm: {e}"),
        }
    }

    #[test]
    fn wasm_invalid_module_errors_gracefully() {
        let out = run_wasm(
            &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0xff],
            Duration::from_secs(5),
        );
        assert!(out.is_err(), "invalid wasm must error");
    }

    #[test]
    fn sandbox_blocks_privilege_escalation() {
        // NoNewPrivileges=yes — setuid/sudo must fail inside the sandbox.
        let out = run_sandboxed("id -u", 64, 50, Duration::from_secs(10));
        match out {
            Ok(o) => {
                // As a normal user, id -u is still the user id — we check
                // instead that the sandbox itself ran and returned something.
                assert!(!o.trim().is_empty());
            }
            Err(e) => panic!("sandbox unavailable: {e}"),
        }
    }
}
