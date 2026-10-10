//! The results file submitted to the scoreboard.
//!
//! The `signature` field is a SHA-256 over the canonical body plus a
//! build-time salt. It only catches casual hand edits: the source is open,
//! so anyone determined can recompute it. The scoreboard backs it up with
//! sanity checks (see docs/RESULTS_FORMAT.md).

use crate::json::Json;
use crate::score::{self, FrameStats, Scores};
use crate::sha256;
use crate::sysinfo::SystemInfo;
use crate::Measurement;

pub const FORMAT_VERSION: u32 = 1;
const SALT: &str = "little-falls/v1/cascade";

pub struct RunInfo {
    pub app_version: &'static str,
    pub preset: String,
    pub resolution: (u32, u32),
    pub unix_time: u64,
    pub nickname: String,
}

pub struct Report {
    pub json: Json,
    pub scores: Scores,
}

fn measurement_json(m: &Measurement) -> Json {
    Json::obj()
        .with("id", m.id)
        .with("name", m.name)
        .with("value", m.value)
        .with("unit", m.unit)
        .with("points", score::points(m).round())
        .with("check", format!("{:016x}", m.check))
        .with("spread", m.spread)
}

pub fn build(
    run: &RunInfo,
    sys: &SystemInfo,
    measurements: &[Measurement],
    frames: Option<&FrameStats>,
    warnings: &[String],
) -> Report {
    let scores = score::compute(measurements);
    let system = Json::obj()
        .with("cpu", sys.cpu_model.as_str())
        .with("cpu_threads", sys.cpu_threads)
        .with("ram_mib", sys.ram_mib)
        .with("gpu", sys.gpu_name.as_str())
        .with("gpu_driver", sys.gpu_driver.as_str())
        .with("gpu_backend", sys.gpu_backend.as_str())
        .with("os", sys.os_name.as_str())
        .with("os_id", sys.os_id.as_str())
        .with("kernel", sys.kernel.as_str())
        .with("gamescope", sys.gamescope);

    let graphics = match frames {
        Some(f) => Json::obj()
            .with("preset", run.preset.as_str())
            .with("width", run.resolution.0)
            .with("height", run.resolution.1)
            .with("frames", f.frames)
            .with("seconds", f.seconds)
            .with("avg_fps", f.avg_fps)
            .with("low1_fps", f.low1_fps)
            .with("p50_ms", f.p50_ms)
            .with("p99_ms", f.p99_ms),
        None => Json::Null,
    };

    let body = Json::obj()
        .with("format_version", FORMAT_VERSION)
        .with("score_version", score::SCORE_VERSION)
        .with("app_version", run.app_version)
        .with("timestamp", run.unix_time)
        .with("nickname", run.nickname.as_str())
        .with(
            "scores",
            Json::obj()
                .with("overall", scores.overall.round())
                .with("graphics", scores.graphics.round())
                .with("cpu", scores.cpu.round())
                .with("memory", scores.memory.round()),
        )
        .with("system", system)
        .with("graphics", graphics)
        .with(
            "warnings",
            warnings.iter().map(|w| Json::Str(w.clone())).collect::<Vec<_>>(),
        )
        .with(
            "tests",
            measurements.iter().map(measurement_json).collect::<Vec<_>>(),
        );

    let sig = signature(&body);
    let json = match body {
        Json::Obj(mut f) => {
            f.push(("signature".into(), Json::Str(sig)));
            Json::Obj(f)
        }
        other => other,
    };
    Report { json, scores }
}

pub fn signature(body: &Json) -> String {
    let mut data = SALT.as_bytes().to_vec();
    data.extend_from_slice(body.to_string_compact().as_bytes());
    sha256::hex(&sha256::digest(&data))
}

/// Writes the report to `dir/little-falls-<timestamp>.json`.
pub fn save(report: &Report, dir: &std::path::Path, unix_time: u64) -> std::io::Result<std::path::PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("little-falls-{unix_time}.json"));
    std::fs::write(&path, report.json.to_string_pretty())?;
    Ok(path)
}

/// Default results folder: $XDG_DATA_HOME/little-falls/results, falling
/// back to ~/.local/share/little-falls/results.
pub fn default_dir() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    base.join("little-falls").join("results")
}

/// A saved run, as shown in the history list.
#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub timestamp: u64,
    pub overall: f64,
    pub graphics: f64,
    pub cpu: f64,
    pub memory: f64,
    pub preset: String,
    pub resolution: (u32, u32),
    pub gpu: String,
    pub warnings: usize,
}

/// Reads every results file in `dir`, newest first. Unreadable files are
/// skipped.
pub fn load_history(dir: &std::path::Path) -> Vec<HistoryEntry> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<HistoryEntry> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|t| crate::json::parse(&t).ok())
        .map(|j| {
            let num = |p: &[&str]| j.at(p).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let s = |p: &[&str]| j.at(p).and_then(|v| v.as_str()).unwrap_or("").to_string();
            HistoryEntry {
                timestamp: num(&["timestamp"]) as u64,
                overall: num(&["scores", "overall"]),
                graphics: num(&["scores", "graphics"]),
                cpu: num(&["scores", "cpu"]),
                memory: num(&["scores", "memory"]),
                preset: s(&["graphics", "preset"]),
                resolution: (
                    num(&["graphics", "width"]) as u32,
                    num(&["graphics", "height"]) as u32,
                ),
                gpu: s(&["system", "gpu"]),
                warnings: j
                    .get("warnings")
                    .and_then(|w| w.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0),
            }
        })
        .collect();
    out.sort_by_key(|e| std::cmp::Reverse(e.timestamp));
    out
}

/// "2026-10-09 18:46" in UTC from a Unix timestamp (no time zone database
/// needed; good enough for a history list).
pub fn format_utc(ts: u64) -> String {
    let days = (ts / 86_400) as i64;
    let secs = ts % 86_400;
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_dates() {
        assert_eq!(format_utc(0), "1970-01-01 00:00");
        assert_eq!(format_utc(1_791_600_000), "2026-10-10 02:40");
    }

    #[test]
    fn history_roundtrip() {
        let dir = std::env::temp_dir().join(format!("lf-history-{}", std::process::id()));
        let run = RunInfo {
            app_version: "t",
            preset: "high".into(),
            resolution: (1280, 800),
            unix_time: 1_791_600_000,
            nickname: String::new(),
        };
        let r = build(&run, &SystemInfo::default(), &[], None, &["w".into()]);
        save(&r, &dir, run.unix_time).unwrap();
        let h = load_history(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].timestamp, 1_791_600_000);
        assert_eq!(h[0].warnings, 1);
    }
}
