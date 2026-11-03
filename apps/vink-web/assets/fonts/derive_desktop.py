#!/usr/bin/env python3
"""Reproduce Desktop TTFs from the approved WOFF2s; --check writes nothing.

Install fonttools==4.68.0 brotli==1.2.1 in an isolated Python environment first.
Builds and previews consume the checked-in output, never run this maintenance tool.
"""

import argparse
import hashlib
from io import BytesIO
import json
from pathlib import Path

import brotli
import fontTools
from fontTools.ttLib import TTFont, woff2


def check(condition, message):
    if not condition:
        raise SystemExit(message)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    source_dir = Path(__file__).resolve().parent
    repo = source_dir.parents[3]
    record = json.loads((source_dir / "provenance.json").read_text())
    config = record["desktop"]
    check(fontTools.__version__ == config["fonttools_version"], "Wrong fontTools version")
    check(brotli.__version__ == config["brotli_version"], "Wrong brotli version")
    output_dir = repo / config["directory"]
    for name, face in record["files"].items():
        source = (source_dir / name).read_bytes()
        check(hashlib.sha256(source).hexdigest() == face["sha256"], f"Source hash: {name}")
        output = BytesIO()
        woff2.decompress(BytesIO(source), output)
        data = output.getvalue()
        derivative = face["desktop"]
        check(len(data) == derivative["bytes"], f"Output length: {name}")
        check(hashlib.sha256(data).hexdigest() == derivative["sha256"], f"Output hash: {name}")
        original, decoded = TTFont(BytesIO(source)), TTFont(BytesIO(data))
        check(set(original.reader.keys()) == set(decoded.reader.keys()), f"Table set: {name}")
        # Decompression recalculates head.checkSumAdjustment; every other table is identical.
        for tag in original.reader.keys():
            if tag != "head":
                check(original.getTableData(tag) == decoded.getTableData(tag), f"Changed {tag}: {name}")
        for field, value in vars(original["head"]).items():
            if field != "checkSumAdjustment":
                check(value == vars(decoded["head"])[field], f"Changed head.{field}: {name}")
        path = output_dir / derivative["file"]
        if args.check:
            check(path.read_bytes() == data, f"Stale derivative: {path}")
        else:
            output_dir.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        print(f"PASS {derivative['file']}: {len(data)} bytes, sha256={derivative['sha256']}; tables preserved")


if __name__ == "__main__":
    main()
