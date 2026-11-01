#!/usr/bin/env python3
"""Derive Vink marks from the approved comparison, never from a generated redraw.

Offline authoring only: Pillow 12.0.0, numpy 2.3.4, vtracer 0.6.12.
The application/build serves the committed derivatives without these dependencies.
"""

import hashlib
import io
import json
from pathlib import Path
import sys

import numpy as np
from PIL import Image
import vtracer

REFERENCE_SHA256 = "ee10182451ce3b658d5d5160f3011e9334ab1f5fa4388d9cb751bb6076cf6cad"
SIZES = (24, 32, 48, 96, 192, 512)
OUT = Path(__file__).parent


def extract(reference, evidence):
    data = Path(reference).read_bytes()
    if hashlib.sha256(data).hexdigest() != REFERENCE_SHA256:
        raise ValueError("Reference changed: inspect the approved pixels before reauthoring")
    source = Image.open(io.BytesIO(data)).convert("RGB")
    assert source.size == (1774, 887)
    evidence = Path(evidence)
    evidence.mkdir(parents=True, exist_ok=True)
    record = {"reference_url": "https://drive.google.com/file/d/1hHQguADQ2tbiNjQ9r8WqMnYUSEnkAyYs/view",
              "reference_sha256": REFERENCE_SHA256, "reference_size": list(source.size),
              "sizes": SIZES, "files": {}, "themes": {}}
    for theme, offset in (("light", 0), ("dark", 887)):
        half = source.crop((offset, 0, offset + 887, 887))
        half.save(evidence / f"reference-{theme}.png")
        pixels = np.asarray(half, dtype=float)
        # The entire outer border is background. Interpolate its row/column field
        # to distinguish the purple artwork from the comparison's soft backdrop.
        vertical = (pixels[:, :24].mean(axis=1) + pixels[:, -24:].mean(axis=1)) / 2
        horizontal = (pixels[:24].mean(axis=0) + pixels[-24:].mean(axis=0)) / 2
        background = vertical[:, None, :] + horizontal[None, :, :] - horizontal.mean(axis=0)
        distance = np.max(np.abs(pixels - background), axis=2)
        floor, feather = (12, 14) if theme == "light" else (22, 16)
        alpha = np.clip((distance - floor) / feather, 0, 1)
        rgba = np.dstack((pixels, np.rint(alpha * 255))).astype("uint8")
        # Same crop and scale in both themes; no shape edits, rotation or inversion.
        crop = (80, 80, 800, 800)
        mark = Image.fromarray(rgba).crop(crop)
        mark.save(evidence / f"extracted-{theme}.png")
        # A geometric, reduced-palette SVG for scalable consumers. Raster is the
        # preferred in-app rendition because it retains the approved highlights.
        stream = io.BytesIO()
        vector_pixels = np.array(mark)
        # Vector contours omit the soft photographic halo and use hard edges.
        if theme == "light":
            shared_outline = vector_pixels[:, :, 3] >= 230
        # The simplified vector pair uses one approved outline (the light half),
        # with each half's original colour pixels. This excludes the dark backdrop
        # glow from geometry and keeps the pair structurally identical.
        vector_pixels[:, :, 3] = np.where(shared_outline, 255, 0)
        vector_pixels[vector_pixels[:, :, 3] == 0, :3] = 0
        Image.fromarray(vector_pixels).save(stream, format="PNG")
        svg = vtracer.convert_raw_image_to_svg(
            stream.getvalue(), img_format="png", colormode="color", hierarchical="stacked",
            mode="spline", filter_speckle=32, color_precision=4, layer_difference=32,
            corner_threshold=60, length_threshold=6, max_iterations=10,
            splice_threshold=45, path_precision=2)
        svg = svg.replace('width="720" height="720"', 'viewBox="0 0 720 720"')
        svg_path = OUT / f"enso-{theme}.svg"
        svg_path.write_text(svg)
        for size in SIZES:
            icon = mark.resize((size, size), Image.Resampling.LANCZOS)
            icon.save(OUT / f"enso-{theme}-{size}.avif", quality=95, subsampling="4:4:4", speed=4)
        record["themes"][theme] = {"source_half": [offset, 0, offset + 887, 887], "crop_in_half": crop}
    import xml.etree.ElementTree as ET
    ns = "{http://www.w3.org/2000/svg}"
    pair = []
    for theme in ("light", "dark"):
        root = ET.fromstring((OUT / f"enso-{theme}.svg").read_text())
        pair.append(f'<g class="{theme}">' + "".join(ET.tostring(child, encoding="unicode") for child in root) + "</g>")
        surface = "#fffaff" if theme == "light" else "#17121d"
        manifest = {"name": "Vink — Việt Ink", "short_name": "Vink", "lang": "vi", "start_url": "/",
                    "display": "standalone", "background_color": surface, "theme_color": surface,
                    "icons": [{"src": f"/pkg/brand/enso-{theme}-{size}.avif", "sizes": f"{size}x{size}",
                               "type": "image/avif", "purpose": "any"} for size in (192, 512)] +
                             [{"src": f"/pkg/brand/enso-{theme}.svg", "sizes": "any", "type": "image/svg+xml", "purpose": "any"}]}
        (OUT / f"manifest-{theme}.webmanifest").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
    (OUT / "favicon.svg").write_text('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 720 720"><style>.dark{display:none}@media(prefers-color-scheme:dark){.light{display:none}.dark{display:inline}}</style>' + "".join(pair) + "</svg>\n")
    for path in sorted(OUT.iterdir()):
        if path.suffix in (".avif", ".svg", ".webmanifest"):
            record["files"][path.name] = {"sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                                          "bytes": path.stat().st_size}
    (OUT / "provenance.json").write_text(json.dumps(record, indent=2) + "\n")


if __name__ == "__main__":
    extract(sys.argv[1], sys.argv[2])
