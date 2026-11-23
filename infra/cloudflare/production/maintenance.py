"""Opt-in, local origin maintenance. The caller holds release.py's host lock."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time

from artifacts import require, sha

STATE = Path("/var/lib/vink-production-release")
GUARD = STATE / "origin-maintenance.json"
RELEASES = Path("/opt/vink-prod/releases")
CURRENT = RELEASES.parent / "current"
ENV = Path("/etc/vink-prod/release.env")
RUNTIME = Path("/etc/vink-prod/runtime")
BACKUPS = Path("/var/backups/vink-prod")
OWNERS = {"vink": ("vink-app", "app"), "vink_identity": ("vink-identity", "identity")}
SERVICES = {"app": ("APP", "web"), "worker": ("WORKER", "worker"), "identity": ("IDENTITY", "identity")}


def durable(path, data):
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as out:
        scratch = Path(out.name)
        try:
            out.write(data)
            out.flush()
            os.fsync(out.fileno())
            os.replace(scratch, path)
            sync_dir(path.parent)
        finally:
            scratch.unlink(missing_ok=True)


def sync_dir(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def write_state(path, state, phase, **fields):
    state.update(phase=phase, **fields)
    durable(path, json.dumps(state, sort_keys=True).encode())


def ledger_files(releases, rev, owner):
    files = sorted((releases / rev / "services" / owner / "migrations").glob("*.sql"))
    ledger = {}
    for file in files:
        version = file.name.split("_", 1)[0]
        require(version.isascii() and version.isdecimal() and version not in ledger and file.is_file()
                and not file.is_symlink(), "invalid migration inventory")
        ledger[version] = hashlib.sha384(file.read_bytes()).hexdigest()
    require(ledger, "missing migration inventory")
    return dict(sorted(ledger.items(), key=lambda entry: int(entry[0])))


def matches_ledger(applied, old, new):
    return list(applied.items()) == list(new.items())[:len(applied)] and old.items() <= applied.items()


def readback(command, **kw):
    return subprocess.check_output(command, text=True, **kw).strip()


def boot_fence():
    startup = readback(["systemctl", "show", "--value", "-p", "ExecStartPre", "vink-prod.service"])
    require(f"argv[]=/usr/bin/test ! -e {GUARD} ;" in startup, "install the Vink systemd start fence")
    restart = subprocess.run(["systemctl", "is-enabled", "podman-restart.service"],
                             capture_output=True, text=True, check=False)
    require(restart.stdout.strip() in {"disabled", "masked", "not-found"}, "Podman boot restart must be inactive")
    require(subprocess.run(["systemctl", "is-active", "--quiet", "podman-restart.service"]).returncode in {3, 4},
            "Podman boot restart must be inactive")


def observed_overlay(unit, labels, flags, release_env):
    base = [str(CURRENT / path) for path in ("services/postgres/compose.yaml", "infra/production/compose.yaml")]
    resend = base + [str(CURRENT / "infra/production/auth-resend.compose.yaml")]
    declared = re.findall(r"(?:^| )-f ([^ ;]+)", unit)
    require(declared in (base, resend), "unknown Vink startup Compose files")
    enabled = declared == resend
    expected = ",".join(declared)
    require(labels["identity"] == expected and all(
        labels[service] in {",".join(base), expected} for service in ("app", "worker")),
        "running Compose labels differ from the Vink unit")
    sign_in = "enabled" if enabled and b"VINK_PROD_AUTH_SIGN_IN=enabled" in release_env.splitlines() else "disabled"
    require(flags["app"].get("VINK_AUTH_SIGN_IN") == sign_in
            and flags["worker"].get("VINK_AUTH_SIGN_IN") == "disabled", "running App/Worker auth differs")
    require(not enabled or (flags["identity"].get("VINK_IDENTITY_AUTHORITY") == "native"
            and flags["identity"].get("VINK_IDENTITY_MAIL_TRANSPORT") == "resend-api"), "running Identity overlay differs")
    require(enabled or flags["identity"].get("VINK_IDENTITY_MAIL_TRANSPORT") != "resend-api",
            "running Resend transport requires the unit overlay")
    return enabled


def checked_backup():
    backup = Path(os.environ["VINK_VERIFIED_BACKUP"]).resolve()
    require(backup.parent == BACKUPS and re.fullmatch(r"\d{8}T\d{6}Z", backup.name), "invalid verified backup")
    sums = backup / "SHA256SUMS"
    require(0 <= time.time() - sums.stat().st_mtime < 86400, "verified backup is older than 24h")
    require({line.split()[-1] for line in sums.read_text().splitlines()} == {"vink.dump", "vink_identity.dump"},
            "backup is not the two-owner production pair")
    subprocess.run(["sha256sum", "--check", "--strict", "SHA256SUMS"], cwd=backup, check=True,
                   stdout=subprocess.DEVNULL)
    return sha(sums.read_bytes())


def switch_sql(metadata, names):
    """Swap both owner DBs atomically; retain the migrated DBs, disconnected."""
    statements = ["BEGIN", "SET LOCAL lock_timeout = '5s'"]
    for owner in OWNERS:
        saved, failed = names[owner]
        original, copy = metadata[owner]
        statements.append(
            f"DO $$ BEGIN IF (SELECT oid FROM pg_database WHERE datname='{owner}') "
            f"IS DISTINCT FROM {original}::oid OR (SELECT oid FROM pg_database WHERE datname='{saved}') "
            f"IS DISTINCT FROM {copy}::oid OR EXISTS (SELECT FROM pg_database WHERE datname='{failed}') "
            "THEN RAISE EXCEPTION 'rollback database identities changed'; END IF; END $$")
        statements.extend([f"ALTER DATABASE {owner} ALLOW_CONNECTIONS false",
                           f"ALTER DATABASE {owner} RENAME TO {failed}",
                           f"ALTER DATABASE {saved} RENAME TO {owner}",
                           f"ALTER DATABASE {owner} ALLOW_CONNECTIONS true"])
    return ";\n".join(statements + ["COMMIT;"])


class Origin:
    def __init__(self, old, spec, old_images):
        self.old, self.new = old, spec["source_sha"]
        self.spec, self.old_images = spec, old_images
        self.work = STATE / f"maintenance-{self.new}"
        self.state_path = self.work / "state.json"
        self.names = {db: (f"{db}_saved_{self.new[:16]}", f"{db}_failed_{self.new[:16]}") for db in OWNERS}
        self.postgres = self.container("postgres", running=True)
        self.before = {db: ledger_files(RELEASES, old, owner) for db, (owner, _) in OWNERS.items()}
        self.after = {db: ledger_files(RELEASES, self.new, owner) for db, (owner, _) in OWNERS.items()}

    def container(self, name, running=False):
        args = ["podman", "ps", "-q" if running else "-aq",
                "--filter", "label=io.podman.compose.project=vink-prod",
                "--filter", f"label=com.docker.compose.service={name}"]
        result = readback(args)
        require(re.fullmatch(r"[0-9a-f]{12,64}", result), f"expected one Vink {name} container")
        return result

    def sql(self, query, db="postgres"):
        return readback(["podman", "exec", "-i", self.postgres, "psql", "-XAtq", "-v", "ON_ERROR_STOP=1",
                         "-U", "vink", "-d", db], input=query)

    def ledger(self, db):
        rows = self.sql("SELECT version, success, encode(checksum, 'hex') "
                        "FROM public._sqlx_migrations ORDER BY version;", db)
        ledger = {}
        for row in rows.splitlines():
            version, success, checksum = row.split("|")
            require(success == "t" and version not in ledger, "failed or duplicate migration")
            ledger[version] = checksum
        return ledger

    def check_ledgers(self, expected):
        for db in OWNERS:
            require(self.ledger(db) == expected[db], f"{db} migration history differs")

    def db_oids(self):
        names = [*OWNERS, *(name for pair in self.names.values() for name in pair)]
        names_sql = ", ".join(f"'{name}'" for name in names)
        rows = self.sql(f"SELECT datname, oid FROM pg_database WHERE datname IN ({names_sql});")
        return {name: int(oid) for name, oid in (row.split("|") for row in rows.splitlines())}

    def drain(self):
        for service in SERVICES:
            ref = self.old_images[SERVICES[service][1]]
            cid = self.container(service)
            expected = readback(["podman", "image", "inspect", "--format", "{{.Id}}", ref])
            running, image_id, _ = readback(["podman", "inspect", "--format",
                                             "{{.State.Running}} {{.Image}} {{.State.ExitCode}}", cid]).split()
            require(image_id == expected, f"unexpected predecessor: {service}")
            if running == "true":
                subprocess.run(["podman", "stop", "-t", "20", cid], check=True)
            require(readback(["podman", "inspect", "--format", "{{.State.Running}} {{.State.ExitCode}}", cid])
                    == "false 0", f"ungraceful stop: {service}; inspect before continuing")
        self.require_drained()

    def require_drained(self):
        for service in SERVICES:
            require(readback(["podman", "inspect", "--format", "{{.State.Running}} {{.State.ExitCode}}",
                              self.container(service)]) == "false 0", f"{service} is running or did not drain")
        require(self.sql("SELECT count(*) FROM pg_stat_activity WHERE datname IN ('vink','vink_identity');") == "0",
                "owner DB still has clients")

    def backup(self):
        dest = self.work / "backup"
        if not dest.exists():
            with tempfile.TemporaryDirectory(dir=self.work) as tmp:
                pair = Path(tmp) / "pair"
                pair.mkdir()
                sums = []
                for db in OWNERS:
                    dump = pair / (db + ".dump")
                    with dump.open("wb") as out:
                        subprocess.run(["podman", "exec", self.postgres, "pg_dump", "-Fc", "-U", "vink", db],
                                       stdout=out, check=True)
                        out.flush()
                        os.fsync(out.fileno())
                    with dump.open("rb") as stream:
                        subprocess.run(["podman", "exec", "-i", self.postgres, "pg_restore", "--list"], stdin=stream,
                                       stdout=subprocess.DEVNULL, check=True)
                    with dump.open("rb") as stream:
                        sums.append(f"{hashlib.file_digest(stream, 'sha256').hexdigest()}  {dump.name}\n")
                durable(pair / "SHA256SUMS", "".join(sums).encode())
                pair.rename(dest)
                sync_dir(self.work)
        subprocess.run(["sha256sum", "--check", "--strict", "SHA256SUMS"], cwd=dest, check=True,
                       stdout=subprocess.DEVNULL)
        return sha((dest / "SHA256SUMS").read_bytes())

    def clone_rollback(self, originals):
        self.require_drained()
        observed = self.db_oids()
        for db, (_, role) in OWNERS.items():
            saved, failed = self.names[db]
            require(observed.get(db) == originals[db] and failed not in observed, "owner database identity changed")
            # Database-level ACL is not copied by CREATE DATABASE TEMPLATE.
            attrs = self.sql(f"SELECT datdba = 'vink'::regrole AND datallowconn AND datconnlimit = -1 "
                             f"AND NOT datistemplate AND NOT EXISTS (SELECT FROM pg_db_role_setting "
                             f"WHERE setdatabase = pg_database.oid) FROM pg_database WHERE datname = '{db}';")
            require(attrs == "t", "unsupported database owner settings")
            if saved not in observed:
                self.sql("SET lock_timeout = '5s';\n"
                         f"CREATE DATABASE {saved} WITH TEMPLATE {db} OWNER vink ALLOW_CONNECTIONS false;")
            self.sql(f"REVOKE ALL ON DATABASE {saved} FROM PUBLIC;"
                     f"GRANT CONNECT ON DATABASE {saved} TO vink_{role}_migrator, vink_{role}_runtime;")
            acl = ("SELECT ARRAY(SELECT entry::text FROM "
                   "unnest(coalesce(datacl, acldefault('d',datdba))) entry ORDER BY 1) "
                   "FROM pg_database WHERE datname = ")
            same_acl = self.sql(f"SELECT ({acl}'{db}') = ({acl}'{saved}');")
            require(same_acl == "t", "rollback copy database ACL differs")
            require(self.sql(f"SELECT NOT datallowconn AND datdba = 'vink'::regrole FROM pg_database "
                             f"WHERE datname='{saved}';") == "t", "rollback copy is not fenced")
        observed = self.db_oids()
        return {db: [originals[db], observed[self.names[db][0]]] for db in OWNERS}

    def container_args(self, envfile):
        return ["podman", "run", "--rm", "--pull=never", "--read-only", "--cap-drop=ALL",
                "--security-opt=no-new-privileges", "--user", "10001:10001", "--cpu-shares=128",
                "--network", f"container:{self.postgres}", "--env-file", str(RUNTIME / envfile)]

    def migrate(self):
        for db, (_, role) in OWNERS.items():
            require(matches_ledger(self.ledger(db), self.before[db], self.after[db]), f"{db} diverged during migration")
            image = self.spec["images"]["web" if role == "app" else role]
            subprocess.run([*self.container_args(role + "-migrator.env"), image, "migrate"],
                           stdin=subprocess.DEVNULL, check=True)
        self.check_ledgers(self.after)

    def private_ready(self, images):
        probe = f"vink-release-probe-{self.new[:16]}"
        with tempfile.TemporaryDirectory(dir=self.work) as socket_dir:
            os.chown(socket_dir, 10001, 10001)
            modes = [
                ("web", "app-runtime.env", ["-e", "VINK_PROFILE=production", "-e", "VINK_PUBLIC_ORIGIN=https://vink.forum",
                                           "-e", "VINK_AUTH_SIGN_IN=disabled", "-e", "VINK_BIND=127.0.0.1:13000"],
                 ["http://127.0.0.1:13000/health/ready"]),
                ("identity", "identity-runtime.env",
                 ["-v", f"{socket_dir}:/probe", "-e", "VINK_IDENTITY_AUTHORITY=password",
                  "-e", "VINK_IDENTITY_SOCKET=/probe/identity.sock", "-e", "VINK_IDENTITY_CALLER_UID=10001"],
                 ["--unix-socket", "/probe/identity.sock", "http://identity/health/ready"]),
            ]
            for image, envfile, args, url in modes:
                # Never remove an unknown container left by an earlier interrupted command.
                cid = readback([*self.container_args(envfile), "--detach", "--name", probe, *args, images[image]],
                               stdin=subprocess.DEVNULL)
                try:
                    for _ in range(30):
                        output = subprocess.run(["podman", "exec", cid, "curl", "-fsS", "--max-time", "2", *url],
                                                capture_output=True, text=True)
                        if output.returncode == 0:
                            health = json.loads(output.stdout)
                            expected = {"database": "ok", "schema": "current", "role": "restricted"}
                            require(health["status"] == "ready" and all(
                                health["checks"].get(key) == value for key, value in expected.items()),
                                f"private {image} readiness differs")
                            break
                        time.sleep(1)
                    else:
                        raise ValueError(f"private {image} did not become ready")
                finally:
                    subprocess.run(["podman", "stop", "-t", "10", cid], check=True)
        self.require_drained()

    def prepare(self, state, evidence):
        if state["phase"] == "draining":
            self.drain()
            self.check_ledgers(self.before)
            require(checked_backup() == evidence["backup_sha256"], "backup differs from restore rehearsal")
            backup_digest = self.backup()
            oids = self.db_oids()
            require(set(oids) == set(OWNERS), "rollback database names already exist")
            write_state(self.state_path, state, "copying", originals=oids, backup_sha256=backup_digest)
        if state["phase"] == "copying":
            metadata = self.clone_rollback(state["originals"])
            write_state(self.state_path, state, "migrating", databases=metadata)
        if state["phase"] == "migrating":
            self.require_drained()
            require(self.backup() == state["backup_sha256"], "drained backup changed")
            self.migrate()
            self.private_ready(self.spec["images"])
            write_state(self.state_path, state, "prepared")
        require(state["phase"] == "prepared", "origin preparation is not resumable at this phase")

    def compose(self, overlay):
        paths = ["services/postgres/compose.yaml", "infra/production/compose.yaml"]
        if overlay:
            paths.append("infra/production/auth-resend.compose.yaml")
        argv = ["podman", "compose", "--project-name", "vink-prod", "--env-file", str(ENV)]
        for path in paths:
            argv.extend(["-f", str(CURRENT / path)])
        return argv

    def env(self, images):
        old = (self.work / "release-before.env").read_text()
        for key, image in SERVICES.values():
            name = f"VINK_PROD_{key}_IMAGE"
            old, count = re.subn(rf"(?m)^{name}=.*$", f"{name}={images[image]}", old)
            require(count == 1, "release image variable missing or duplicated")
        return old.encode()

    def observe(self, rev, images):
        args = [images[key] for key in ("web", "worker", "identity")]
        script = RELEASES / self.new / "infra/cloudflare/production/origin.sh"
        return subprocess.run([script, "observe", rev, rev, *args, self.spec["platform"]],
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0

    def start(self, state, rev, images, starting, complete):
        target = RELEASES / rev
        require(readback(["git", "-C", target, "rev-parse", "HEAD"]) == rev and
                not readback(["git", "-C", target, "status", "--porcelain"]), "origin start tree changed")
        require(CURRENT.resolve() in (RELEASES / self.old, target), "origin tree changed")
        new_env = self.env(images)
        require(ENV.read_bytes() in ((self.work / "release-before.env").read_bytes(), new_env),
                "release environment changed")
        # starting is durable before either path changes or any process can accept writes.
        if CURRENT.resolve() != target or ENV.read_bytes() != new_env:
            durable(ENV, new_env)
            with tempfile.TemporaryDirectory(dir=CURRENT.parent) as tmp:
                link = Path(tmp) / "current"
                link.symlink_to(target)
                os.replace(link, CURRENT)
                sync_dir(CURRENT.parent)
        if not self.observe(rev, images):
            subprocess.run([*self.compose(state["overlay"]), "up", "-d", "--no-build", "--no-deps",
                            "identity", "app", "worker"], check=True)
        require(self.observe(rev, images), "origin readiness not observed; resume forward only")
        if state["phase"] == starting:
            write_state(self.state_path, state, complete)
        if GUARD.exists():
            GUARD.unlink()
            sync_dir(GUARD.parent)

    def open(self, state):
        if state["phase"] == "prepared":
            self.require_drained()
            self.check_ledgers(self.after)
            require(self.backup() == state["backup_sha256"], "drained backup changed")
            oids = self.db_oids()
            require(all(oids.get(db) == state["databases"][db][0] and
                        oids.get(self.names[db][0]) == state["databases"][db][1] for db in OWNERS),
                    "rollback databases are missing or changed")
            self.private_ready(self.spec["images"])
            write_state(self.state_path, state, "opening")
        require(state["phase"] in {"opening", "opened"}, "origin cannot open")
        self.start(state, self.new, self.spec["images"], "opening", "opened")

    def rollback(self, state):
        require(state["phase"] in {"draining", "copying", "migrating", "prepared", "restoring",
                                   "rollback_opening", "rolled_back"},
                "writes may have reopened; preserve both DBs and reconcile forward")
        if state["phase"] in {"draining", "copying", "migrating", "prepared"}:
            self.drain()
            if "databases" not in state:
                self.check_ledgers(self.before)
            else:
                require(self.backup() == state["backup_sha256"], "drained backup changed")
                write_state(self.state_path, state, "restoring")
        if state["phase"] == "restoring":
            self.require_drained()
            metadata = state["databases"]
            observed = self.db_oids()
            if all(observed.get(db) == metadata[db][0] and
                   observed.get(self.names[db][0]) == metadata[db][1] for db in OWNERS):
                self.sql(switch_sql(metadata, self.names))
            observed = self.db_oids()
            require(all(observed.get(db) == metadata[db][1] and
                        observed.get(self.names[db][1]) == metadata[db][0] for db in OWNERS),
                    "rollback database identities did not match")
            self.check_ledgers(self.before)
        if state["phase"] not in {"rollback_opening", "rolled_back"}:
            self.private_ready(self.old_images)
            write_state(self.state_path, state, "rollback_opening")
        self.start(state, self.old, self.old_images, "rollback_opening", "rolled_back")

    def preflight(self, evidence):
        boot_fence()
        require(CURRENT.resolve() == RELEASES / self.old, "wrong maintenance predecessor")
        require(self.sql("SELECT count(*) FROM pg_database WHERE datname='vink_notifications';") == "0",
                "this maintenance path supports App and Identity only")
        require(set(self.db_oids()) == set(OWNERS), "rollback database names already exist")
        require(checked_backup() == evidence["backup_sha256"], "backup differs from restore rehearsal")
        data = int(self.sql("SELECT sum(pg_database_size(datname)) FROM pg_database "
                            "WHERE datname IN ('vink','vink_identity');"))
        pg_disk = readback(["podman", "exec", self.postgres, "sh", "-c", 'df -Pk "$PGDATA"'])
        require(int(pg_disk.splitlines()[-1].split()[3]) * 1024 >= 5 * 1024**3 + data,
                "insufficient PostgreSQL disk for maintenance")
        fs = os.statvfs(STATE)
        require(fs.f_bavail * fs.f_frsize >= 5 * 1024**3 + 2 * data,
                "insufficient disk for maintenance backup")
        current_env = ENV.read_bytes()
        require({b"VINK_PROD_SECRETS_DIR=/etc/vink-prod/runtime", b"VINK_PROD_UID=10001"}
                <= set(current_env.splitlines()), "unknown runtime env directory or UID")
        labels, flags = {}, {}
        template = '{{index .Config.Labels "com.docker.compose.project.config_files"}}'
        for service in SERVICES:
            cid = self.container(service)
            labels[service] = readback(["podman", "inspect", "--format", template, cid])
            values = readback(["podman", "inspect", "--format", "{{range .Config.Env}}{{println .}}{{end}}", cid])
            # Only read back mode flags. Never put env values in the journal or output.
            flags[service] = {key: value for key, _, value in
                              (line.partition("=") for line in values.splitlines())
                              if key in {"VINK_AUTH_SIGN_IN", "VINK_IDENTITY_AUTHORITY", "VINK_IDENTITY_MAIL_TRANSPORT"}}
        unit = readback(["systemctl", "show", "--value", "-p", "ExecStart", "vink-prod.service"])
        self.check_ledgers(self.before)
        return current_env, observed_overlay(unit, labels, flags, current_env)

    def operate(self, mode, evidence):
        boot_fence()
        os.environ["PODMAN_COMPOSE_PROVIDER"] = "/usr/bin/podman-compose"
        with (BACKUPS / ".lock").open("a") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            key = {"from": self.old, "release": self.spec, "old_images": self.old_images, "review": evidence}
            if not self.state_path.exists():
                require(mode == "prepare" and not GUARD.exists(), "origin maintenance has not been prepared")
                current_env, overlay = self.preflight(evidence)
                self.work.mkdir(exist_ok=True)
                durable(self.work / "release-before.env", current_env)
                require(self.env(self.old_images) == current_env, "predecessor image env drift")
                state = key | {"overlay": overlay}
                write_state(self.state_path, state, "draining")
            else:
                state = json.loads(self.state_path.read_bytes())
                require(all(state.get(field) == value for field, value in key.items()), "maintenance inputs changed")
            if GUARD.exists():
                require(GUARD.read_text() == self.new, "another origin maintenance owns the restart fence")
            elif state["phase"] not in {"opened", "rolled_back"}:
                require(state["phase"] == "draining", "maintenance restart fence was removed")
                durable(GUARD, self.new.encode())
            if mode == "prepare":
                self.prepare(state, evidence)
            else:
                {"open": self.open, "rollback": self.rollback}[mode](state)
