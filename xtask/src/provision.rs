//! `verify postgres` checks for services/postgres provisioning, on the test harness server.
//!
//! The operator script runs with scratch names for three owners: an App database first populated
//! the pre-provisioning way (one superuser migrates and seeds it), a fresh App database and an
//! Identity database. The outcome is then checked with the provisioned credentials themselves —
//! the real binaries migrate and seed, and each role attempts connections and DDL — rather than
//! with the script's own post-checks. Scratch names never match a real owner; they are removed
//! before and after every run.

use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::{AssertSqlSafe, Connection, Executor, PgConnection};

use crate::pgurl::PgUrl;
use crate::process::Report;
use crate::web;

const FRESH: &str = "vink_verify_prov_fresh";
const LEGACY: &str = "vink_verify_prov_legacy";
const IDENTITY: &str = "vink_verify_prov_identity";

/// Data written before provisioning; the title is NFD on purpose, so any rewrite would show.
const LEGACY_AUTHOR: &str = "55555555-5555-4555-8555-555555555555";
const LEGACY_WORK: &str = "66666666-6666-4666-8666-666666666666";
const LEGACY_REVISION: &str = "77777777-7777-4777-8777-777777777777";
const LEGACY_TITLE: &str = "[Mẫu] Đe\u{302}\u{301}m trước khi chuyển quyền";
const LEGACY_DOCUMENT: &str = r#"{"schema_version":1,"blocks":[{"lines":["Dòng một: ầ ẩ ẫ ậ ắ ằ ẳ ẵ ặ","  thụt lề hai khoảng"]},{"lines":["Khổ hai — Đ đ ơ ư"]}]}"#;

const PRIVILEGE: &[&str] = &["42501"];

struct Owner {
    database: &'static str,
    migrator: String,
    runtime: String,
}

impl Owner {
    fn new(database: &'static str) -> Self {
        Self {
            database,
            migrator: format!("{database}_migrator"),
            runtime: format!("{database}_runtime"),
        }
    }
}

struct Run<'a> {
    root: &'a Path,
    admin: PgUrl,
    admin_url: String,
    password: String,
    fresh: Owner,
    legacy: Owner,
    identity: Owner,
}

pub fn verify(report: &mut Report, root: &Path, admin_url: &str) -> bool {
    let admin = match PgUrl::parse(admin_url) {
        Ok(admin) => admin,
        Err(error) => {
            report.fail(
                "test harness URL",
                &format!("VINK_TEST_DATABASE_URL: {error}"),
            );
            return false;
        }
    };
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let run = Run {
        root,
        admin,
        admin_url: admin_url.to_string(),
        password: format!("verify-{:x}-{nonce:x}", std::process::id()),
        fresh: Owner::new(FRESH),
        legacy: Owner::new(LEGACY),
        identity: Owner::new(IDENTITY),
    };
    if !report.step(
        "psql 15+ available (services/postgres/provision.sh)",
        psql_version,
    ) || !report.step("scratch owners absent before the run", || run.cleanup())
    {
        return false;
    }
    let ok = run.steps(report);
    let cleaned = report.step("scratch databases and roles removed", || run.cleanup());
    ok && cleaned
}

fn psql_version() -> Result<(), String> {
    let out = Command::new("psql")
        .arg("--version")
        .output()
        .map_err(|_| "psql not found; install a PostgreSQL 15+ client".to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let major: u32 = text
        .split_whitespace()
        .find_map(|word| word.split('.').next()?.parse().ok())
        .unwrap_or(0);
    if major >= 15 {
        println!("==> {text}");
        Ok(())
    } else {
        Err(format!("psql 15+ is required (\\getenv); found `{text}`"))
    }
}

fn block_on<T>(future: impl Future<Output = Result<T, String>>) -> Result<T, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?
        .block_on(future)
}

async fn connect(url: &str) -> Result<PgConnection, String> {
    PgConnection::connect(url)
        .await
        .map_err(|e| format!("connect: {e}"))
}

fn sqlstate(error: &sqlx::Error) -> Option<String> {
    match error {
        sqlx::Error::Database(db) => db.code().map(|code| code.into_owned()),
        _ => None,
    }
}

/// `sql` must fail with one of `codes`; success means the role holds a privilege it must not.
async fn denied(connection: &mut PgConnection, sql: &str, codes: &[&str]) -> Result<(), String> {
    match connection.execute(AssertSqlSafe(sql.to_string())).await {
        Ok(_) => Err(format!("`{sql}` succeeded but must be refused")),
        Err(error) if sqlstate(&error).is_some_and(|c| codes.contains(&c.as_str())) => Ok(()),
        Err(error) => Err(format!("`{sql}` failed unexpectedly: {error}")),
    }
}

