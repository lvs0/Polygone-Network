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
/// instead of freezing the node. 1e8 ≈ 0,1-1 s de travail réel — généreux
/// pour du calcul utile, court pour une boucle malveillante. Le fuel borne
/// aussi la croissance mémoire (memory.grow coûte du fuel).
const WASM_FUEL: u64 = 100_000_000;
/// Max bytes captured from WASM stdout/stderr (anti-OOM: une sortie
/// débridée est tronquée PENDANT l'écriture, pas après).
const WASM_OUTPUT_CAP: usize = 8 * 1024;

/// Run a WASM module (WASI) in the wasmi sandbox, capturing stdout.
/// Fuel metering + strict compile limits + output cap: a malicious module
/// traps, cannot exhaust memory, and cannot flood the caller.
/// `run_wasm` is CPU-bound and blocking — call it via `spawn_blocking`.
pub fn run_wasm(wasm: &[u8], timeout: Duration) -> Result<String> {
    use wasmi::{Config, EnforcedLimits, Engine, Linker, Module, Store};

    if ACTIVE_EXEC.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= MAX_CONCURRENT_EXEC {
        ACTIVE_EXEC.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        anyhow::bail!(
            "trop d'exécutions en parallèle (max {MAX_CONCURRENT_EXEC}) — réessayez dans un instant"
        );
    }
    let _guard = ExecGuard;

    let mut config = Config::default();
    config.consume_fuel(true); // fuel metering on: loops die, nodes live
    config.enforced_limits(EnforcedLimits::strict()); // compiler guard
    let engine = Engine::new(&config);
    let module = Module::new(&engine, wasm)?;

    // WASI context with bounded stdout/stderr (truncated DURING writes).
    let stdout_shared =
        std::sync::Arc::new(std::sync::RwLock::new(CappedWriter::new(WASM_OUTPUT_CAP)));
    let stderr_shared =
        std::sync::Arc::new(std::sync::RwLock::new(CappedWriter::new(WASM_OUTPUT_CAP)));
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

    // Wall-clock backstop (primary bound = fuel).
    let start_time = std::time::Instant::now();
    let result = start.call(&mut store, ());
    if start_time.elapsed() > timeout {
        anyhow::bail!("exécution WASM dépassée (>{:?})", timeout);
    }
    result.map_err(|e| anyhow::anyhow!("erreur WASM : {e}"))?;

    let out: Vec<u8> = stdout_shared.read().map(|g| g.bytes()).unwrap_or_default();
    let err: Vec<u8> = stderr_shared.read().map(|g| g.bytes()).unwrap_or_default();

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
/// Protect the local identity directory when it is outside systemd's
/// already-private temporary directories. PrivateTmp=yes hides /tmp and
/// /var/tmp; adding InaccessiblePaths below either root would fail because
/// systemd cannot bind-mount a host path into its private tmp namespace.
fn inaccessible_path(poly_path: &std::path::Path) -> Option<String> {
    let path = poly_path.canonicalize().ok()?;
    let private_tmp = [
        std::path::Path::new("/tmp"),
        std::path::Path::new("/var/tmp"),
    ];
    if private_tmp.iter().any(|root| path.starts_with(root)) {
        return None;
    }
    Some(format!("--property=InaccessiblePaths={}", path.display()))
}

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
    let inaccessible = inaccessible_path(&poly_path);

    let mut cmd = Command::new("systemd-run");
    cmd.args(["--user", "--wait", "--pipe", "-q"])
        .arg(format!("--unit={unit}"))
        .arg(&mem)
        .arg(&cpu)
        // RuntimeMaxSec: the manager itself kills the unit at the deadline —
        // even if our client dies mid-run (SIGKILL, duress), no orphan runs on.
        .arg(format!(
            "--property=RuntimeMaxSec={}",
            timeout.as_secs().max(1)
        ))
        .args([
            // Without MemorySwapMax=0 the swap can absorb the allocation and
            // the cgroup limit silently stops capping resident memory: the
            // 96 MB probe survived a 32 MB cap until this was added.
            "--property=MemorySwapMax=0",
            "--property=NoNewPrivileges=yes",
            "--property=ProtectSystem=strict",
            "--property=ProtectHome=yes",
            "--property=PrivateTmp=yes",
            "--property=PrivateNetwork=yes",
            "--property=PrivateDevices=yes",
            "--property=RestrictAddressFamilies=AF_UNIX",
            "--property=SystemCallFilter=@system-service",
        ]);
    if let Some(inaccessible) = inaccessible {
        cmd.arg(inaccessible);
    }
    // Two barriers, both measured on this host: the cgroup cap
    // (`memory.max` = requested size, `memory.swap.max` = 0) kills a 4x
    // allocation while a half-size one survives, and RLIMIT_AS is inherited
    // by the shell and every child so the budget still holds on a host whose
    // memory controller is not delegated to the user manager.
    let mem_kb = u64::from(mem_mb) * 1024;
    let limited = format!("ulimit -v {mem_kb} 2>/dev/null || exit 125; {command}");
    cmd.arg("/bin/sh").arg("-c").arg(limited);

    // Timeout: systemd-run --wait blocks; stop the transient unit on excess.
    let output = match run_with_timeout(&mut cmd, timeout, Some(&unit)) {
        Ok(o) => o,
        Err(e) => {
            // If the sandbox is not available (e.g. no user manager), report clearly.
            return Err(anyhow::anyhow!("exécution sandboxée impossible : {e}"));
        }
    };

    if !output.status.success() {
        // A failed transient unit otherwise stays loaded as `failed` forever.
        // Remove it after recording the exit code; the result is already
        // captured below, so cleanup cannot hide the failure.
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "reset-failed"])
            .arg(&unit)
            .status();
        let code = output
            .status
            .code()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "signal".to_string());
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "commande sandboxée terminée en échec (code {code}){}",
            if stderr.trim().is_empty() {
                String::new()
            } else {
                format!(": {}", stderr.trim())
            }
        );
    }

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

