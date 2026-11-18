#!/usr/bin/env python3
"""Generate Vink production or scratch database credentials into a new private directory."""

import argparse
import os
from pathlib import Path
import secrets


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    os.umask(0o077)
    args.directory.mkdir(mode=0o700)
    passwords = {key: secrets.token_hex(32) for key in (
        "bootstrap", "app_runtime", "app_migrator", "identity_runtime", "identity_migrator"
    )}
    host = "10.89.3.10:5432"
    app_url = f"postgres://vink_app_runtime:{passwords['app_runtime']}@{host}/vink"
    files = {
        "postgres-bootstrap-password": passwords["bootstrap"],
        "app-runtime.env": f"DATABASE_URL={app_url}\n",
        "worker-runtime.env": f"DATABASE_URL={app_url}\n",
        "identity-runtime.env": (
            f"VINK_IDENTITY_DATABASE_URL=postgres://vink_identity_runtime:"
            f"{passwords['identity_runtime']}@{host}/vink_identity\n"
        ),
        "app-migrator.env": (
            f"VINK_MIGRATE_DATABASE_URL=postgres://vink_app_migrator:"
            f"{passwords['app_migrator']}@{host}/vink\n"
        ),
        "identity-migrator.env": (
            f"VINK_IDENTITY_MIGRATE_DATABASE_URL=postgres://vink_identity_migrator:"
            f"{passwords['identity_migrator']}@{host}/vink_identity\n"
        ),
        "provision.env": f"PGPASSWORD={passwords['bootstrap']}\n" + "".join(
            f"VINK_{key.upper()}_PASSWORD={passwords[key]}\n"
            for key in ("app_runtime", "app_migrator", "identity_runtime", "identity_migrator")
        ),
    }
    for name, content in files.items():
        with (args.directory / name).open("x", encoding="utf-8") as target:
            target.write(content)
    # Only the PostgreSQL bind mount is visible to UID 70. The host parent stays 0700.
    (args.directory / "postgres-bootstrap-password").chmod(0o444)


if __name__ == "__main__":
    main()
