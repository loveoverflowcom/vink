"""Public Pages inputs only. Called after extracting /opt/vink/site/pkg from an OCI digest."""
import hashlib
import json
from pathlib import Path
import re
import shutil

SHA = re.compile(r"[0-9a-f]{40}")
PROJECT = "vink-production"
MAX_FILE = 25 * 1024 * 1024
TYPES = {".js": "text/javascript", ".css": "text/css", ".wasm": "application/wasm",
         ".woff2": "font/woff2", ".webp": "image/webp", ".png": "image/png",
         ".svg": "image/svg+xml", ".webmanifest": "application/manifest+json",
         ".json": "application/json", ".txt": "text/plain", ".md": "text/plain"}
# Only public build products. The old Handle assets remain allowlisted for retained rollback packages.
FILES = re.compile(
    r"vink(?:-editor)?\.js|vink\.css|vink_bg\.wasm|author-landscape\.webp|"
    r"snippets/vink-web-[a-f0-9]+/(?:(?:editor|expressions)/bridge|public-cache/indexeddb|profile/(?:navigation|handle))\.js|"
    r"snippets/vink-patterns-[a-f0-9]+/inline[0-9]+\.js|"
    r"fonts/(?:[\w-]+\.woff2|OFL-[\w-]+\.txt)|brand/vpen-r1-[\w-]+\.(?:svg|png|webmanifest)|"
    r"empty/vink-empty-[\w-]+\.webp|expressions/(?:build\.json|pilot\.js|NOTICE\.txt|"
    r"LICENSE-(?:STATIC|DOTLOTTIE|THIRD-PARTY)\.txt|LICENSE-ANIMATED\.md|"
    r"assets/noto-[\w-]+\.(?:svg|json)|runtime/dotlottie-[\w-]+\.(?:wasm|js))", re.ASCII)
REQUIRED = {"vink.js", "vink.css", "vink_bg.wasm", "vink-editor.js",
            "fonts/be-vietnam-pro-400.woff2", "fonts/literata-variable.woff2",
            "brand/vpen-r1-favicon.svg", "expressions/pilot.js", "empty/vink-empty-first-draft.webp"}


def require(ok, reason):
    if not ok:
        raise ValueError(reason)


def encode(value):
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def validate(spec):
    require(set(spec) == {"environment", "public_origin", "source_sha", "platform", "images"},
            "unexpected release fields")
    require(spec["environment"] == "production" and spec["public_origin"] == "https://vink.forum",
            "wrong application target")
    require(SHA.fullmatch(spec["source_sha"]) and spec["platform"] in ("linux/amd64", "linux/arm64"),
            "invalid revision/platform")
    require(set(spec["images"]) == {"web", "worker", "identity"}, "three qualified images required")
    for name, image in spec["images"].items():
        require(re.fullmatch(f"ghcr.io/loveoverflowcom/vink-{name}@sha256:[a-f0-9]{{64}}", image),
                f"wrong {name} image")


def inventory(pkg):
    entries = {}
    require(pkg.is_dir() and not pkg.is_symlink(), "expected pkg directory")
    for file in sorted(pkg.rglob("*")):
        require(not file.is_symlink(), "symlink in public package")
        if file.is_dir():
            continue
        name = file.relative_to(pkg).as_posix()
        require(file.is_file() and FILES.fullmatch(name), f"unexpected public asset {name}")
        size = file.stat().st_size
        require(0 < size <= MAX_FILE, f"Pages file limit or empty asset: {name} ({size})")
        entries[name] = {"bytes": size, "sha256": sha(file.read_bytes()), "type": TYPES[file.suffix]}
    require(REQUIRED <= entries.keys(), f"missing assets: {sorted(REQUIRED - entries.keys())}")
    return entries


def record(pkg, spec):
    validate(spec)
    entries = inventory(pkg)
    return {**spec, "assets": entries, "asset_digest": sha(encode(entries))}


def stage(archives, target, output, legacy=None):
    """Rebuild the edge inventory from retained immutable packages, including on rollback."""
    index = {}
    require(not output.exists(), "staging directory exists")
    for archive in sorted(archives.iterdir()):
        require(SHA.fullmatch(archive.name) and not archive.is_symlink(), "invalid archive name")
        receipt = json.loads((archive / "manifest.json").read_bytes())
        spec = {key: receipt[key] for key in ("environment", "public_origin", "source_sha", "platform", "images")}
        pkg = archive / "pkg"
        require(archive.name == spec["source_sha"] and record(pkg, spec) == receipt, "archive drift")
        prefix = f"_v/{archive.name}/pkg"
        shutil.copytree(pkg, output / prefix)
        for name, item in receipt["assets"].items():
            path = f"/{prefix}/{name}"
            index[path] = item
            if archive.name == (legacy or target):
                index[f"/pkg/{name}"] = {**item, "path": path}
    require(f"/_v/{target}/pkg/vink.js" in index, "release missing")
    require(len(index) + 3 <= 20000, "Pages file limit; review retention before proceeding")
    template = Path(__file__).with_name("worker.mjs").read_text()
    require(template.count("/*VINK_ASSETS*/ {}") == 1, "worker template drift")
    worker = template.replace("/*VINK_ASSETS*/ {}", encode(index).decode().strip())
    (output / "_worker.js").write_text(worker)
    (output / "_routes.json").write_bytes(encode({"version": 1, "include": ["/*"], "exclude": []}))
    (output / "404.html").write_text("Not found")
    return sha(worker.encode())