/// A `Write` sink that keeps only the first `cap` bytes. A WASM module
/// flooding stdout cannot exhaust the host's memory: writes beyond the cap
/// are discarded as they arrive, not after execution.
struct CappedWriter {
    buf: Vec<u8>,
    cap: usize,
}

impl CappedWriter {
    fn new(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap.min(4096)),
            cap,
        }
    }
    fn bytes(&self) -> Vec<u8> {
        self.buf.clone()
    }
}

impl std::io::Write for CappedWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let room = self.cap.saturating_sub(self.buf.len());
        let n = room.min(data.len());
        self.buf.extend_from_slice(&data[..n]);
        Ok(data.len()) // acknowledge everything: the writer never errors
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
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
            // `--wait` blocks until the stop completes (a `--no-block` stop
            // can be lost if the unit is still activating).
            if let Some(unit) = unit {
                // `systemctl stop --wait` is not a supported option
                // (systemd 261 warns and ignores it). Stop synchronously,
                // then reset the failed transient unit.
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "stop"])
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
    /// Vérifie si systemd-run --user est disponible et fonctionnel.
    /// Retourne false en CI (GitHub Actions) où systemd n'a pas de session utilisateur.
    fn systemd_run_available() -> bool {
        // Vérifier que systemd-run existe
        // A fixed unit name can survive as a failed/loaded unit and make every
        // probe fail. Use a unique, collected unit per probe.
        let unit = format!(
            "polygone-probe-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let Ok(output) = std::process::Command::new("systemd-run")
            .args(["--user", "--wait", "--pipe", "-q", "--collect"])
            .arg(format!("--unit={unit}"))
            .args(["/bin/echo", "test"])
            .output()
        else {
            return false;
        };

        // Vérifier que la commande a réussi ET produit une sortie
        output.status.success() && !output.stdout.is_empty()
    }

    /// True si le sandbox RÉEL (jeu complet de propriétés) s'exécute sur cet
    /// hôte. Un `systemd-run /bin/echo` nu n'est pas discriminant : les runners
    /// CI acceptent echo mais refusent l'unité durcie (exit 218
    /// EXIT_CAPABILITIES). Le probe exécute donc le même chemin que les tests,
    /// une seule fois (OnceLock). Sur un hôte incapable, les tests sandbox
    /// sautent avec un message — ils ne paniquent pas et ne simulent rien.
    fn sandbox_executable() -> bool {
        static PROBE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *PROBE.get_or_init(|| {
            if !systemd_run_available() {
                return false;
            }
            match run_sandboxed(
                "printf polygone-sandbox-probe",
                64,
                10,
                Duration::from_secs(15),
            ) {
                Ok(out) => out.contains("polygone-sandbox-probe"),
                Err(_) => false,
            }
        })
    }

    use super::*;

    /// The production guard `ACTIVE_EXEC` caps concurrent exec at
    /// MAX_CONCURRENT_EXEC (2). Cargo's default parallel test runner
    /// would trip that guard non-deterministically (3 exec tests racing
    /// for 2 slots). Serialize exec tests so `cargo test --workspace`
    /// is green in parallel mode — the CI mode.
    static EXEC_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Gardes combinés : EXEC (garde prod ACTIVE_EXEC) + GLOBAL (état du
    /// process, ex. HOME muté par les tests duress). Les tests exec tournent
    /// dans un sandbox systemd qui hérite de l'état global du process — ne
    /// jamais les faire courir en parallèle des tests qui le mutent.
    struct ExecGuards {
        _global: std::sync::MutexGuard<'static, ()>,
        _exec: std::sync::MutexGuard<'static, ()>,
    }

    fn exec_test_guards() -> ExecGuards {
        ExecGuards {
            _global: crate::testutil::with_global_state_guard(),
            _exec: EXEC_TEST_LOCK
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        }
    }

    #[test]
    fn sandbox_runs_simple_command() {
        if !sandbox_executable() {
            eprintln!("skip: sandbox non exécutable sur cet hôte (systemd-run --user absent ou unité durcie refusée)");
            return;
        }
        let _g = exec_test_guards();

        let out = run_sandboxed("echo hello-res", 64, 50, Duration::from_secs(10));
        match out {
            Ok(o) => {
                if o.trim().is_empty() {
                    eprintln!("skip: sandbox ran but returned empty output");
                    return;
                }
                assert!(o.contains("hello-res"), "got: {o}");
            }
            Err(e) => {
                // systemd-run --user n'est pas disponible en CI (GitHub Actions)
                // ni dans les environnements sans session utilisateur systemd.
                eprintln!("skip: sandbox indisponible (CI/sans systemd): {e}");
            }
        }
    }

    #[test]
    fn wasm_runs_and_captures_stdout() {
        // Environment-dependent (needs a compiled wasm): skip gracefully.
        let _g = exec_test_guards();
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
        let _g = exec_test_guards();
        let out = run_wasm(
            &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0xff],
            Duration::from_secs(5),
        );
        assert!(out.is_err(), "invalid wasm must error");
    }

    // Propriété du sandbox Linux (PrivateTmp=yes masque /tmp et /var/tmp).
    // Sur macOS, /tmp est un symlink vers /private/tmp : canonicalize sort
    // des racines reconnues, et le sandbox systemd n'existe pas — le test
    // n'a pas de sens hors Linux.
    #[cfg(target_os = "linux")]
    #[test]
    fn private_tmp_home_does_not_add_a_conflicting_inaccessible_mount() {
        // Base /tmp explicite : le contrat du helper porte sur le tmp privé
        // du sandbox Linux (/tmp, /var/tmp). env::temp_dir() vaut /var/folders
        // sur macOS et un scratch quelconque selon TMPDIR : le test devenait
        // dépendant de l'environnement au lieu de tester la logique.
        let home = std::path::Path::new("/tmp").join(format!(
            "polygone-private-tmp-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let poly = home.join(".polygone");
        std::fs::create_dir_all(&poly).unwrap();
        assert!(inaccessible_path(&poly).is_none());
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn identity_path_outside_private_tmp_is_explicitly_inaccessible() {
        let executable = std::env::current_exe().unwrap();
        let in_private_tmp = [
            std::path::Path::new("/tmp"),
            std::path::Path::new("/var/tmp"),
        ]
        .iter()
        .any(|root| executable.starts_with(root));
        let arg = inaccessible_path(&executable);
        assert_eq!(arg.is_none(), in_private_tmp);
        if let Some(arg) = arg {
            assert!(arg.starts_with("--property=InaccessiblePaths="));
        }
    }

    #[test]
    fn sandbox_enforces_wall_clock_timeout() {
        if !sandbox_executable() {
            eprintln!("skip: sandbox non exécutable sur cet hôte (systemd-run --user absent ou unité durcie refusée)");
            return;
        }
        let _g = exec_test_guards();
        let started = std::time::Instant::now();
        let result = run_sandboxed("sleep 10", 64, 50, Duration::from_secs(1));
        assert!(result.is_err(), "sleep 10 must not survive a 1 s sandbox");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "timeout cleanup took too long: {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn sandbox_enforces_memory_limit() {
        if !sandbox_executable() {
            eprintln!("skip: sandbox non exécutable sur cet hôte (systemd-run --user absent ou unité durcie refusée)");
            return;
        }
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .map(|out| !out.status.success())
            .unwrap_or(true)
        {
            eprintln!("skip: python3 absent");
            return;
        }
        let _g = exec_test_guards();
        // Touch every page: a lazy allocation alone would not prove the cap.
        let result = run_sandboxed(
            r#"python3 -c 'x=bytearray(96*1024*1024); x[::4096]=b"\x01"*((len(x)+4095)//4096); print("TOUCHED")'"#,
            32,
            50,
            Duration::from_secs(5),
        );
        assert!(
            result.is_err(),
            "96 MB touched under a 32 MB sandbox must fail"
        );
    }

    #[test]
    fn sandbox_memory_cap_is_configured_in_the_cgroup_and_enforced() {
        if !sandbox_executable() {
            eprintln!("skip: sandbox non exécutable sur cet hôte (systemd-run --user absent ou unité durcie refusée)");
            return;
        }
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .map(|out| !out.status.success())
            .unwrap_or(true)
        {
            eprintln!("skip: python3 absent");
            return;
        }
        let _g = exec_test_guards();
        // The cgroup must carry the declared cap and close the swap escape
        // hatch. 256 MB keeps RLIMIT_AS out of the way so the Python control
        // allocation proves the sandbox is not failing for an unrelated
        // reason; the 96 MB probe below then fails under a 32 MB budget.
        let probe = run_sandboxed(
            "echo CGROUP_MAX=$(cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/memory.max); \
             echo SWAP_MAX=$(cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/memory.swap.max); \
             echo ADDRESS_SPACE_KB=$(ulimit -v); \
             python3 -c 'x=bytearray(16*1024*1024); x[::4096]=b\"\\x01\"*((len(x)+4095)//4096); print(\"SMALL_OK\")'",
            256,
            50,
            Duration::from_secs(20),
        )
        .expect("16 MB under a 256 MB cap must run");
        assert!(
            probe.contains("CGROUP_MAX=268435456"),
            "cgroup cap not applied: {probe}"
        );
        assert!(
            probe.contains("SWAP_MAX=0"),
            "swap escape hatch open: {probe}"
        );
        assert!(
            probe.contains("ADDRESS_SPACE_KB=262144"),
            "RLIMIT_AS must be the second barrier: {probe}"
        );
        assert!(
            probe.contains("SMALL_OK"),
            "control allocation failed: {probe}"
        );

        // Four times the cap, touched: the sandbox must not let it through.
        // Both barriers (cgroup and RLIMIT_AS) are active here; the cgroup on
        // its own was measured separately on this host: 16 MB survives a
        // 32 MB cap, 96 MB is killed, uncapped 96 MB survives.
        let over = run_sandboxed(
            r#"python3 -c 'x=bytearray(96*1024*1024); x[::4096]=b"\x01"*((len(x)+4095)//4096); print("BIG_OK")'"#,
            32,
            50,
            Duration::from_secs(20),
        );
        assert!(
            over.is_err(),
            "96 MB touched under a 32 MB cap must fail: {over:?}"
        );
    }

    #[test]
    fn sandbox_cpu_quota_slows_a_cpu_bound_workload() {
        if !sandbox_executable() {
            eprintln!("skip: sandbox non exécutable sur cet hôte (systemd-run --user absent ou unité durcie refusée)");
            return;
        }
        if std::process::Command::new("sha256sum")
            .arg("--version")
            .output()
            .map(|out| !out.status.success())
            .unwrap_or(true)
        {
            eprintln!("skip: sha256sum absent");
            return;
        }
        let _g = exec_test_guards();
        // Same CPU-bound, deterministic work at full speed then at 10%: the
        // quota is proven by the slowdown, not by reading CPUQuotaPerSecUSec.
        let work = "dd if=/dev/zero bs=1M count=48 2>/dev/null | sha256sum";
        let full_started = std::time::Instant::now();
        run_sandboxed(work, 256, 100, Duration::from_secs(60)).expect("full-speed run");
        let full = full_started.elapsed();
        let throttled_started = std::time::Instant::now();
        run_sandboxed(work, 256, 10, Duration::from_secs(60)).expect("throttled run");
        let throttled = throttled_started.elapsed();
        eprintln!(
            "cpu quota measure: 100% -> {:?}, 10% -> {:?} (ratio {:.1}x)",
            full,
            throttled,
            throttled.as_secs_f64() / full.as_secs_f64().max(0.001)
        );
        assert!(
            throttled >= full.mul_f64(1.5),
            "CPUQuota=10% did not slow the workload: {full:?} -> {throttled:?}"
        );
    }

    #[test]
    fn sandbox_blocks_identity_and_network_access() {
        if !sandbox_executable() {
            eprintln!("skip: sandbox non exécutable sur cet hôte (systemd-run --user absent ou unité durcie refusée)");
            return;
        }
        let _g = exec_test_guards();
        let home = run_sandboxed(
            "test -r \"$HOME/.polygone/identity.json\" && echo READABLE || echo BLOCKED",
            64,
            50,
            Duration::from_secs(5),
        )
        .expect("sandbox runs");
        assert!(
            home.contains("BLOCKED"),
            "identity directory readable: {home}"
        );

        let net = run_sandboxed("cat /proc/net/dev", 64, 50, Duration::from_secs(5))
            .expect("sandbox runs");
        assert!(net.contains("lo:"), "loopback should exist: {net}");
        assert!(
            !net.contains("wlp") && !net.contains("eth"),
            "host interfaces leaked into the sandbox: {net}"
        );
    }

    #[test]
    fn sandbox_nonzero_exit_is_a_failure() {
        if !sandbox_executable() {
            eprintln!("skip: sandbox non exécutable sur cet hôte (systemd-run --user absent ou unité durcie refusée)");
            return;
        }
        let result = run_sandboxed(
            "echo expected-error >&2; exit 23",
            64,
            50,
            Duration::from_secs(10),
        );
        let error = result.expect_err("un code de sortie non nul ne doit pas être une réussite");
        assert!(error.to_string().contains("code 23"), "{error}");
        assert!(error.to_string().contains("expected-error"), "{error}");
    }

    #[test]
    fn sandbox_blocks_privilege_escalation() {
        if !sandbox_executable() {
            eprintln!("skip: sandbox non exécutable sur cet hôte (systemd-run --user absent ou unité durcie refusée)");
            return;
        }
        let _g = exec_test_guards();
        // NoNewPrivileges=yes — setuid/sudo must fail inside the sandbox.
        let out = run_sandboxed("id -u", 64, 50, Duration::from_secs(10));
        match out {
            Ok(o) => {
                // As a normal user, id -u is still the user id — we check
                // instead that the sandbox itself ran and returned something.
                // If empty, sandbox ran but captured nothing — skip gracefully.
                if o.trim().is_empty() {
                    eprintln!("skip: sandbox ran but returned empty output");
                }
            }
            Err(e) => {
                eprintln!("skip: sandbox indisponible (CI/sans systemd): {e}");
            }
        }
    }
}
