//! `vink-app [serve|migrate|dev-seed|moderator …]` — HTTP/SSR server and explicit database commands.
//!
//! `serve`, `dev-seed` and `moderator` use the runtime credential (`DATABASE_URL`); `migrate` uses
//! only the migration credential (`VINK_MIGRATE_DATABASE_URL`). `moderator grant|revoke|list` is
//! the operator's only way to give or take comment-moderation authority; no HTTP route does it.
//! `principal suspend|reinstate|list` likewise is the only way to suspend a principal (D-05/S-03
//! spike). `tag list|history|create|rename|describe|alias|retire` is the only way to change the
//! curated tag catalog (TAG-PR-03, #97 CHOICE-02 A); each change names the catalog version the
//! operator last read and is audited in the same transaction. Exit codes: 0 success, 1 runtime
//! failure, 2 invalid configuration or usage, or a catalog change that was refused or conflicted
//! (nothing written).

use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::Arc;

use vink_app::application::Vink;
use vink_app::application::catalog::{
    CatalogError, CatalogRejection, CatalogRequest, ChangeCatalog, CuratorAuthority, facet_code,
};
use vink_app::config::{Config, MigrateConfig, Profile};
use vink_app::domain::discussion::moderation::GrantNote;
use vink_app::domain::ids::PrincipalId;
use vink_app::domain::principal::SuspensionReason;
use vink_app::domain::taxonomy::{
    CatalogCommand, CatalogTag, SlugKind, TagDescription, TagLabel, TagSlug, TagStatus,
};
use vink_app::http::{AppState, router};
use vink_app::persistence::{self, PgStore, lazy_pool};
use vink_app::{dev_seed, identity, runtime};

const USAGE: &str = "usage: vink-app [serve|migrate|dev-seed]\n       vink-app moderator grant <principal-uuid> <note>\n       vink-app moderator revoke <principal-uuid>\n       vink-app moderator list\n       vink-app principal suspend <principal-uuid> <reason>\n       vink-app principal reinstate <principal-uuid>\n       vink-app principal list\n       vink-app tag list\n       vink-app tag history [<limit>]\n       vink-app tag create <catalog-version> <form|theme> <label>\n       vink-app tag rename <catalog-version> <tag-uuid> <label>\n       vink-app tag describe <catalog-version> <tag-uuid> [<description>]\n       vink-app tag alias <catalog-version> <tag-uuid> <label>\n       vink-app tag retire <catalog-version> <tag-uuid> [<successor-uuid>]";

/// Default number of audit rows `tag history` prints.
const DEFAULT_HISTORY: i64 = 50;

/// Operator command on comment-moderation grants, parsed before any configuration is read.
#[derive(Debug, PartialEq, Eq)]
enum ModeratorCommand {
    Grant(PrincipalId, GrantNote),
    Revoke(PrincipalId),
    List,
}

fn parse_moderator(args: &[String]) -> Result<ModeratorCommand, String> {
    let principal = |raw: Option<&String>| {
        raw.and_then(|raw| PrincipalId::parse(raw))
            .ok_or_else(|| "expected a principal UUID".to_string())
    };
    match args.first().map(String::as_str) {
        Some("grant") => {
            let note = GrantNote::parse(&args.get(2..).unwrap_or_default().join(" "))
                .ok_or("grant needs a one-line note of at most 200 characters")?;
            Ok(ModeratorCommand::Grant(principal(args.get(1))?, note))
        }
        Some("revoke") if args.len() == 2 => Ok(ModeratorCommand::Revoke(principal(args.get(1))?)),
        Some("list") if args.len() == 1 => Ok(ModeratorCommand::List),
        _ => Err("expected `grant <principal> <note>`, `revoke <principal>` or `list`".into()),
    }
}

/// Operator command on principal suspensions, parsed before any configuration is read.
#[derive(Debug, PartialEq, Eq)]
enum PrincipalCommand {
    Suspend(PrincipalId, SuspensionReason),
    Reinstate(PrincipalId),
    List,
}

