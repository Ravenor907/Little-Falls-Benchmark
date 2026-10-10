//! Little Falls core: CPU and memory benchmarks, system detection,
//! scoring and the results file format.
//!
//! This crate deliberately has zero external dependencies so the
//! measurement code stays small, auditable and fast to build.

pub mod cpu;
pub mod json;
pub mod memory;
pub mod results;
pub mod score;
pub mod sha256;
pub mod sysinfo;

use std::time::{Duration, Instant};

/// Result of one timed workload.
#[derive(Clone, Debug)]
pub struct Measurement {
    /// Stable identifier, e.g. `cpu.single.sieve`.
    pub id: &'static str,
    /// Human readable name.
    pub name: &'static str,
    /// Measured value (higher is better unless `lower_is_better`).
    pub value: f64,
    /// Unit of `value`, e.g. `Mops/s`, `GB/s`, `ns`.
    pub unit: &'static str,
    pub lower_is_better: bool,
    /// Workload output checksum. Used to detect broken or tampered runs.
    pub check: u64,
    /// Relative spread between the fastest and slowest repeat
    /// ((slowest - fastest) / fastest). High values mean background load
    /// or thermal throttling disturbed the measurement.
    pub spread: f64,
}

/// Runs `f` `reps` times and returns the fastest wall time, the check
/// value of the last run, and the relative spread between the fastest and
/// slowest runs. Taking the best run filters out noise from background
/// tasks; the spread reports how much noise there was.
pub fn best_of<F: FnMut() -> u64>(reps: u32, mut f: F) -> (Duration, u64, f64) {
    let mut best = Duration::MAX;
    let mut worst = Duration::ZERO;
    let mut check = 0;
    for _ in 0..reps.max(1) {
        let t = Instant::now();
        check = std::hint::black_box(f());
        let el = t.elapsed();
        best = best.min(el);
        worst = worst.max(el);
    }
    let spread = (worst.as_secs_f64() - best.as_secs_f64()) / best.as_secs_f64().max(1e-9);
    (best, check, spread)
}

/// Number of worker threads to use for multi-threaded tests.
pub fn thread_count() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
}