fn tail(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(8)..].join("\n    ")
}

impl Run<'_> {
    fn url(&self, role: &str, database: &str) -> String {
        self.admin.with(role, &self.password, database)
    }

    fn admin_on(&self, database: &str) -> String {
        self.admin.with_database(database)
    }

    fn steps(&self, report: &mut Report) -> bool {
        let mut before = String::new();
        report.step(
            "legacy App database: one superuser migrates and seeds it (pre-provisioning layout)",
            || {
                self.legacy_layout()?;
                before = self.snapshot(LEGACY)?;
                Ok(())
            },
        ) && report.step(
            "provision.sh adopts the legacy App database and creates fresh App + Identity",
            || {
                self.provision(&self.legacy, false, &self.password)?;
                self.provision(&self.fresh, false, &self.password)
            },
        ) && report.step(
            "rerun with other passwords changes no password; --check passes",
            || self.rerun(),
        ) && report.step(
            "migrate: runtime credential refused without fallback; migrator applies and reruns",
            || self.migrations(),
        ) && report.step(
            "legacy IDs, Unicode, drafts, revisions, publication pointer and history unchanged",
            || self.unchanged(&before),
        ) && report.step(
            "runtime roles: DML only; no DDL, TEMP, history writes or other owner databases",
            || self.isolation(),
        ) && report.step(
            "negative control: a stray CREATE grant fails --check and is visible to the probe",
            || self.negative_control(),
        ) && report.step(
            "worker starts with the runtime role, reports a current schema, stops on SIGTERM",
            || web::worker_lifecycle(self.root, &self.url(&self.fresh.runtime, FRESH), "current"),
        )
    }

    /// Drops the scratch databases and roles; harmless when they do not exist.
    fn cleanup(&self) -> Result<(), String> {
        let admin = self.admin_url.clone();
        let mut statements: Vec<String> = [FRESH, LEGACY, IDENTITY]
            .iter()
            .map(|db| format!("DROP DATABASE IF EXISTS {db} WITH (FORCE)"))
            .collect();
        for owner in [&self.fresh, &self.legacy, &self.identity] {
            for role in [&owner.migrator, &owner.runtime] {
                statements.push(format!("DROP ROLE IF EXISTS {role}"));
            }
        }
        block_on(async move {
            let mut connection = connect(&admin).await?;
            for statement in statements {
                connection
                    .execute(AssertSqlSafe(statement.clone()))
                    .await
                    .map_err(|e| format!("{statement}: {e}"))?;
            }
            Ok(())
        })
    }

    fn app(&self, args: &[&str], env: &[(&str, String)]) -> Result<Output, String> {
        Command::new(web::binary(self.root, "vink-app"))
            .args(args)
            .current_dir(self.root)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("VINK_LOG_FORMAT", "text")
            .envs(env.iter().map(|(k, v)| (k, v)))
            .output()
            .map_err(|e| format!("vink-app {}: {e}", args.join(" ")))
    }

    fn app_ok(&self, args: &[&str], env: &[(&str, String)]) -> Result<(), String> {
        let out = self.app(args, env)?;
        if out.status.success() {
            println!("==> vink-app {} ok", args.join(" "));
            Ok(())
        } else {
            Err(format!(
                "vink-app {} exited with {}:\n    {}",
                args.join(" "),
                out.status,
                tail(&out.stderr)
            ))
        }
    }

    fn migrate(&self, url: String) -> Result<(), String> {
        self.app_ok(&["migrate"], &[("VINK_MIGRATE_DATABASE_URL", url)])
    }

    fn seed(&self, url: String) -> Result<(), String> {
        self.app_ok(
            &["dev-seed"],
            &[("DATABASE_URL", url), ("VINK_PROFILE", "local".into())],
        )
    }

    fn provision(&self, app: &Owner, check: bool, password: &str) -> Result<(), String> {
        let script = self.root.join("services/postgres/provision.sh");
        let mut command = Command::new("sh");
        command
            .arg(&script)
            .current_dir(self.root)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", std::env::var_os("HOME").unwrap_or_default())
            .envs(self.admin.psql_env())
            .env("VINK_APP_DATABASE", app.database)
            .env("VINK_APP_MIGRATOR_ROLE", &app.migrator)
            .env("VINK_APP_RUNTIME_ROLE", &app.runtime)
            .env("VINK_IDENTITY_DATABASE", self.identity.database)
            .env("VINK_IDENTITY_MIGRATOR_ROLE", &self.identity.migrator)
            .env("VINK_IDENTITY_RUNTIME_ROLE", &self.identity.runtime)
            .stdout(Stdio::inherit())
            .stderr(Stdio::piped());
        if check {
            command.arg("--check");
        }
        if !password.is_empty() {
            for key in [
                "VINK_APP_MIGRATOR_PASSWORD",
                "VINK_APP_RUNTIME_PASSWORD",
                "VINK_IDENTITY_MIGRATOR_PASSWORD",
                "VINK_IDENTITY_RUNTIME_PASSWORD",
            ] {
                command.env(key, password);
            }
        }
        let out = command
            .output()
            .map_err(|e| format!("{}: {e}", script.display()))?;
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stderr).lines() {
                // psql prefixes notices with the script path; keep the message.
                println!(
                    "    {}",
                    line.rsplit_once(": NOTICE:  ").map_or(line, |(_, m)| m)
                );
            }
            Ok(())
        } else {
            Err(format!(
                "provision.sh{} for {} exited with {}:\n    {}",
                if check { " --check" } else { "" },
                app.database,
                out.status,
                tail(&out.stderr)
            ))
        }
    }

    fn legacy_layout(&self) -> Result<(), String> {
        let admin = self.admin_url.clone();
        block_on(async move {
            let mut connection = connect(&admin).await?;
            connection
                .execute(AssertSqlSafe(format!("CREATE DATABASE {LEGACY}")))
                .await
                .map_err(|e| format!("create {LEGACY}: {e}"))?;
            Ok(())
        })?;
        self.migrate(self.admin_on(LEGACY))?;
        self.seed(self.admin_on(LEGACY))?;
        let url = self.admin_on(LEGACY);
        block_on(async move {
            let mut connection = connect(&url).await?;
            let mut tx = connection.begin().await.map_err(|e| e.to_string())?;
            sqlx::query("INSERT INTO principals (id, display_name) VALUES ($1::uuid, $2)")
                .bind(LEGACY_AUTHOR)
                .bind("[Mẫu] Người viết trước provisioning")
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            sqlx::query("INSERT INTO works (id, owner_id) VALUES ($1::uuid, $2::uuid)")
                .bind(LEGACY_WORK)
                .bind(LEGACY_AUTHOR)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            sqlx::query(
                "INSERT INTO working_drafts (work_id, version, title, document)
                 VALUES ($1::uuid, 7, $2, $3::text::jsonb)",
            )
            .bind(LEGACY_WORK)
            .bind(LEGACY_TITLE)
            .bind(LEGACY_DOCUMENT)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
            sqlx::query(
                "INSERT INTO revisions (id, work_id, title, document, content_hash, created_by)
                 VALUES ($1::uuid, $2::uuid, $3, $4::text::jsonb, repeat('ab', 32), $5::uuid)",
            )
            .bind(LEGACY_REVISION)
            .bind(LEGACY_WORK)
            .bind(LEGACY_TITLE)
            .bind(LEGACY_DOCUMENT)
            .bind(LEGACY_AUTHOR)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
            sqlx::query(
                "INSERT INTO publications (work_id, revision_id, generation, published_at)
                 VALUES ($1::uuid, $2::uuid, 3, '2026-11-01T00:00:00Z')",
            )
            .bind(LEGACY_WORK)
            .bind(LEGACY_REVISION)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
            tx.commit().await.map_err(|e| e.to_string())
        })
    }

    /// Every row the App owns plus the full migration history, as text.
    fn snapshot(&self, database: &str) -> Result<String, String> {
        let url = self.admin_on(database);
        block_on(async move {
            let mut connection = connect(&url).await?;
            sqlx::query_scalar(
                "SELECT json_build_object(
                    'principals', (SELECT json_agg(t ORDER BY t.id) FROM principals t),
                    'works', (SELECT json_agg(t ORDER BY t.id) FROM works t),
                    'working_drafts', (SELECT json_agg(t ORDER BY t.work_id) FROM working_drafts t),
                    'revisions', (SELECT json_agg(t ORDER BY t.id) FROM revisions t),
                    'publications', (SELECT json_agg(t ORDER BY t.work_id) FROM publications t),
                    'history', (SELECT json_agg(json_build_object(
                                    'version', h.version, 'description', h.description,
                                    'installed_on', h.installed_on, 'success', h.success,
                                    'checksum', encode(h.checksum, 'hex'),
                                    'execution_time', h.execution_time) ORDER BY h.version)
                                  FROM _sqlx_migrations h))::text",
            )
            .fetch_one(&mut connection)
            .await
            .map_err(|e| format!("snapshot {database}: {e}"))
        })
    }

    /// Number of scratch roles and their stored password verifiers (compared, never printed).
    fn verifiers(&self) -> Result<(i64, String), String> {
        let admin = self.admin_url.clone();
        block_on(async move {
            let mut connection = connect(&admin).await?;
            sqlx::query_as(
                "SELECT count(*), coalesce(string_agg(rolname || '=' || coalesce(rolpassword, '-'),
                                                      E'\\n' ORDER BY rolname), '')
                   FROM pg_authid WHERE rolname LIKE 'vink\\_verify\\_prov\\_%'",
            )
            .fetch_one(&mut connection)
            .await
            .map_err(|e| format!("read pg_authid: {e}"))
        })
    }

    fn rerun(&self) -> Result<(), String> {
        let before = self.verifiers()?;
        if before.0 != 6 {
            return Err(format!(
                "expected six scratch roles after provisioning, found {}",
                before.0
            ));
        }
        let other = format!("{}-other", self.password);
        self.provision(&self.legacy, false, &other)?;
        self.provision(&self.fresh, false, &other)?;
        if self.verifiers()? != before {
            return Err("a rerun changed an existing role password".into());
        }
        self.provision(&self.legacy, true, "")?;
        self.provision(&self.fresh, true, "")
    }

    fn migrations(&self) -> Result<(), String> {
        let out = self.app(
            &["migrate"],
            &[(
                "VINK_MIGRATE_DATABASE_URL",
                self.url(&self.fresh.runtime, FRESH),
            )],
        )?;
        let stderr = String::from_utf8_lossy(&out.stderr);
        if out.status.code() != Some(1) || !stderr.contains("lacks a privilege") {
            return Err(format!(
                "migrate as the runtime role: expected exit 1 naming the missing privilege, got {}:\n    {}",
                out.status,
                tail(&out.stderr)
            ));
        }
        let url = self.admin_on(FRESH);
        let untouched: bool = block_on(async move {
            let mut connection = connect(&url).await?;
            sqlx::query_scalar("SELECT to_regclass('public._sqlx_migrations') IS NULL")
                .fetch_one(&mut connection)
                .await
                .map_err(|e| e.to_string())
        })?;
        if !untouched {
            return Err("a refused migration left a history table behind".into());
        }
        println!("==> migrate as the runtime role refused (exit 1), nothing created");
        self.migrate(self.url(&self.fresh.migrator, FRESH))?;
        self.migrate(self.url(&self.fresh.migrator, FRESH))?;
        self.migrate(self.url(&self.legacy.migrator, LEGACY))?;
        self.seed(self.url(&self.fresh.runtime, FRESH))
    }

    fn unchanged(&self, before: &str) -> Result<(), String> {
        if self.snapshot(LEGACY)? != before {
            return Err("legacy rows or migration history differ after provisioning".into());
        }
        let url = self.url(&self.legacy.runtime, LEGACY);
        block_on(async move {
            let mut connection = connect(&url).await?;
            let (title, document, revision): (String, String, String) = sqlx::query_as(
                "SELECT d.title, d.document::text, p.revision_id::text
                   FROM working_drafts d JOIN publications p ON p.work_id = d.work_id
                  WHERE d.work_id = $1::uuid",
            )
            .bind(LEGACY_WORK)
            .fetch_one(&mut connection)
            .await
            .map_err(|e| format!("read legacy draft as runtime: {e}"))?;
            let expected: serde_json::Value =
                serde_json::from_str(LEGACY_DOCUMENT).map_err(|e| e.to_string())?;
            let stored: serde_json::Value =
                serde_json::from_str(&document).map_err(|e| e.to_string())?;
            if title != LEGACY_TITLE || stored != expected || revision != LEGACY_REVISION {
                return Err("legacy draft/publication read back differently".into());
            }
            Ok(())
        })
    }

    fn isolation(&self) -> Result<(), String> {
        let app_runtimes = [
            (self.url(&self.fresh.runtime, FRESH), FRESH),
            (self.url(&self.legacy.runtime, LEGACY), LEGACY),
        ];
        for (url, database) in app_runtimes {
            block_on(async move {
                app_runtime(&url)
                    .await
                    .map_err(|e| format!("{database}: {e}"))
            })?;
        }
        let identity = self.url(&self.identity.runtime, IDENTITY);
        block_on(async move {
            let mut connection = connect(&identity).await?;
            denied(
                &mut connection,
                "CREATE TABLE vink_probe (id int)",
                PRIVILEGE,
            )
            .await?;
            denied(
                &mut connection,
                "CREATE TEMP TABLE vink_probe (id int)",
                PRIVILEGE,
            )
            .await
        })?;
        // The same credential with only the database name changed must be refused.
        let crossings = [
            (&self.legacy.runtime, IDENTITY),
            (&self.legacy.runtime, FRESH),
            (&self.fresh.runtime, LEGACY),
            (&self.identity.runtime, LEGACY),
            (&self.identity.runtime, FRESH),
            (&self.legacy.migrator, IDENTITY),
            (&self.identity.migrator, LEGACY),
        ];
        for (role, database) in crossings {
            let url = self.url(role, database);
            let role = role.clone();
            block_on(async move {
                match PgConnection::connect(&url).await {
                    Ok(_) => Err(format!("{role} connected to {database}")),
                    Err(error) if sqlstate(&error).as_deref() == Some("42501") => Ok(()),
                    Err(error) => Err(format!("{role} → {database}: unexpected {error}")),
                }
            })?;
        }
        let migrator = self.url(&self.fresh.migrator, FRESH);
        block_on(async move {
            let mut connection = connect(&migrator).await?;
            denied(&mut connection, "CREATE DATABASE vink_probe", PRIVILEGE).await?;
            denied(&mut connection, "CREATE ROLE vink_probe", PRIVILEGE).await
        })?;
        println!("==> runtime/migrator privileges and cross-database refusals as expected");
        Ok(())
    }

    fn negative_control(&self) -> Result<(), String> {
        let admin = self.admin_on(LEGACY);
        let grant = format!("GRANT CREATE ON SCHEMA public TO {}", self.legacy.runtime);
        let revoke = format!(
            "REVOKE CREATE ON SCHEMA public FROM {}",
            self.legacy.runtime
        );
        let exec = |sql: String| {
            let admin = admin.clone();
            block_on(async move {
                let mut connection = connect(&admin).await?;
                connection
                    .execute(AssertSqlSafe(sql.clone()))
                    .await
                    .map_err(|e| format!("{sql}: {e}"))?;
                Ok(())
            })
        };
        exec(grant)?;
        let caught = self.provision(&self.legacy, true, "").is_err();
        let runtime = self.url(&self.legacy.runtime, LEGACY);
        let probe = block_on(async move {
            let mut connection = connect(&runtime).await?;
            connection
                .execute("CREATE TABLE vink_probe (id int)")
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        });
        exec("DROP TABLE IF EXISTS vink_probe".into())?;
        exec(revoke)?;
        if !caught {
            return Err("provision.sh --check passed with a stray CREATE grant".into());
        }
        probe.map_err(|e| format!("the probe could not observe the stray grant: {e}"))?;
        println!("==> stray grant: --check failed and the runtime role could create a table");
        self.provision(&self.legacy, true, "")
    }
}

