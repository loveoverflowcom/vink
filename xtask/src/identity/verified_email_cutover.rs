//! Upgrade rehearsal on the provisioner's disposable, isolated Identity/App pair.
use super::{Run, sql};
use sqlx::PgPool;
use std::ffi::OsStr;
use std::time::{Duration, Instant};

const PREVIOUS: i64 = 20261205000000;
const VALID: &str = "60700000-0000-4000-8000-000000000001";
const BOUND: &str = "60700000-0000-4000-8000-000000000002";
const UNBOUND: &str = "60700000-0000-4000-8000-000000000003";
const PRINCIPAL: &str = "60700000-0000-4000-8000-000000000004";

impl Run<'_> {
    pub(super) fn rehearse_verified_email_cutover(&self) -> Result<String, String> {
        let s = self.scratch;
        let migrations = s.root.join("services/vink-identity/migrations");
        let remaining = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| err.to_string())?
            .block_on(async {
                let migrator = sqlx::migrate::Migrator::new(migrations.as_path())
                    .await
                    .map_err(|e| e.to_string())?;
                let pool = PgPool::connect(&s.identity_migrator)
                    .await
                    .map_err(|_| "could not connect to scratch Identity")?;
                migrator
                    .run_to(PREVIOUS, &pool)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<_, String>(
                    migrator
                        .iter()
                        .filter(|m| m.version > PREVIOUS)
                        .map(|m| m.version)
                        .collect::<Vec<_>>(),
                )
            })?;
        sql(
            &s.identity_runtime,
            &format!(
                "INSERT INTO accounts (id,disabled_at) VALUES ('{VALID}',now()),('{BOUND}',NULL),('{UNBOUND}',NULL)"
            ),
        )?;
        sql(
            &s.identity_runtime,
            &format!(
                "INSERT INTO account_emails (account_id,address,verified_at) VALUES ('{VALID}','valid-cutover@example.test','2026-01-01 00:00:00+00'),('{BOUND}','malformed',now())"
            ),
        )?;
        sql(
            &s.identity_runtime,
            &format!(
                "INSERT INTO sessions (id,account_id,token_hash,expires_at) VALUES (gen_random_uuid(),'{BOUND}',decode(repeat('01',32),'hex'),now()+interval '1 hour')"
            ),
        )?;
        let statements = [
            format!(
                "INSERT INTO principals (id, display_name) VALUES ('{PRINCIPAL}','Cutover scratch')"
            ),
            format!(
                "INSERT INTO principal_bindings(authority,subject_id,principal_id) VALUES ('vink-identity','{BOUND}','{PRINCIPAL}')"
            ),
            format!(
                "INSERT INTO author_handle_names(handle,principal_id) VALUES ('cutover-scratch','{PRINCIPAL}')"
            ),
            format!(
                "INSERT INTO author_profiles(principal_id,current_handle) VALUES ('{PRINCIPAL}','cutover-scratch')"
            ),
        ];
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?
            .block_on(async {
                use sqlx::{AssertSqlSafe, Connection, Executor, PgConnection};
                let mut conn = PgConnection::connect(&s.app_runtime)
                    .await
                    .map_err(|_| "App scratch fixture connection failed")?;
                let mut tx = conn.begin().await.map_err(|e| e.to_string())?;
                for stmt in statements {
                    (&mut *tx)
                        .execute(AssertSqlSafe(stmt))
                        .await
                        .map_err(|e| e.to_string())?;
                }
                tx.commit().await.map_err(|e| e.to_string())
            })?;
        self.review_cutover("inventory", "examined=3 invalid=2 apply=false")?;
        self.review_cutover("retire", "examined=3 invalid=2 apply=true")?;
        self.review_cutover("retire", "examined=1 invalid=0 apply=true")?;
        if self.migrate("before-drain", &s.identity_migrator)?.0 != Some(1) {
            return Err("cutover opened with pending receipts".into());
        }
        let uid = self.uid()?;
        let identity = self.start_with(
            "legacy-receipts",
            &s.identity_runtime,
            Some(uid),
            &[("VINK_IDENTITY_AUTHORITY", OsStr::new("password"))],
        )?;
        let (app, _) = self.start_app("legacy-reconciler", &s.app_runtime, uid)?;
        let deadline = Instant::now() + Duration::from_secs(60);
        while sql(
            &s.admin_identity,
            "SELECT count(*) FROM account_deletions WHERE applied_at IS NULL",
        )? != 0
        {
            if Instant::now() >= deadline {
                return Err("legacy receipts did not drain across the scratch pair".into());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        self.stop(app)?;
        self.stop(identity)?;
        for (url, statement, want) in [
            (
                &s.admin_app,
                format!(
                    "SELECT count(*) FROM account_deletions WHERE purged AND ((account_id='{BOUND}' AND principal_id='{PRINCIPAL}') OR (account_id='{UNBOUND}' AND principal_id IS NULL))"
                ),
                2,
            ),
            (
                &s.admin_app,
                format!(
                    "SELECT count(*) FROM principal_bindings b JOIN author_profiles p USING(principal_id) WHERE b.subject_id='{BOUND}' AND p.principal_id='{PRINCIPAL}' AND p.current_handle IS NULL"
                ),
                1,
            ),
            (
                &s.admin_identity,
                format!(
                    "SELECT count(*) FROM account_emails WHERE account_id='{VALID}' AND address='valid-cutover@example.test' AND verified_at='2026-01-01 00:00:00+00'"
                ),
                1,
            ),
            (
                &s.admin_identity,
                format!("SELECT count(*) FROM sessions WHERE account_id='{BOUND}'"),
                0,
            ),
        ] {
            if sql(url, &statement)? != want {
                return Err("legacy retirement postflight mismatch".into());
            }
        }
        println!(
            "==> verified-email cutover: examined 3, retired 2; retry 0; 2 App receipts purged and acknowledged; valid mailbox and immutable binding preserved"
        );
        Ok(format!("applied={remaining:?}"))
    }

    fn review_cutover(&self, mode: &str, expected: &str) -> Result<(), String> {
        let output = self
            .binary()
            .args(["maintenance", "verified-email", mode])
            .env("VINK_IDENTITY_DATABASE_URL", &self.scratch.identity_runtime)
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() || String::from_utf8_lossy(&output.stdout).trim() != expected {
            return Err(format!(
                "verified-email {mode}: unexpected scratch inventory"
            ));
        }
        Ok(())
    }
}
