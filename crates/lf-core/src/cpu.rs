//! CPU workloads.
//!
//! Each workload does a fixed, deterministic amount of work and returns a
//! checksum of its output, so a broken or shortcut run can be detected.
//! The same workloads run single-threaded and on every hardware thread.

use crate::{best_of, sha256, thread_count, Measurement};

/// Work size multipliers. `Quick` exists for tests and CI smoke runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    Quick,
    Full,
}

impl Size {
    fn scale(self) -> usize {
        match self {
            Size::Quick => 1,
            Size::Full => 8,
        }
    }
}

// ---------------------------------------------------------------- sieve

/// Sieve of Eratosthenes over a bit array. Integer and branch heavy,
/// working set fits in cache.
pub fn sieve(limit: usize) -> u64 {
    let words = limit / 64 + 1;
    let mut bits = vec![0u64; words]; // 1 = composite
    let mut i = 2;
    while i * i <= limit {
        if bits[i / 64] & (1 << (i % 64)) == 0 {
            let mut j = i * i;
            while j <= limit {
                bits[j / 64] |= 1 << (j % 64);
                j += i;
            }
        }
        i += 1;
    }
    let mut count = 0u64;
    let mut sum = 0u64;
    for n in 2..=limit {
        if bits[n / 64] & (1 << (n % 64)) == 0 {
            count += 1;
            sum = sum.wrapping_add(n as u64);
        }
    }
    count ^ sum.rotate_left(17)
}

// ---------------------------------------------------------------- n-body

/// Gravitational n-body simulation. Floating point and sqrt heavy, the
/// same kind of math game physics does.
pub fn nbody(bodies: usize, steps: usize) -> u64 {
    let mut pos: Vec<[f64; 3]> = Vec::with_capacity(bodies);
    let mut vel: Vec<[f64; 3]> = vec![[0.0; 3]; bodies];
    let mut mass: Vec<f64> = Vec::with_capacity(bodies);
    // Deterministic pseudo random initial state.
    let mut rng = Lcg(0x5EED_1234);
    for _ in 0..bodies {
        pos.push([rng.f64() * 10.0, rng.f64() * 10.0, rng.f64() * 10.0]);
        mass.push(0.5 + rng.f64());
    }
    let dt = 0.001;
    for _ in 0..steps {
        for i in 0..bodies {
            let mut acc = [0.0f64; 3];
            let pi = pos[i];
            for j in 0..bodies {
                if i == j {
                    continue;
                }
                let d = [pos[j][0] - pi[0], pos[j][1] - pi[1], pos[j][2] - pi[2]];
                let r2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2] + 0.01;
                let inv = mass[j] / (r2 * r2.sqrt());
                acc[0] += d[0] * inv;
                acc[1] += d[1] * inv;
                acc[2] += d[2] * inv;
            }
            for k in 0..3 {
                vel[i][k] += acc[k] * dt;
            }
        }
        for i in 0..bodies {
            for k in 0..3 {
                pos[i][k] += vel[i][k] * dt;
            }
        }
    }
    // Quantize so tiny FP differences across compilers do not matter.
    let mut h = 0u64;
    for p in &pos {
        for v in p {
            h = h
                .wrapping_mul(31)
                .wrapping_add((v * 1000.0).round() as i64 as u64);
        }
    }
    h
}

// ---------------------------------------------------------------- matmul

/// Blocked single precision matrix multiply. SIMD and cache bound,
/// similar to the math in animation and audio processing.
pub fn matmul(n: usize, reps: usize) -> u64 {
    let mut rng = Lcg(42);
    let a: Vec<f32> = (0..n * n).map(|_| rng.f64() as f32 - 0.5).collect();
    let b: Vec<f32> = (0..n * n).map(|_| rng.f64() as f32 - 0.5).collect();
    let mut c = vec![0f32; n * n];
    const BS: usize = 64;
    for _ in 0..reps {
        c.iter_mut().for_each(|x| *x = 0.0);
        for ii in (0..n).step_by(BS) {
            for kk in (0..n).step_by(BS) {
                for i in ii..(ii + BS).min(n) {
                    for k in kk..(kk + BS).min(n) {
                        let aik = a[i * n + k];
                        let brow = &b[k * n..k * n + n];
                        let crow = &mut c[i * n..i * n + n];
                        for (cv, bv) in crow.iter_mut().zip(brow) {
                            *cv += aik * bv;
                        }
                    }
                }
            }
        }
    }
    c.iter().fold(0u64, |h, v| {
        h.wrapping_mul(31).wrapping_add((v * 100.0).round() as i64 as u64)
    })
}

// ---------------------------------------------------------------- hashing

