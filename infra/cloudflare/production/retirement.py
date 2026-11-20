"""Validate an operator's reviewed retirement inventory. Reads files only."""
import argparse
import json
from pathlib import Path
from artifacts import require

KINDS = {"test-host", "test-origin", "pages-project", "pages-url", "custom-domain", "dns-record",
         "caddy-vhost", "systemd", "containers", "network", "volume", "database", "backup",
         "bucket", "config-reader", "credentials", "shared-build", "auth-qualification"}


def validate(data):
    require(data["public_origin"] == "https://vink.forum" and data["production_receipt"],
            "production qualification receipt required")
    items = data["resources"]
    require(KINDS == {item["kind"] for item in items}, "incomplete retirement inventory")
    require(all(item["id"] and item["evidence"] for item in items), "resource identity/readback required")
    require(any(x["kind"] == "test-host" and x["id"] == "test.vink.forum" for x in items)
            and any(x["kind"] == "test-origin" and x["id"] == "origin-test.vink.forum" for x in items),
            "wrong test target")
    for item in items:
        action = item["action"]
        require(action in {"retain", "detach", "stop", "restrict", "gone"}, "unsupported retirement action")
        require(item["owner"] in {"shared", "test"}, "unresolved owner")
        require(isinstance(item["callers"], list), "caller inventory required")
        if item["kind"] in {"volume", "database", "backup", "shared-build", "bucket", "auth-qualification"} or item["owner"] == "shared":
            require(action == "retain", "preserve shared and durable resources")
        if action != "retain":
            require(not item["callers"], f"remaining callers: {item['id']}")
        if item["kind"] in {"test-host", "test-origin", "pages-project", "pages-url", "custom-domain"}:
            require(action in {"gone", "restrict", "detach", "stop"}, "public app still reachable")
    return [(i["kind"], i["action"], i["id"]) for i in items]


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("inventory", type=Path)
    print(json.dumps(validate(json.loads(parser.parse_args().inventory.read_bytes())), ensure_ascii=False))
