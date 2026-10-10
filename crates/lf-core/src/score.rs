//! Turning raw measurements into points.
//!
//! Every measurement is converted to points with a fixed calibration
//! constant, linearly: twice as fast earns twice the points. Category scores
//! are geometric means, so one outlier test cannot dominate, and the
//! overall score is a weighted geometric mean of the categories.

use crate::Measurement;

/// Calibration constants: the measured value that earns 1000 points in
/// each test. Points scale linearly from there. Changing these changes
/// every score, so bump `SCORE_VERSION` whenever they change and the
/// scoreboard keeps versions separate.
pub const SCORE_VERSION: u32 = 1;

fn reference(id: &str) -> Option<f64> {
    Some(match id {
        "cpu.single.sieve" => 540.0,
        "cpu.single.nbody" => 620.0,
        "cpu.single.matmul" => 36.0,
        "cpu.single.hash" => 540.0,
        "cpu.multi.sieve" => 9700.0,
        "cpu.multi.nbody" => 11000.0,
        "cpu.multi.matmul" => 580.0,
        "cpu.multi.hash" => 9700.0,
        "mem.read" => 85.0,
        "mem.write" => 70.0,
        "mem.copy" => 90.0,
        "mem.latency" => 90.0,
        "gfx.avg_fps" => 120.0,
        "gfx.low1_fps" => 90.0,
        _ => return None,
    })
}

pub const WEIGHT_GRAPHICS: f64 = 0.5;
pub const WEIGHT_CPU: f64 = 0.3;
pub const WEIGHT_MEMORY: f64 = 0.2;

pub fn points(m: &Measurement) -> f64 {
    match reference(m.id) {
        Some(r) if m.value > 0.0 => {
            if m.lower_is_better {
                1000.0 * r / m.value
            } else {
                1000.0 * m.value / r
            }
        }
        _ => 0.0,
    }
}

pub fn geomean(v: &[f64]) -> f64 {
    let v: Vec<f64> = v.iter().copied().filter(|x| *x > 0.0).collect();
    if v.is_empty() {
        return 0.0;
    }
    (v.iter().map(|x| x.ln()).sum::<f64>() / v.len() as f64).exp()
}

pub fn category(ms: &[Measurement], prefix: &str) -> f64 {
    let pts: Vec<f64> = ms
        .iter()
        .filter(|m| m.id.starts_with(prefix))
        .map(points)
        .collect();
    geomean(&pts)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Scores {
    pub graphics: f64,
    pub cpu: f64,
    pub memory: f64,
    pub overall: f64,
}

pub fn compute(ms: &[Measurement]) -> Scores {
    let graphics = category(ms, "gfx.");
    let cpu = category(ms, "cpu.");
    let memory = category(ms, "mem.");
    // Weighted geometric mean over the categories that actually ran.
    let mut lw = 0.0;
    let mut w = 0.0;
    for (s, wt) in [
        (graphics, WEIGHT_GRAPHICS),
        (cpu, WEIGHT_CPU),
        (memory, WEIGHT_MEMORY),
    ] {
        if s > 0.0 {
            lw += s.ln() * wt;
            w += wt;
        }
    }
    let overall = if w > 0.0 { (lw / w).exp() } else { 0.0 };
    Scores {
        graphics,
        cpu,
        memory,
        overall,
    }
}

/// Frame time statistics for the graphics test.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameStats {
    pub frames: usize,
    pub seconds: f64,
    pub avg_fps: f64,
    /// Average FPS of the slowest 1% of frames.
    pub low1_fps: f64,
    pub p50_ms: f64,
    pub p99_ms: f64,
}

pub fn frame_stats(frame_times_s: &[f64]) -> FrameStats {
    if frame_times_s.is_empty() {
        return FrameStats::default();
    }
    let total: f64 = frame_times_s.iter().sum();
    let mut sorted = frame_times_s.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = sorted.len();
    let pct = |p: f64| sorted[((n as f64 * p) as usize).min(n - 1)] * 1000.0;
    let worst = (n / 100).max(1);
    let worst_avg = sorted[n - worst..].iter().sum::<f64>() / worst as f64;
    FrameStats {
        frames: n,
        seconds: total,
        avg_fps: n as f64 / total,
        low1_fps: 1.0 / worst_avg,
        p50_ms: pct(0.5),
        p99_ms: pct(0.99),
    }
}

/// Common display refresh rates and frame limiter targets.
const CAP_RATES: [f64; 19] = [
    30.0, 40.0, 45.0, 48.0, 50.0, 60.0, 72.0, 75.0, 90.0, 100.0, 120.0, 144.0, 165.0, 170.0, 180.0, 200.0,
    240.0, 280.0, 360.0,
];

/// Detects a run held to a fixed rate by vsync or a frame limiter. Such a
/// run measures the display, not the GPU. The tell: the average sits on a
/// standard rate *and* frame times barely vary, even though the scene's
/// load changes a lot along the flythrough.
pub fn detect_frame_cap(frame_times_s: &[f64]) -> Option<f64> {
    if frame_times_s.len() < 120 {
        return None;
    }
    let mut sorted = frame_times_s.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = sorted.len();
    let median = sorted[n / 2];
    let p10 = sorted[n / 10];
    let p90 = sorted[n * 9 / 10];
    let fps = 1.0 / median;
    let rate = CAP_RATES
        .iter()
        .copied()
        .min_by(|a, b| (a - fps).abs().partial_cmp(&(b - fps).abs()).unwrap())?;
    let on_rate = (fps - rate).abs() / rate < 0.015;
    let steady = p90 / p10 < 1.12;
    (on_rate && steady).then_some(rate)
}

pub fn graphics_measurements(fs: &FrameStats) -> Vec<Measurement> {
    vec![
        Measurement {
            id: "gfx.avg_fps",
            name: "Average FPS",
            value: fs.avg_fps,
            unit: "fps",
            lower_is_better: false,
            check: fs.frames as u64,
            spread: 0.0,
        },
        Measurement {
            id: "gfx.low1_fps",
            name: "1% low FPS",
            value: fs.low1_fps,
            unit: "fps",
            lower_is_better: false,
            check: fs.frames as u64,
            spread: 0.0,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_stats_basic() {
        let mut ft = vec![1.0 / 60.0; 99];
        ft.push(1.0 / 10.0);
        let s = frame_stats(&ft);
        assert_eq!(s.frames, 100);
        assert!((s.low1_fps - 10.0).abs() < 1e-6);
        assert!(s.avg_fps > 50.0 && s.avg_fps < 60.0);
    }

    #[test]
    fn detects_vsync_but_not_natural_variation() {
        // Locked to 60 Hz with small jitter.
        let locked: Vec<f64> = (0..600).map(|i| 1.0 / 60.0 + ((i % 7) as f64 - 3.0) * 0.00005).collect();
        assert_eq!(detect_frame_cap(&locked), Some(60.0));
        // Uncapped: averages near 60 but load varies along the path.
        let free: Vec<f64> = (0..600).map(|i| 1.0 / (45.0 + 30.0 * ((i as f64) / 600.0))).collect();
        assert_eq!(detect_frame_cap(&free), None);
    }

    #[test]
    fn reference_scores_1000() {
        let m = Measurement {
            id: "mem.latency",
            name: "",
            value: 90.0,
            unit: "ns",
            lower_is_better: true,
            check: 0,
            spread: 0.0,
        };
        assert!((points(&m) - 1000.0).abs() < 1e-9);
    }
}
