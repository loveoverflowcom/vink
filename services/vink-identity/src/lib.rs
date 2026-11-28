//! Vink Identity: a separate process that owns authentication facts and credential/security state
//! (accounts, verified addresses, one-time challenges, sessions, the outbox of codes) in its own
//! database, serving a private contract on a Unix socket. Without configuration it answers
//! `authority_not_configured`. The native email-code authority (`authority`, `email_code`,
//! `accounts`) mails codes through a durable outbox and worker (`delivery`) over a transport
//! (`mail`: Resend HTTPS or a local outbox for development). D-05 is still Provisional: a working mail
//! path is not an accepted production authority (see `docs/architecture/auth-authority.md`).
//!
//! Identity grants no Vink permission (admin, editor, publish) and never reads App data. It issues
//! and verifies session credentials; the App owns principals, the subject → principal binding,
//! suspension, authorization and works. Neither crate imports the other.
//!
//! # Levels
//!
//! Modules depend only downward; the `levels` test below fails on any other edge.
//!
//! | Level | Modules | Role |
//! | --- | --- | --- |
//! | 0 | `email_code`, `password`, `persistence` | Pure credential policy, migrator and pools |
//! | 1 | `contract`, `mail` | Wire types; mail transports |
//! | 2 | `outcome`, `delivery`, `config` | Use-case outcomes; outbox and worker; configuration |
//! | 3 | `accounts` | Transactions over the Identity database |
//! | 4 | `authority` | Use cases: method gate and orchestration; the store stays inside |
//! | 5 | `http` | Contract transport: parse, call one use case, map the outcome |
//! | 6 | `server`, `runtime`, `main` | Socket, process lifecycle and composition |
//!
//! Every module is public because the `vink-identity` binary and the PostgreSQL integration tests
//! are separate crates; items that only this crate uses are `pub(crate)`.

pub mod accounts;
pub mod authority;
pub mod config;
pub mod contract;
pub mod delivery;
pub mod email_code;
pub mod http;
pub mod mail;
pub mod outcome;
pub mod password;
pub mod persistence;
pub mod runtime;
pub mod server;

/// Owner-local levelization check: each module may name (`crate::…`, outside comments and its
/// `tests` module) only the modules listed for it. Adding an edge is a design change: update the
/// table and the level table above together.
#[cfg(test)]
mod levels {
    const ALLOWED: &[(&str, &[&str])] = &[
        ("email_code", &[]),
        ("password", &[]),
        ("persistence", &[]),
        ("contract", &["password"]),
        ("mail", &["email_code"]),
        ("outcome", &["contract", "email_code"]),
        ("delivery", &["email_code", "mail"]),
        ("config", &["mail"]),
        (
            "accounts",
            &[
                "contract",
                "delivery",
                "email_code",
                "mail",
                "outcome",
                "password",
            ],
        ),
        (
            "authority",
            &["accounts", "contract", "email_code", "outcome", "password"],
        ),
        (
            "http",
            &[
                "authority",
                "contract",
                "delivery",
                "email_code",
                "outcome",
                "password",
                "persistence",
            ],
        ),
        ("server", &["http"]),
        ("runtime", &["config"]),
    ];

    const SOURCES: &[(&str, &str)] = &[
        ("accounts", include_str!("accounts.rs")),
        ("accounts", include_str!("accounts/deletion.rs")),
        ("authority", include_str!("authority.rs")),
        ("config", include_str!("config.rs")),
        ("contract", include_str!("contract.rs")),
        ("delivery", include_str!("delivery.rs")),
        ("email_code", include_str!("email_code.rs")),
        ("http", include_str!("http.rs")),
        ("http", include_str!("http/deletion.rs")),
        ("mail", include_str!("mail.rs")),
        ("mail", include_str!("mail/resend.rs")),
        ("outcome", include_str!("outcome.rs")),
        ("password", include_str!("password.rs")),
        ("persistence", include_str!("persistence.rs")),
        ("runtime", include_str!("runtime.rs")),
        ("server", include_str!("server.rs")),
    ];

    /// Source lines before the unit-test module, without comment lines.
    fn production_code(source: &str) -> impl Iterator<Item = &str> {
        let mut lines = source.lines().peekable();
        std::iter::from_fn(move || {
            loop {
                let line = lines.next()?;
                if line.trim() == "#[cfg(test)]"
                    && lines
                        .peek()
                        .is_some_and(|next| next.starts_with("mod tests"))
                {
                    return None;
                }
                if !line.trim_start().starts_with("//") {
                    return Some(line);
                }
            }
        })
    }

    fn crate_references(line: &str) -> impl Iterator<Item = &str> {
        line.match_indices("crate::").map(move |(at, prefix)| {
            let rest = &line[at + prefix.len()..];
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            &rest[..end]
        })
    }

    #[test]
    fn every_module_depends_only_on_its_allowed_lower_levels() {
        let mut violations = Vec::new();
        for (module, source) in SOURCES {
            let allowed = ALLOWED
                .iter()
                .find(|(name, _)| name == module)
                .map(|(_, allowed)| *allowed)
                .unwrap_or_else(|| panic!("module `{module}` has no row in ALLOWED"));
            for line in production_code(source) {
                for target in crate_references(line) {
                    if target != *module && !allowed.contains(&target) {
                        violations.push(format!("{module} -> {target}: {}", line.trim()));
                    }
                }
            }
        }
        assert!(violations.is_empty(), "{violations:#?}");
    }

    #[test]
    fn the_wire_contract_and_use_case_outcomes_do_not_know_sql() {
        for (module, source) in SOURCES {
            if ["contract", "outcome", "authority"].contains(module) {
                assert!(
                    !production_code(source).any(|line| line.contains("sqlx")),
                    "{module} names sqlx"
                );
            }
        }
    }

    #[test]
    fn the_check_sees_real_edges() {
        let lines: Vec<_> = production_code(include_str!("http.rs")).collect();
        assert!(
            lines
                .iter()
                .flat_map(|line| crate_references(line))
                .any(|target| target == "authority")
        );
        assert!(crate_references("use crate::accounts::{AccountStore};").eq(["accounts"]));
    }
}