/// SHA-256 over a buffer. Pure integer throughput, like asset
/// decompression and checksumming during game loads.
pub fn hashing(bytes: usize, rounds: usize) -> u64 {
    let mut buf: Vec<u8> = (0..bytes).map(|i| (i * 7 + 3) as u8).collect();
    let mut last = [0u8; 32];
    for _ in 0..rounds {
        last = sha256::digest(&buf);
        buf[..32].copy_from_slice(&last);
    }
    u64::from_le_bytes(last[..8].try_into().unwrap())
}

// ---------------------------------------------------------------- runner

struct Workload {
    key: &'static str,
    name: &'static str,
    unit: &'static str,
    /// Units of work one call performs (used for the rate).
    work: f64,
    run: fn(Size) -> u64,
}

fn workloads(size: Size) -> Vec<Workload> {
    let s = size.scale();
    // Work amounts are expressed in millions of "operations" so the
    // reported rate is in M<unit>/s.
    let sieve_n = 2_000_000 * s;
    let nb_bodies = 256;
    let nb_steps = 4 * s;
    let mm_n = 256;
    let mm_reps = s;
    let hash_bytes = 1 << 20;
    let hash_rounds = 4 * s;
    vec![
        Workload {
            key: "sieve",
            name: "Prime sieve",
            unit: "M numbers/s",
            work: sieve_n as f64 / 1e6,
            run: |s| sieve(2_000_000 * s.scale()),
        },
        Workload {
            key: "nbody",
            name: "N-body physics",
            unit: "M interactions/s",
            work: (nb_bodies * nb_bodies * nb_steps) as f64 / 1e6,
            run: |s| nbody(256, 4 * s.scale()),
        },
        Workload {
            key: "matmul",
            name: "Matrix multiply",
            unit: "GFLOP/s",
            work: (2 * mm_n * mm_n * mm_n * mm_reps) as f64 / 1e9,
            run: |s| matmul(256, s.scale()),
        },
        Workload {
            key: "hash",
            name: "SHA-256 hashing",
            unit: "MB/s",
            work: (hash_bytes * hash_rounds) as f64 / 1e6,
            run: |s| hashing(1 << 20, 4 * s.scale()),
        },
    ]
}

/// Progress callback: (test name, fraction complete 0..1).
pub type Progress<'a> = &'a mut dyn FnMut(&str, f32);

/// Runs every workload single-threaded, then on all threads.
pub fn run_all(size: Size, progress: Progress) -> Vec<Measurement> {
    let list = workloads(size);
    let threads = thread_count();
    let total = (list.len() * 2) as f32;
    let mut out = Vec::new();
    let mut step = 0.0;

    for w in &list {
        progress(&format!("CPU single-thread: {}", w.name), step / total);
        let (t, check) = best_of(3, || (w.run)(size));
        out.push(Measurement {
            id: leak(format!("cpu.single.{}", w.key)),
            name: leak(format!("{} (1 thread)", w.name)),
            value: w.work / t.as_secs_f64(),
            unit: w.unit,
            lower_is_better: false,
            check,
        });
        step += 1.0;
    }

    for w in &list {
        progress(&format!("CPU multi-thread: {}", w.name), step / total);
        let run = w.run;
        let (t, check) = best_of(2, || {
            std::thread::scope(|sc| {
                let handles: Vec<_> = (0..threads).map(|_| sc.spawn(move || run(size))).collect();
                handles.into_iter().fold(0u64, |h, j| h ^ j.join().unwrap())
            })
        });
        out.push(Measurement {
            id: leak(format!("cpu.multi.{}", w.key)),
            name: leak(format!("{} ({} threads)", w.name, threads)),
            value: w.work * threads as f64 / t.as_secs_f64(),
            unit: w.unit,
            lower_is_better: false,
            check,
        });
        step += 1.0;
    }
    progress("CPU tests done", 1.0);
    out
}

/// Measurement ids and names are built once per run, so leaking these
/// few short strings is simpler than threading lifetimes everywhere.
fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// Tiny deterministic PRNG for workload inputs.
pub struct Lcg(pub u64);
impl Lcg {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 ^ (self.0 >> 33)
    }
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sieve_counts_primes() {
        // 25 primes below 100, sum 1060.
        assert_eq!(sieve(100), 25 ^ 1060u64.rotate_left(17));
    }

    #[test]
    fn workloads_are_deterministic() {
        assert_eq!(nbody(32, 2), nbody(32, 2));
        assert_eq!(matmul(64, 1), matmul(64, 1));
        assert_eq!(hashing(4096, 2), hashing(4096, 2));
    }
}
