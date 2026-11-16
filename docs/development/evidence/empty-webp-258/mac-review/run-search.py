"""One-off #266 public-search evidence with an owned, readiness-checked local server."""
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.request

stem = sys.argv[1]
base = Path('review-266')
env = dict(os.environ, DATABASE_URL='postgres://vink266@127.0.0.1:55566/vink266_focused',
           VINK_PROFILE='local', VINK_AUTH_SIGN_IN='disabled',
           VINK_DEV_PRINCIPAL='11111111-1111-4111-8111-111111111111',
           VINK_BIND='127.0.0.1:55570', VINK_PUBLIC_ORIGIN='http://127.0.0.1:55570',
           VINK_SITE_ROOT=str(Path('target/site').resolve()))
with (base / f'{stem}-server.log').open('w') as server_log:
    server = subprocess.Popen(['target/debug/vink-app', 'serve'], env=env,
                              stdout=server_log, stderr=server_log)
    try:
        for _ in range(100):
            if server.poll() is not None:
                raise RuntimeError(f'server exited {server.returncode}')
            try:
                if urllib.request.urlopen('http://127.0.0.1:55570/health/ready', timeout=1).status == 200:
                    break
            except Exception:
                time.sleep(.1)
        else:
            raise RuntimeError('server not ready')
        with (base / f'{stem}.log').open('w') as log:
            result = subprocess.run([
                'node', 'apps/vink-web/e2e/browser-smoke.mjs',
                '--base', 'http://127.0.0.1:55570', '--chrome',
                '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
                '--state', 'review-266/guard-state.json', '--phase', 'public-search',
            ], stdout=log, stderr=log)
        print(f'Browser exit {result.returncode}; server live={server.poll() is None}')
    finally:
        if server.poll() is None:
            server.terminate()
        server.wait(timeout=15)
sys.exit(result.returncode)
