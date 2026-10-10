#!/usr/bin/env python3
"""Download the CC0 art assets listed in tools/assets.json into assets/.

Usage: python3 tools/fetch_assets.py [out-dir]   (default: assets)

Layout produced (what the app expects):
  assets/textures/<id>/{diff,nor_gl,rough}.jpg
  assets/models/<id>/<id>.gltf  (+ .bin and textures/, as published)
  assets/hdri/<id>.hdr
  assets/CREDITS.txt, assets/files.json

Re-running skips files that already exist, so CI can cache the folder.
"""
import json
import sys
import time
import urllib.request
from pathlib import Path

API = "https://api.polyhaven.com"
UA = {"User-Agent": "little-falls-benchmark asset fetch"}
ROOT = Path(__file__).resolve().parent.parent

# Our map names -> Poly Haven's file keys (matched case-insensitively).
MAP_KEYS = {"diff": ["diffuse", "diff"], "nor_gl": ["nor_gl"], "rough": ["rough", "roughness"]}


def get(url, tries=4):
    for i in range(tries):
        try:
            with urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=120) as r:
                return r.read()
        except Exception:  # noqa: BLE001
            if i == tries - 1:
                raise
            time.sleep(2 * (i + 1))


def download(url, dest: Path):
    if dest.exists() and dest.stat().st_size > 0:
        return False
    dest.parent.mkdir(parents=True, exist_ok=True)
    tmp = dest.with_suffix(dest.suffix + ".part")
    tmp.write_bytes(get(url))
    tmp.rename(dest)
    return True


def files(asset_id):
    return json.loads(get(f"{API}/files/{asset_id}"))


def pick(d, res):
    """Choose the requested resolution, else the closest smaller one."""
    if res in d:
        return d[res]
    order = ["1k", "2k", "4k", "8k"]
    for r in reversed(order[: order.index(res) + 1] if res in order else order):
        if r in d:
            return d[r]
    return d[sorted(d)[0]]


def main():
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "assets"
    manifest = json.loads((ROOT / "tools" / "assets.json").read_text())
    res = manifest["resolution"]
    listing = {}
    fetched = 0

    for tid, maps in manifest["textures"].items():
        f = files(tid)
        keys = {k.lower(): k for k in f}
        listing[tid] = sorted(f)
        for m in maps:
            key = next((keys[c] for c in MAP_KEYS[m] if c in keys), None)
            if key is None:
                raise SystemExit(f"{tid}: no '{m}' map (has {sorted(f)})")
            entry = pick(f[key], res)
            fmt = "jpg" if "jpg" in entry else next(iter(entry))
            fetched += download(entry[fmt]["url"], out / "textures" / tid / f"{m}.{fmt}")

    for mid in manifest["models"]:
        f = files(mid)
        listing[mid] = sorted(f)
        g = pick(f["gltf"], res)["gltf"]
        fetched += download(g["url"], out / "models" / mid / f"{mid}.gltf")
        for rel, inc in g.get("include", {}).items():
            fetched += download(inc["url"], out / "models" / mid / rel)

    for hid in manifest["hdris"]:
        f = files(hid)
        listing[hid] = sorted(f)
        h = pick(f["hdri"], res)
        fetched += download(h["hdr"]["url"], out / "hdri" / f"{hid}.hdr")

    names = list(manifest["textures"]) + manifest["models"] + manifest["hdris"]
    (out / "CREDITS.txt").write_text(
        "Little Falls art assets\n\n"
        "All assets below are from Poly Haven (https://polyhaven.com) and are\n"
        "released under CC0 (public domain). Thank you to the Poly Haven team.\n\n"
        + "\n".join(f"  https://polyhaven.com/a/{n}" for n in names)
        + "\n"
    )
    (out / "files.json").write_text(json.dumps(listing, indent=1))
    total = sum(p.stat().st_size for p in out.rglob("*") if p.is_file())
    print(f"assets ready in {out}: {fetched} downloaded, {total / 1e6:.1f} MB total")


if __name__ == "__main__":
    main()
