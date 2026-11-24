"""Validate public asset inventories and package Pages deployments for SSR or standalone CSR."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import tempfile

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
            "brand/vpen-r1-favicon.svg", "empty/vink-empty-first-draft.webp"}


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


def is_preview_asset(name):
    parts = name.split("/")
    return any(part in {"expressions", "preview", "fixtures"} or part.startswith(
        ("vink-ui-preview-", "vink_ui_preview", "expression_hydration")) for part in parts)


def inventory(pkg, *, retained=False):
    entries = {}
    require(pkg.is_dir() and not pkg.is_symlink(), "expected pkg directory")
    for file in sorted(pkg.rglob("*")):
        require(not file.is_symlink(), "symlink in public package")
        if file.is_dir():
            continue
        name = file.relative_to(pkg).as_posix()
        if not retained:
            require(not is_preview_asset(name), f"Preview asset in product package {name}")
        require(file.is_file() and FILES.fullmatch(name), f"unexpected public asset {name}")
        size = file.stat().st_size
        require(0 < size <= MAX_FILE, f"Pages file limit or empty asset: {name} ({size})")
        entries[name] = {"bytes": size, "sha256": sha(file.read_bytes()), "type": TYPES[file.suffix]}
    require(REQUIRED <= entries.keys(), f"missing assets: {sorted(REQUIRED - entries.keys())}")
    return entries


def record(pkg, spec, *, retained=False):
    validate(spec)
    entries = inventory(pkg, retained=retained)
    return {**spec, "assets": entries, "asset_digest": sha(encode(entries))}


def read_archive(archive):
    """Revalidate a pre-cutover, image-qualified asset archive against its receipt."""
    require(SHA.fullmatch(archive.name) and not archive.is_symlink(), "invalid archive name")
    receipt = json.loads((archive / "manifest.json").read_bytes())
    spec = {key: receipt[key] for key in ("environment", "public_origin", "source_sha", "platform", "images")}
    pkg = archive / "pkg"
    require(archive.name == spec["source_sha"] and record(pkg, spec, retained=True) == receipt, "archive drift")
    return pkg, receipt


def stage(archives, target, output, legacy=None):
    """Rebuild the edge inventory from retained immutable packages, including on rollback."""
    index = {}
    require(not output.exists(), "staging directory exists")
    for archive in sorted(archives.iterdir()):
        pkg, receipt = read_archive(archive)
        prefix = f"_v/{archive.name}/pkg"
        shutil.copytree(pkg, output / prefix)
        for name, item in receipt["assets"].items():
            path = f"/{prefix}/{name}"
            index[path] = item
            if archive.name == (legacy or target):
                index[f"/pkg/{name}"] = {**item, "path": path}
    require(f"/_v/{target}/pkg/vink.js" in index, "release missing")
    return write_worker(output, index)


def write_worker(output, index):
    require(len(index) + 4 <= 20000, "Pages file limit; review retention before proceeding")
    template = Path(__file__).with_name("worker.mjs").read_text()
    require(template.count("/*VINK_ASSETS*/ {}") == 1, "worker template drift")
    worker = template.replace("/*VINK_ASSETS*/ {}", encode(index).decode().strip())
    (output / "_worker.js").write_text(worker)
    (output / "_routes.json").write_bytes(encode({"version": 1, "include": ["/*"], "exclude": []}))
    (output / "404.html").write_text("Not found")
    return sha(worker.encode())


def spa_entry(site):
    shell = (site / "index.html").read_bytes()
    namespaces = set(re.findall(rb'/_v/([0-9a-f]{40})/pkg/', shell))
    require(len(namespaces) == 1, "entry point must pin exactly one release")
    revision = namespaces.pop().decode()
    require(b'{{' not in shell and f'/_v/{revision}/pkg/vink.js'.encode() in shell,
            "entry point lacks pinned bootstrap")
    return shell, revision


def stage_spa(active, retained, output):
    """Package the selected shell and explicit rollback assets, without contacting the VPS."""
    require(not output.exists(), "staging directory exists")
    shell, _ = spa_entry(active)
    index = {"/": {"sha256": sha(shell), "type": "text/html"}}
    packages = {}
    for site in [active, *retained]:
        if site != active and (site / "manifest.json").is_file():
            pkg, receipt = read_archive(site)
            version, entries = site.name, receipt["assets"]
        else:
            _, version = spa_entry(site)
            namespace = site / "_v" / version
            require(not namespace.is_symlink() and not namespace.parent.is_symlink(), "symlink in release path")
            pkg = namespace / "pkg"
            entries = inventory(pkg)
        if version in packages:
            require(entries == packages[version][1], "different bytes for immutable release")
        packages[version] = (pkg, entries)
    with tempfile.TemporaryDirectory(dir=output.parent) as temp:
        build = Path(temp) / "stage"
        for version, (pkg, entries) in packages.items():
            prefix = f"_v/{version}/pkg"
            shutil.copytree(pkg, build / prefix)
            for name, entry in entries.items():
                index[f"/{prefix}/{name}"] = entry
        (build / "index.html").write_bytes(shell)
        digest = write_worker(build, index)
        build.rename(output)
    return digest


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="Stage the standalone frontend for Pages")
    parser.add_argument("--spa", type=Path, required=True)
    parser.add_argument("--retain", type=Path, action="append", default=[])
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    print(stage_spa(args.spa, args.retain, args.out))
