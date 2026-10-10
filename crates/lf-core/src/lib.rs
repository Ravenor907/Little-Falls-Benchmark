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
}

/// Runs `f` `reps` times and returns the fastest wall time plus the
/// check value of the last run. Taking the best run filters out noise
/// from background tasks.
pub fn best_of<F: FnMut() -> u64>(reps: u32, mut f: F) -> (Duration, u64) {
    let mut best = Duration::MAX;
    let mut check = 0;
    for _ in 0..reps.max(1) {
        let t = Instant::now();
        check = std::hint::black_box(f());
        let el = t.elapsed();
        if el < best {
            best = el;
        }
    }
    (best, check)
}

/// Number of worker threads to use for multi-threaded tests.
pub fn thread_count() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
}