fn parse_principal(args: &[String]) -> Result<PrincipalCommand, String> {
    let principal = |raw: Option<&String>| {
        raw.and_then(|raw| PrincipalId::parse(raw))
            .ok_or_else(|| "expected a principal UUID".to_string())
    };
    match args.first().map(String::as_str) {
        Some("suspend") => {
            let reason = SuspensionReason::parse(&args.get(2..).unwrap_or_default().join(" "))
                .ok_or("suspend needs a one-line reason of at most 200 characters")?;
            Ok(PrincipalCommand::Suspend(principal(args.get(1))?, reason))
        }
        Some("reinstate") if args.len() == 2 => {
            Ok(PrincipalCommand::Reinstate(principal(args.get(1))?))
        }
        Some("list") if args.len() == 1 => Ok(PrincipalCommand::List),
        _ => {
            Err("expected `suspend <principal> <reason>`, `reinstate <principal>` or `list`".into())
        }
    }
}

/// Operator command on the curated tag catalog. A change is validated by the application here,
/// before any configuration is read or any connection is made.
#[derive(Debug, PartialEq, Eq)]
enum TagCommand {
    List,
    History(i64),
    Change(CatalogCommand),
}

fn parse_tag(args: &[String]) -> Result<TagCommand, String> {
    let rest = |from: usize| args.get(from..).unwrap_or_default().join(" ");
    let version = || -> Result<i64, String> {
        args.get(1)
            .and_then(|raw| raw.parse().ok())
            .ok_or_else(|| "expected the catalog version from `vink-app tag list`".to_string())
    };
    let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
    let request = match args.first().map(String::as_str) {
        Some("list") if args.len() == 1 => return Ok(TagCommand::List),
        Some("history") if args.len() <= 2 => {
            let limit = match args.get(1) {
                None => DEFAULT_HISTORY,
                Some(raw) => raw.parse().map_err(|_| "expected a history limit")?,
            };
            return Ok(TagCommand::History(limit));
        }
        Some("create") if args.len() >= 4 => CatalogRequest::Create {
            facet: arg(2),
            label: rest(3),
        },
        Some("rename") if args.len() >= 4 => CatalogRequest::Rename {
            tag: arg(2),
            label: rest(3),
        },
        Some("describe") if args.len() >= 3 => CatalogRequest::Describe {
            tag: arg(2),
            description: (args.len() > 3).then(|| rest(3)),
        },
        Some("alias") if args.len() >= 4 => CatalogRequest::AddAlias {
            tag: arg(2),
            label: rest(3),
        },
        Some("retire") if (3..=4).contains(&args.len()) => CatalogRequest::Retire {
            tag: arg(2),
            successor: args.get(3).cloned(),
        },
        _ => {
            return Err("expected `list`, `history [<limit>]`, or `create|rename|describe|alias|retire <catalog-version> …`".into());
        }
    };
    let change = ChangeCatalog {
        expected_version: version()?,
        request,
    };
    change
        .validate()
        .map(TagCommand::Change)
        .map_err(|error| format!("{}: {error}", error.field()))
}

