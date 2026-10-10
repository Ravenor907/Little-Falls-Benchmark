# Results file format

Each run writes `little-falls-<unix time>.json` to
`~/.local/share/little-falls/results/` (or the folder given by `--out`).

```json
{
  "format_version": 1,
  "score_version": 1,
  "app_version": "0.1.0",
  "timestamp": 1791600000,
  "nickname": "",
  "scores": { "overall": 1000, "graphics": 1000, "cpu": 1000, "memory": 1000 },
  "system": {
    "cpu": "Intel(R) Core(TM) i9-14900K",
    "cpu_threads": 32,
    "ram_mib": 31842,
    "gpu": "AMD Radeon RX 9070 XT (RADV GFX1201)",
    "gpu_driver": "radv Mesa 25.x",
    "gpu_backend": "Vulkan",
    "os": "Bazzite 42",
    "os_id": "bazzite",
    "kernel": "6.x",
    "gamescope": false
  },
  "graphics": {
    "preset": "high", "width": 2560, "height": 1440,
    "frames": 7200, "seconds": 60.0,
    "avg_fps": 120.0, "low1_fps": 90.0, "p50_ms": 8.3, "p99_ms": 11.0
  },
  "tests": [
    { "id": "cpu.single.sieve", "name": "Prime sieve (1 thread)", "value": 540.0,
      "unit": "M numbers/s", "points": 1000, "check": "…" }
  ],
  "signature": "…"
}
```

`graphics` is `null` for `--cpu-only` runs. `scores` fields are 0 for
categories that did not run.

## Integrity

- `check` is a checksum of each workload's output. A run that skipped or
  broke a workload produces the wrong value.
- `signature` is a SHA-256 over the canonical file body plus a salt.

The source is open, so neither stops a determined cheater: they catch
accidental corruption and casual hand edits. The scoreboard adds sanity
checks (values within physical limits, scores consistent with test
values, known check values) and maintainers can reject outliers.
