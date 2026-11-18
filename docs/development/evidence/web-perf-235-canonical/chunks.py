"""Run the ignored #235 fixture over loopback HTTP and an unbuffered Caddy reverse proxy.

Usage: python3 chunks.py /path/to/ssr_boundary-test-binary /path/to/caddy > chunks.json
"""
import contextlib
import http.client
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import time
from html.parser import HTMLParser

WORK = "/tac-pham/33333333-3333-4333-8333-333333333333"
ARTICLE = "Phố cũ mưa giăng sợi mỏng,"
PRIVATE = "PRIVATE-DRAFT-MUST-NOT-ENTER-HTML"


class Document(HTMLParser):
    def __init__(self):
        super().__init__()
        self.inert = 0
        self.visible = []
        self.canonical = None
        self.noindex = False
        self.description = False
        self.title = ""
        self.in_title = False

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag in ("script", "template", "style"):
            self.inert += 1
        if self.inert:
            return
        if tag == "title":
            self.in_title = True
        if tag == "link" and attrs.get("rel") == "canonical":
            self.canonical = attrs.get("href")
        if tag == "meta":
            self.noindex |= attrs.get("name") == "robots" and "noindex" in attrs.get("content", "")
            self.description |= attrs.get("name") == "description"

    def handle_endtag(self, tag):
        if tag in ("script", "template", "style"):
            self.inert -= 1
        if tag == "title":
            self.in_title = False

    def handle_data(self, data):
        if not self.inert:
            self.visible.append(data)
            if self.in_title:
                self.title += data


def read(origin, case):
    start = time.monotonic()
    ms = lambda: round((time.monotonic() - start) * 1000, 2)
    conn = http.client.HTTPConnection(origin, timeout=12)
    try:
        conn.request("GET", WORK)
        response = conn.getresponse()
        headers = ms()
        data = bytearray()
        article_ms = None
        chunks = []
        while packet := response.read1(16384):
            data.extend(packet)
            chunks.append([ms(), len(packet)])
            doc = Document()
            doc.feed(data.decode(errors="ignore"))
            if article_ms is None and ARTICLE in "".join(doc.visible):
                article_ms = ms()
        body = data.decode()
        doc = Document()
        doc.feed(body)
        published = case == "published"
        expected_status = {"missing": 404, "error": 503}.get(case, 200)
        valid = (response.status == expected_status and PRIVATE not in body
                 and (article_ms is not None) == published)
        visible = "".join(doc.visible)
        secondary_fallback = "Chưa tải được nhận xét. Bạn vẫn có thể đọc tác phẩm." in visible
        if published:
            valid &= secondary_fallback
            valid &= "[Mẫu] Mưa trên phố cũ" in doc.title and bool(doc.canonical) and doc.description
        else:
            valid &= ARTICLE not in body and "[Mẫu] Mưa trên phố cũ" not in body and not doc.canonical
        if case in ("hidden", "withdrawn"):
            valid &= doc.noindex and response.getheader("Cache-Control") == "no-store"
        return dict(status=response.status, headers_ms=headers, article_ms=article_ms, chunks=chunks,
                    bytes=len(data), static_html_contract=bool(valid), secondary_fallback=secondary_fallback, canonical=bool(doc.canonical),
                    noindex=doc.noindex, description=doc.description)
    finally:
        conn.close()


@contextlib.contextmanager
def child(args, **kwargs):
    p = subprocess.Popen(args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, **kwargs)
    try:
        yield p
    finally:
        p.terminate()
        try:
            p.wait(timeout=3)
        except subprocess.TimeoutExpired:
            p.kill()
            p.wait()


def ready(predicate, child):
    for _ in range(250):
        if child.poll() is not None:
            raise RuntimeError("fixture exited before readiness")
        if predicate():
            return
        time.sleep(.02)
    raise TimeoutError("loopback readiness")


def listening(port):
    with socket.socket() as sock:
        return sock.connect_ex(("127.0.0.1", port)) == 0


def sample(binary, caddy, mode, delay, case, critical_ms):
    with tempfile.TemporaryDirectory() as tmp:
        folder = Path(tmp)
        address = folder / "origin"
        env = dict(os.environ, VINK_SSR_PROBE_FILE=str(address), VINK_SSR_PROBE_MODE=mode,
                   VINK_SSR_PROBE_CRITICAL_MS=str(critical_ms), VINK_SSR_PROBE_DELAY=str(delay), VINK_SSR_PROBE_CASE=case)
        with child([binary, "--ignored", "--exact", "reader_chunk_probe"], env=env) as fixture:
            ready(lambda: address.exists() and address.stat().st_size, fixture)
            origin = address.read_text()
            with socket.socket() as sock:
                sock.bind(("127.0.0.1", 0))
                port = sock.getsockname()[1]
            config = folder / "Caddyfile"
            config.write_text("{\n admin off\n persist_config off\n}\nhttp://127.0.0.1:%d {\n"
                              " reverse_proxy %s {\n flush_interval -1\n }\n}\n" % (port, origin))
            with child([caddy, "run", "--config", str(config)], cwd=folder) as proxy:
                ready(lambda: listening(port), proxy)
                return [dict(mode=mode, critical_ms=critical_ms, delay_ms=delay, case=case, via=via, **read(host, case))
                        for via, host in [("direct", origin), ("caddy", f"127.0.0.1:{port}")]]


if __name__ == "__main__":
    binary, caddy = (str(Path(arg).resolve()) for arg in sys.argv[1:])
    rows = []
    for mode in ["async", "out-of-order", "async"]:
        for case in ["published", "hidden", "withdrawn", "missing", "error"]:
            for delay in ([0, 200, 1000] if case == "published" else [0]):
                for critical_ms in [0, 30]:
                    rows.extend(sample(binary, caddy, mode, delay, case, critical_ms))
    print(json.dumps(rows, indent=2))
    assert all(row["static_html_contract"] for row in rows if row["mode"] == "async")
