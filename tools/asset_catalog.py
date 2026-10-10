#!/usr/bin/env python3
"""Survey Poly Haven (CC0) for forest assets and render labeled contact
sheets, so assets can be chosen by eye. Runs in CI, which has internet.

Usage: asset_catalog.py <out-dir>
"""
import io
import json
import sys
import urllib.request
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

API = "https://api.polyhaven.com"
UA = {"User-Agent": "little-falls-benchmark asset survey"}

KEYWORDS = {
    "textures": ["rock", "moss", "bark", "forest", "ground", "leaves", "leaf", "mud", "soil",
                 "stone", "cliff", "gravel", "root", "pine", "needle", "wood", "river", "wet"],
    "models": ["rock", "moss", "fern", "plant", "tree", "stump", "log", "branch", "bush",
               "shrub", "grass", "leaf", "root", "boulder", "stone", "mushroom", "nature"],
    "hdris": ["forest", "woods", "tree", "park", "nature", "river", "canyon", "meadow"],
}


def get(url):
    with urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=60) as r:
        return r.read()


def matches(asset, words):
    hay = " ".join([asset.get("name", "")] + asset.get("tags", []) + asset.get("categories", [])).lower()
    return any(w in hay for w in words)


def sheet(items, kind, out, page):
    cols, size, label = 6, 256, 34
    rows = (len(items) + cols - 1) // cols
    img = Image.new("RGB", (cols * size, rows * (size + label)), (24, 24, 24))
    draw = ImageDraw.Draw(img)
    try:
        font = ImageFont.truetype("DejaVuSans.ttf", 15)
    except OSError:
        font = ImageFont.load_default()
    for i, (aid, _) in enumerate(items):
        x, y = (i % cols) * size, (i // cols) * (size + label)
        try:
            thumb = Image.open(io.BytesIO(get(f"https://cdn.polyhaven.com/asset_img/thumbs/{aid}.png?width=256&height=256")))
            thumb = thumb.convert("RGB").resize((size, size))
            img.paste(thumb, (x, y))
        except Exception as e:  # noqa: BLE001
            draw.text((x + 8, y + 8), f"no thumb: {e}"[:40], fill=(200, 80, 80), font=font)
        draw.text((x + 6, y + size + 8), aid[:30], fill=(235, 235, 235), font=font)
    img.save(out / f"{kind}-{page:02d}.jpg", quality=82)


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    summary = {}
    for kind, words in KEYWORDS.items():
        assets = json.loads(get(f"{API}/assets?t={kind}"))
        picked = {k: v for k, v in assets.items() if matches(v, words)}
        # Most downloaded first: popular assets tend to be the best scans.
        items = sorted(picked.items(), key=lambda kv: -kv[1].get("download_count", 0))
        summary[kind] = {
            k: {
                "name": v.get("name"),
                "categories": v.get("categories"),
                "tags": v.get("tags"),
                "polycount": v.get("polycount"),
                "dimensions": v.get("dimensions"),
                "downloads": v.get("download_count"),
            }
            for k, v in items
        }
        for p in range(0, min(len(items), 180), 30):
            sheet(items[p:p + 30], kind, out, p // 30 + 1)
        print(f"{kind}: {len(items)} matching of {len(assets)}")
    (out / "catalog.json").write_text(json.dumps(summary, indent=1))


if __name__ == "__main__":
    main()
