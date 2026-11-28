//! Private, disposable receiver for the opt-in real publication-to-inbox browser journey.
use super::{Browser, Server, smoke_with_env};
use crate::{env::clear_capabilities, identity::sql, pgurl::PgUrl, process};
use std::{path::Path, process::Command};

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    root: &Path,
    app: &Server,
    admin_url: &str,
    chrome: &Browser,
    state: &Path,
    outbox: &str,
    origin: &str,
    sockets: &Path,
) -> Result<(), String> {
    let admin = PgUrl::parse(admin_url)?;
    let name = format!("vink_verify_web_receiver_{}", std::process::id());
    let password = format!(
        "receiver-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    );
    let db = admin.with_database(&name);
    sql(admin_url, &format!("CREATE DATABASE {name}"))?;
    if let Err(err) = sql(
        admin_url,
        &format!("CREATE ROLE {name} LOGIN PASSWORD '{password}'"),
    ) {
        sql(admin_url, &format!("DROP DATABASE {name}"))?;
        return Err(err);
    }
    let result = (|| {
        let mut cmd = Command::new(app.binary(root, "vink-notifications"));
        clear_capabilities(&mut cmd);
        cmd.arg("migrate")
            .env("VINK_NOTIFICATIONS_MIGRATE_DATABASE_URL", &db);
        process::run(&mut cmd, "migrate scratch Notifications database")?;
        for statement in [
            format!("REVOKE ALL ON DATABASE {name} FROM PUBLIC"),
            format!("GRANT CONNECT ON DATABASE {name} TO {name}"),
            "REVOKE CREATE ON SCHEMA public FROM PUBLIC".into(),
            format!("GRANT USAGE ON SCHEMA public TO {name}"),
            format!("GRANT SELECT,INSERT,UPDATE,DELETE ON ALL TABLES IN SCHEMA public TO {name}"),
            format!("GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO {name}"),
            format!("REVOKE INSERT,UPDATE,DELETE ON _sqlx_migrations FROM {name}"),
        ] {
            sql(&db, &statement)?;
        }
        process::run(
            &mut app.command(
                root,
                &[
                    "notifications",
                    "activate",
                    "evaluator-message browser scenario",
                ],
            ),
            "activate scratch notifications",
        )?;
        let mut settings = vec![
            (
                "VINK_E2E_WORKER_BIN".into(),
                app.binary(root, "vink-worker").display().to_string(),
            ),
            (
                "VINK_E2E_RECEIVER_BIN".into(),
                app.binary(root, "vink-notifications").display().to_string(),
            ),
            (
                "VINK_E2E_RECEIVER_DB".into(),
                admin.with(&name, &password, &name),
            ),
            ("VINK_E2E_SOCKET_DIR".into(), sockets.display().to_string()),
        ];
        settings.extend(app.env.iter().filter(|(k, _)| k == "DATABASE_URL").cloned());
        smoke_with_env(
            root,
            &["--phase", "evaluator-message", "--outbox", outbox],
            chrome,
            origin,
            state,
            &settings,
        )
    })();
    let dropped = sql(admin_url, &format!("DROP DATABASE {name} WITH (FORCE)"));
    let role = sql(admin_url, &format!("DROP ROLE {name}"));
    result.and(dropped.map(|_| ())).and(role.map(|_| ()))
}
