//! Linux system detection from /proc and /etc. GPU details are filled in
//! by the graphics app, which gets them from the Vulkan adapter.

use std::fs;

#[derive(Clone, Debug, Default)]
pub struct SystemInfo {
    pub cpu_model: String,
    pub cpu_threads: usize,
    pub ram_mib: u64,
    pub os_name: String,
    pub os_id: String,
    pub kernel: String,
    pub gpu_name: String,
    pub gpu_driver: String,
    pub gpu_backend: String,
    /// True when running under Valve's gamescope compositor (Steam Deck
    /// game mode, SteamOS, Bazzite game mode).
    pub gamescope: bool,
}

impl SystemInfo {
    pub fn detect() -> Self {
        let cpuinfo = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
        let cpu_model = cpuinfo
            .lines()
            .find(|l| l.starts_with("model name"))
            .and_then(|l| l.split_once(':'))
            .map(|(_, v)| v.trim().to_string())
            .unwrap_or_else(|| "Unknown CPU".into());

        let meminfo = fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let ram_kib: u64 = meminfo
            .lines()
            .find(|l| l.starts_with("MemTotal:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let os = fs::read_to_string("/etc/os-release").unwrap_or_default();
        let field = |k: &str| {
            os.lines()
                .find_map(|l| l.strip_prefix(&format!("{k}=")))
                .map(|v| v.trim_matches('"').to_string())
        };

        Self {
            cpu_model,
            cpu_threads: crate::thread_count(),
            ram_mib: ram_kib / 1024,
            os_name: field("PRETTY_NAME").unwrap_or_else(|| "Linux".into()),
            os_id: field("ID").unwrap_or_else(|| "linux".into()),
            kernel: fs::read_to_string("/proc/sys/kernel/osrelease")
                .map(|s| s.trim().to_string())
                .unwrap_or_default(),
            gamescope: std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some()
                || std::env::var("XDG_CURRENT_DESKTOP")
                    .map(|v| v.eq_ignore_ascii_case("gamescope"))
                    .unwrap_or(false),
            ..Default::default()
        }
    }
}
