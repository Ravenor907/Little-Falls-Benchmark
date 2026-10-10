//! Memory bandwidth and latency tests.

use crate::{best_of, cpu::Lcg, thread_count, Measurement};

/// Buffer size for bandwidth tests. Large enough to defeat any current
/// CPU cache (including 3D V-Cache parts with 128 MB+ of L3).
const BW_BYTES: usize = 512 << 20;
const QUICK_BW_BYTES: usize = 32 << 20;

/// Pointer-chase working set for latency.
const LAT_BYTES: usize = 256 << 20;
const QUICK_LAT_BYTES: usize = 16 << 20;

fn split_mut(buf: &mut [u64], parts: usize) -> Vec<&mut [u64]> {
    let chunk = buf.len().div_ceil(parts);
    buf.chunks_mut(chunk).collect()
}

fn split(buf: &[u64], parts: usize) -> Vec<&[u64]> {
    let chunk = buf.len().div_ceil(parts);
    buf.chunks(chunk).collect()
}

fn read_sum(s: &[u64]) -> u64 {
    // Four independent accumulators let the compiler vectorize and keep
    // several loads in flight.
    let mut acc = [0u64; 4];
    let (chunks, rest) = s.as_chunks::<4>();
    for c in chunks {
        acc[0] = acc[0].wrapping_add(c[0]);
        acc[1] = acc[1].wrapping_add(c[1]);
        acc[2] = acc[2].wrapping_add(c[2]);
        acc[3] = acc[3].wrapping_add(c[3]);
    }
    let tail: u64 = rest.iter().fold(0, |a, b| a.wrapping_add(*b));
    acc.iter().fold(tail, |a, b| a.wrapping_add(*b))
}

/// Multi-threaded read, write and copy bandwidth in GB/s.
pub fn bandwidth(quick: bool) -> Vec<Measurement> {
    let bytes = if quick { QUICK_BW_BYTES } else { BW_BYTES };
    let words = bytes / 8;
    let threads = thread_count();
    let mut src: Vec<u64> = (0..words as u64).collect();
    let mut dst: Vec<u64> = vec![0; words];
    let gb = bytes as f64 / 1e9;

    let (t_read, c_read, s_read) = best_of(5, || {
        std::thread::scope(|sc| {
            let hs: Vec<_> = split(&src, threads)
                .into_iter()
                .map(|p| sc.spawn(move || read_sum(p)))
                .collect();
            hs.into_iter()
                .fold(0u64, |a, h| a.wrapping_add(h.join().unwrap()))
        })
    });

    let mut pass = 0u64;
    let (t_write, c_write, s_write) = best_of(5, || {
        pass += 1;
        let v = pass;
        std::thread::scope(|sc| {
            for p in split_mut(&mut dst, threads) {
                sc.spawn(move || p.fill(v));
            }
        });
        dst[words / 2]
    });

    let (t_copy, c_copy, s_copy) = best_of(5, || {
        std::thread::scope(|sc| {
            for (d, s) in split_mut(&mut dst, threads).into_iter().zip(split(&src, threads)) {
                sc.spawn(move || d.copy_from_slice(s));
            }
        });
        dst[words - 1]
    });
    // Copy moves the data twice (read + write).
    src[0] = c_copy; // keep src observably used

    vec![
        Measurement {
            id: "mem.read",
            name: "Memory read",
            value: gb / t_read.as_secs_f64(),
            unit: "GB/s",
            lower_is_better: false,
            check: c_read,
            spread: s_read,
        },
        Measurement {
            id: "mem.write",
            name: "Memory write",
            value: gb / t_write.as_secs_f64(),
            unit: "GB/s",
            lower_is_better: false,
            check: c_write,
            spread: s_write,
        },
        Measurement {
            id: "mem.copy",
            name: "Memory copy",
            value: 2.0 * gb / t_copy.as_secs_f64(),
            unit: "GB/s",
            lower_is_better: false,
            check: c_copy,
            spread: s_copy,
        },
    ]
}

/// Random access latency in nanoseconds, measured by chasing a single
/// random cycle through a large buffer (Sattolo's algorithm guarantees
/// one cycle that visits every slot).
pub fn latency(quick: bool) -> Measurement {
    let bytes = if quick { QUICK_LAT_BYTES } else { LAT_BYTES };
    // One pointer per 64-byte cache line so every hop is a fresh line.
    const STRIDE: usize = 8;
    let slots = bytes / 64;
    let mut order: Vec<usize> = (0..slots).collect();
    let mut rng = Lcg(0xC0FFEE);
    for i in (1..slots).rev() {
        let j = (rng.next_u64() % i as u64) as usize;
        order.swap(i, j);
    }
    let mut next = vec![0usize; slots * STRIDE];
    for i in 0..slots {
        next[i * STRIDE] = order[i] * STRIDE;
    }
    drop(order);

    let hops = if quick { 2_000_000 } else { 20_000_000 };
    let (t, check, spread) = best_of(3, || {
        let mut p = 0usize;
        for _ in 0..hops {
            p = next[p];
        }
        p as u64
    });
    Measurement {
        id: "mem.latency",
        name: "Memory latency",
        value: t.as_secs_f64() * 1e9 / hops as f64,
        unit: "ns",
        lower_is_better: true,
        check,
        spread,
    }
}

pub fn run_all(quick: bool, progress: crate::cpu::Progress) -> Vec<Measurement> {
    progress("Memory bandwidth", 0.0);
    let mut v = bandwidth(quick);
    progress("Memory latency", 0.6);
    v.push(latency(quick));
    progress("Memory tests done", 1.0);
    v
}

#[cfg(test)]
mod tests {
    #[test]
    fn quick_memory_runs() {
        let mut v = super::bandwidth(true);
        v.push(super::latency(true));
        for m in v {
            assert!(m.value.is_finite() && m.value > 0.0, "{}", m.id);
        }
    }
}