/// Checks one App runtime role from its own connection.
async fn app_runtime(url: &str) -> Result<(), String> {
    let mut connection = connect(url).await?;
    let works: i64 = sqlx::query_scalar("SELECT count(*) FROM works")
        .fetch_one(&mut connection)
        .await
        .map_err(|e| format!("read works: {e}"))?;
    if works == 0 {
        return Err("no works visible to the runtime role".into());
    }
    let mut tx = connection.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("INSERT INTO principals (id, display_name) VALUES (gen_random_uuid(), 'probe')")
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("runtime INSERT: {e}"))?;
    tx.rollback().await.map_err(|e| e.to_string())?;
    let elevated: bool = sqlx::query_scalar(
        "SELECT rolsuper OR rolcreatedb OR rolcreaterole OR rolreplication OR rolbypassrls
                OR EXISTS (SELECT FROM pg_auth_members WHERE member = r.oid)
           FROM pg_roles r WHERE rolname = current_user",
    )
    .fetch_one(&mut connection)
    .await
    .map_err(|e| e.to_string())?;
    if elevated {
        return Err("runtime role has elevated attributes or memberships".into());
    }
    for sql in [
        "CREATE TABLE vink_probe (id int)",
        "CREATE TEMP TABLE vink_probe (id int)",
        "CREATE SCHEMA vink_probe",
        "ALTER TABLE works ADD COLUMN vink_probe int",
        "DROP TABLE publications",
        "TRUNCATE works",
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
         VALUES (1, 'probe', true, '\\x00', 0)",
        "UPDATE _sqlx_migrations SET success = false",
        "DELETE FROM _sqlx_migrations",
    ] {
        denied(&mut connection, sql, PRIVILEGE).await?;
    }
    // Not installable by the runtime; a server without contrib reports it unavailable instead.
    denied(
        &mut connection,
        "CREATE EXTENSION dblink",
        &["42501", "58P01", "0A000"],
    )
    .await
}