enum Loaded {
    App(Config),
    Migrate(MigrateConfig),
}

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_else(|| "serve".into());
    if !matches!(
        command.as_str(),
        "serve" | "migrate" | "dev-seed" | "moderator" | "principal" | "tag"
    ) {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let moderator_command = if command == "moderator" {
        let args: Vec<String> = std::env::args().skip(2).collect();
        match parse_moderator(&args) {
            Ok(parsed) => Some(parsed),
            Err(error) => {
                eprintln!("vink-app moderator: {error}\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    } else {
        None
    };
    let principal_command = if command == "principal" {
        let args: Vec<String> = std::env::args().skip(2).collect();
        match parse_principal(&args) {
            Ok(parsed) => Some(parsed),
            Err(error) => {
                eprintln!("vink-app principal: {error}\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    } else {
        None
    };
    let tag_command = if command == "tag" {
        let args: Vec<String> = std::env::args().skip(2).collect();
        match parse_tag(&args) {
            Ok(parsed) => Some(parsed),
            Err(error) => {
                eprintln!("vink-app tag: {error}\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    } else {
        None
    };
    let loaded = match command.as_str() {
        "migrate" => MigrateConfig::from_env().map(Loaded::Migrate),
        _ => Config::from_env().map(Loaded::App),
    };
    let loaded = match loaded {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("vink-app: {error}");
            return ExitCode::from(2);
        }
    };
    runtime::init_tracing(match &loaded {
        Loaded::App(config) => config.log_format,
        Loaded::Migrate(config) => config.log_format,
    });
    let tokio = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(error) => {
            tracing::error!(%error, "cannot start async runtime");
            return ExitCode::FAILURE;
        }
    };
    let result = tokio.block_on(async {
        match loaded {
            Loaded::Migrate(config) => migrate(&config).await,
            Loaded::App(config) if command == "dev-seed" => seed(&config).await,
            Loaded::App(config) if let Some(operation) = moderator_command => {
                moderator(&config, operation).await
            }
            Loaded::App(config) if let Some(operation) = principal_command => {
                principal(&config, operation).await
            }
            Loaded::App(config) if let Some(operation) = tag_command => {
                tag(&config, operation).await
            }
            Loaded::App(config) => serve(config).await,
        }
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Usage(message)) => {
            tracing::error!("{message}");
            ExitCode::from(2)
        }
        Err(Failure::Runtime(message)) => {
            tracing::error!("{message}");
            ExitCode::FAILURE
        }
    }
}

enum Failure {
    Usage(String),
    Runtime(String),
}

async fn connect(database_url: &str, command: &'static str) -> Result<sqlx::PgPool, Failure> {
    persistence::connect(database_url, command)
        .await
        .map_err(|error| Failure::Runtime(format!("cannot connect to PostgreSQL: {error}")))
}

async fn migrate(config: &MigrateConfig) -> Result<(), Failure> {
    let pool = connect(&config.database_url, "vink-app migrate").await?;
    let report = persistence::migrate(&pool)
        .await
        .map_err(|error| Failure::Runtime(format!("migration failed: {error}")))?;
    pool.close().await;
    tracing::info!(applied = ?report.applied, latest = report.latest, "migrations applied");
    Ok(())
}

async fn seed(config: &Config) -> Result<(), Failure> {
    if config.profile != Profile::Local {
        return Err(Failure::Usage(
            "dev-seed writes synthetic data and requires VINK_PROFILE=local".into(),
        ));
    }
    let pool = connect(&config.database_url, "vink-app dev-seed").await?;
    dev_seed::apply(&pool)
        .await
        .map_err(|error| Failure::Runtime(format!("seed failed (run migrate first?): {error}")))?;
    tracing::info!(
        work = %dev_seed::SAMPLE_WORK,
        author = %dev_seed::SAMPLE_AUTHOR,
        "development fixture present"
    );
    Ok(())
}

async fn moderator(config: &Config, command: ModeratorCommand) -> Result<(), Failure> {
    let pool = connect(&config.database_url, "vink-app moderator").await?;
    let unavailable = |error: vink_app::application::ports::StoreError| match error {
        vink_app::application::ports::StoreError::UnknownPrincipal => {
            Failure::Usage("the principal has no Vink record".into())
        }
        other => Failure::Runtime(format!("moderator command failed: {other}")),
    };
    match command {
        ModeratorCommand::Grant(principal, note) => {
            match persistence::grant_moderator(&pool, principal, &note)
                .await
                .map_err(unavailable)?
            {
                persistence::GrantOutcome::Granted(grant) => {
                    println!("granted {grant} to {principal}");
                }
                persistence::GrantOutcome::AlreadyActive(grant) => {
                    println!("{principal} already holds active grant {grant}; nothing changed");
                }
            }
        }
        ModeratorCommand::Revoke(principal) => {
            match persistence::revoke_moderator(&pool, principal)
                .await
                .map_err(unavailable)?
            {
                Some(grant) => println!("revoked {grant} from {principal}"),
                None => println!("{principal} holds no active grant; nothing changed"),
            }
        }
        ModeratorCommand::List => {
            for record in persistence::active_moderators(&pool)
                .await
                .map_err(unavailable)?
            {
                println!(
                    "{}\t{}\t{}\t{}",
                    record.principal, record.display_name, record.granted_at, record.note
                );
            }
        }
    }
    pool.close().await;
    Ok(())
}

async fn principal(config: &Config, command: PrincipalCommand) -> Result<(), Failure> {
    let pool = connect(&config.database_url, "vink-app principal").await?;
    let failed = |error: vink_app::application::ports::StoreError| match error {
        vink_app::application::ports::StoreError::UnknownPrincipal => {
            Failure::Usage("the principal has no Vink record".into())
        }
        other => Failure::Runtime(format!("principal command failed: {other}")),
    };
    match command {
        PrincipalCommand::Suspend(principal, reason) => {
            match persistence::suspend_principal(&pool, principal, &reason)
                .await
                .map_err(failed)?
            {
                persistence::SuspensionOutcome::Suspended(id) => {
                    println!("suspended {principal} ({id})");
                }
                persistence::SuspensionOutcome::AlreadySuspended(id) => {
                    println!("{principal} is already suspended ({id}); nothing changed");
                }
            }
        }
        PrincipalCommand::Reinstate(principal) => {
            match persistence::reinstate_principal(&pool, principal)
                .await
                .map_err(failed)?
            {
                Some(id) => println!("lifted suspension {id} of {principal}"),
                None => println!("{principal} is not suspended; nothing changed"),
            }
        }
        PrincipalCommand::List => {
            for record in persistence::active_suspensions(&pool)
                .await
                .map_err(failed)?
            {
                println!(
                    "{}\t{}\t{}\t{}",
                    record.principal, record.display_name, record.suspended_at, record.reason
                );
            }
        }
    }
    pool.close().await;
    Ok(())
}

fn dash(value: Option<impl std::fmt::Display>) -> String {
    value.map_or_else(|| "-".to_string(), |value| value.to_string())
}

fn tag_line(tag: &CatalogTag) -> String {
    let (status, successor) = match tag.status() {
        TagStatus::Active => ("active", None),
        TagStatus::Retired { successor } => ("retired", successor),
    };
    format!(
        "tag\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        tag.id(),
        facet_code(tag.facet()),
        status,
        tag.slug().as_str(),
        tag.label().as_str(),
        dash(successor),
        dash(tag.description().map(TagDescription::as_str)),
    )
}

const fn kind_name(kind: SlugKind) -> &'static str {
    match kind {
        SlugKind::Canonical => "canonical",
        SlugKind::Alias => "alias",
        SlugKind::Former => "former",
    }
}

fn refusal(rejection: CatalogRejection) -> String {
    match rejection {
        CatalogRejection::NotCurator => "no curator authority".into(),
        CatalogRejection::UnknownTag => "no tag has this id".into(),
        CatalogRejection::TagIdTaken => "the generated tag id is taken; run it again".into(),
        CatalogRejection::SlugTaken { holder } => {
            format!("the label's slug already names tag {holder} (its name, alias or former name)")
        }
        CatalogRejection::Retired => "the tag is retired; retirement is terminal".into(),
        CatalogRejection::AliasIsCanonicalName => "the alias spells the tag's own name".into(),
        CatalogRejection::InvalidSuccessor(error) => format!("invalid successor: {error:?}"),
    }
}

async fn tag(config: &Config, command: TagCommand) -> Result<(), Failure> {
    let pool = connect(&config.database_url, "vink-app tag").await?;
    let vink = Vink::new(PgStore::new(pool.clone()));
    let failed = |error: CatalogError| match error {
        CatalogError::Invalid(input) => Failure::Usage(format!("{}: {input}", input.field())),
        CatalogError::Conflict { current } => Failure::Usage(format!(
            "the catalog changed: it is at version {}; nothing was written. Read it again with `vink-app tag list` and decide again",
            current.get()
        )),
        CatalogError::Rejected(rejection) => Failure::Usage(format!(
            "refused: {}; nothing was written",
            refusal(rejection)
        )),
        other => Failure::Runtime(format!("tag command failed: {other}")),
    };
    match command {
        TagCommand::List => {
            let listing = vink.catalog_listing().await.map_err(failed)?;
            println!("catalog version\t{}", listing.version.get());
            for entry in &listing.entries {
                println!("{}", tag_line(&entry.tag));
                for name in &entry.names {
                    println!(
                        "name\t{}\t{}\t{}\t{}",
                        entry.tag.id(),
                        kind_name(name.kind),
                        name.slug.as_str(),
                        dash(name.alias_label.as_ref().map(TagLabel::as_str)),
                    );
                }
            }
        }
        TagCommand::History(limit) => {
            for record in vink.catalog_history(limit).await.map_err(failed)? {
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    record.version.get(),
                    record.recorded_at,
                    record.operation.name(),
                    record.tag,
                    dash(record.slug.as_ref().map(TagSlug::as_str)),
                    dash(record.successor),
                    if record.changed {
                        "changed"
                    } else {
                        "unchanged"
                    },
                    record.authority,
                    record.database_role,
                );
            }
        }
        TagCommand::Change(command) => {
            let changed = vink
                .change_catalog(CuratorAuthority::operator(), command)
                .await
                .map_err(failed)?;
            let write = &changed.write;
            println!(
                "catalog version {}: {} {}{}",
                write.version().get(),
                changed.operation.name(),
                changed.subject,
                if write.changed() {
                    ""
                } else {
                    " (same state; nothing else changed)"
                }
            );
            if let Some(tag) = write.tag() {
                println!("{}", tag_line(tag));
            }
            for slug in write.slugs() {
                println!(
                    "name\t{}\t{}\t{}",
                    slug.tag,
                    kind_name(slug.kind.slug_kind()),
                    slug.slug.as_str()
                );
            }
        }
    }
    pool.close().await;
    Ok(())
}

async fn serve(config: Config) -> Result<(), Failure> {
    let pool = lazy_pool(&config.database_url)
        .map_err(|error| Failure::Usage(format!("DATABASE_URL is not usable: {error}")))?;
    let directory = Arc::new(Vink::new(PgStore::new(pool.clone())));
    let resolver = identity::from_config(&config, directory);
    if config.dev_principal.is_some() {
        tracing::warn!(
            "local identity fixture enabled: loopback requests act as VINK_DEV_PRINCIPAL; this is not authentication"
        );
        if config.identity.is_some() {
            tracing::warn!(
                "the local fixture takes precedence; Identity is only probed by readiness"
            );
        }
    }
    let probe = config.identity.as_ref().map(|identity| {
        identity::IdentityClient::new(identity.socket.clone(), identity.server_uid)
    });
    let state = AppState::new(
        Vink::new(PgStore::new(pool.clone())),
        Arc::from(resolver),
        &config.public_origin,
        &config.site_root,
    )
    .with_sign_in(identity::sign_in_client(&config))
    .with_identity_probe(probe)
    .with_restricted_role_required(config.profile == Profile::Production);
    if !config.site_root.join("pkg").join("vink.js").exists() {
        tracing::warn!(
            site_root = %config.site_root.display(),
            "browser bundle missing; pages render but will not hydrate (run `cargo xtask build`)"
        );
    }
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(|error| Failure::Runtime(format!("cannot bind {}: {error}", config.bind)))?;
    tracing::info!(
        bind = %config.bind,
        origin = %config.public_origin,
        profile = ?config.profile,
        "vink-app listening"
    );
    axum::serve(
        listener,
        router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(runtime::shutdown_signal())
    .await
    .map_err(|error| Failure::Runtime(format!("server error: {error}")))?;
    pool.close().await;
    tracing::info!("vink-app stopped");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn principal_arguments_are_validated_before_any_configuration() {
        let principal = "22222222-2222-4222-8222-222222222222";
        let id = PrincipalId::parse(principal).unwrap();
        assert_eq!(
            parse_principal(&args(&["suspend", principal, "Spam", "lặp", "lại"])),
            Ok(PrincipalCommand::Suspend(
                id,
                SuspensionReason::parse("Spam lặp lại").unwrap()
            ))
        );
        assert_eq!(
            parse_principal(&args(&["reinstate", principal])),
            Ok(PrincipalCommand::Reinstate(id))
        );
        assert_eq!(
            parse_principal(&args(&["list"])),
            Ok(PrincipalCommand::List)
        );
        for bad in [
            &["suspend", principal][..],
            &["suspend", "not-a-uuid", "lý do"][..],
            &["reinstate"][..],
            &["reinstate", principal, "x"][..],
            &["delete", principal][..],
        ] {
            assert!(parse_principal(&args(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn moderator_arguments_are_validated_before_any_configuration() {
        let principal = "22222222-2222-4222-8222-222222222222";
        let id = PrincipalId::parse(principal).unwrap();
        assert_eq!(
            parse_moderator(&args(&["grant", principal, "Ban", "biên", "tập"])),
            Ok(ModeratorCommand::Grant(
                id,
                GrantNote::parse("Ban biên tập").unwrap()
            ))
        );
        assert_eq!(
            parse_moderator(&args(&["revoke", principal])),
            Ok(ModeratorCommand::Revoke(id))
        );
        assert_eq!(
            parse_moderator(&args(&["list"])),
            Ok(ModeratorCommand::List)
        );
        for bad in [
            &["grant", principal][..],
            &["grant", "not-a-uuid", "note"],
            &["revoke"],
            &["revoke", principal, "extra"],
            &["list", "extra"],
            &["promote", principal],
            &[],
        ] {
            assert!(parse_moderator(&args(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn tag_changes_are_validated_by_the_application_before_any_configuration() {
        use vink_app::domain::taxonomy::{CatalogChange, TagFacet};
        let tag = "a0000000-0000-4000-8000-000000000011";
        let Ok(TagCommand::Change(created)) =
            parse_tag(&args(&["create", "3", "theme", "  Ký", "ức "]))
        else {
            panic!("create must parse");
        };
        assert_eq!(created.expected.get(), 3);
        let CatalogChange::Create { facet, label, .. } = &created.change else {
            panic!("{created:?}");
        };
        assert_eq!((*facet, label.as_str()), (TagFacet::Theme, "Ký ức"));
        let Ok(TagCommand::Change(described)) = parse_tag(&args(&["describe", "4", tag])) else {
            panic!("describe without text clears");
        };
        assert!(matches!(
            described.change,
            CatalogChange::Describe {
                description: None,
                ..
            }
        ));
        assert!(matches!(
            parse_tag(&args(&["retire", "4", tag, tag])),
            Ok(TagCommand::Change(_))
        ));
        assert_eq!(parse_tag(&args(&["list"])), Ok(TagCommand::List));
        assert_eq!(
            parse_tag(&args(&["history"])),
            Ok(TagCommand::History(DEFAULT_HISTORY))
        );
        assert_eq!(
            parse_tag(&args(&["history", "5"])),
            Ok(TagCommand::History(5))
        );
        for (bad, needle) in [
            (&["create", "x", "theme", "Ký ức"][..], "catalog version"),
            (&["create", "0", "theme", "Ký ức"][..], "catalog_version"),
            (&["create", "1", "genre", "Ký ức"][..], "facet"),
            (&["create", "1", "theme", "海"][..], "label"),
            (&["rename", "1", "not-a-tag", "Ký ức"][..], "tag"),
            (&["alias", "1", tag, "   "][..], "label"),
            (&["retire", "1", tag, "x"][..], "successor"),
            (&["retire", "1", tag, tag, "x"][..], "expected"),
            (&["create", "1", "theme"][..], "expected"),
            (&["merge", "1", tag, tag][..], "expected"),
            (&["delete", tag][..], "expected"),
            (&["list", "x"][..], "expected"),
            (&["history", "x"][..], "history limit"),
        ] {
            let error = parse_tag(&args(bad)).expect_err("must be refused");
            assert!(error.contains(needle), "{bad:?}: {error}");
        }
    }
}
