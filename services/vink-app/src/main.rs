//! `vink-app [serve|migrate|dev-seed|moderator …]` — HTTP/SSR server and explicit database commands.
//!
//! `serve`, `dev-seed` and `moderator` use the runtime credential (`DATABASE_URL`); `migrate` uses
//! only the migration credential (`VINK_MIGRATE_DATABASE_URL`). `moderator grant|revoke|list` is
//! the operator's only way to give or take comment-moderation authority; no HTTP route does it.
//! `principal suspend|reinstate|list` likewise is the only way to suspend a principal (D-05/S-03
//! spike). `tag list|history|create|rename|describe|alias|retire` is the only way to change the
//! curated tag catalog (TAG-PR-03, #97 CHOICE-02 A); each change names the catalog version the
//! operator last read and is audited in the same transaction. `outbox status|dead|requeue` is the
//! operator's view of the discussion outbox (DISC-04): queue health, dead letters, and the only way
//! to requeue one (audited, dead letters only; never through HTTP). Exit codes: 0 success, 1 runtime
//! failure, 2 invalid configuration or usage, or a catalog change that was refused or conflicted
//! (nothing written).

use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::Arc;

use vink_app::application::Vink;
use vink_app::application::catalog::{
    CatalogError, CatalogRejection, CatalogRequest, ChangeCatalog, CuratorAuthority, facet_code,
};
use vink_app::application::post_review::{
    RequestReview, ReviewAuthority, ReviewResult, ReviewWrite,
};
use vink_app::config::{Config, MigrateConfig, Profile};
use vink_app::domain::discussion::admission::PostReviewFinding;
use vink_app::domain::discussion::lifecycle::ModerationVersion;
use vink_app::domain::discussion::moderation::GrantNote;
use vink_app::domain::ids::CommentId;
use vink_app::domain::ids::PrincipalId;
use vink_app::domain::principal::SuspensionReason;
use vink_app::domain::taxonomy::{
    CatalogCommand, CatalogTag, SlugKind, TagDescription, TagLabel, TagSlug, TagStatus,
};
use vink_app::http::{AppState, router};
use vink_app::persistence::{self, PgStore, lazy_pool};
use vink_app::{dev_seed, identity, runtime};

const USAGE: &str = "usage: vink-app [serve|migrate|dev-seed]\n       vink-app admin add <principal-uuid> [<reason>]\n       vink-app admin remove <principal-uuid> [<reason>]\n       vink-app admin list\n       vink-app moderator grant <principal-uuid> <note>\n       vink-app moderator revoke <principal-uuid>\n       vink-app moderator list\n       vink-app principal suspend <principal-uuid> <reason>\n       vink-app principal reinstate <principal-uuid>\n       vink-app principal list\n       vink-app tag list\n       vink-app tag history [<limit>]\n       vink-app tag create <catalog-version> <form|theme> <label>\n       vink-app tag rename <catalog-version> <tag-uuid> <label>\n       vink-app tag describe <catalog-version> <tag-uuid> [<description>]\n       vink-app tag alias <catalog-version> <tag-uuid> <label>\n       vink-app tag retire <catalog-version> <tag-uuid> [<successor-uuid>]\n       vink-app outbox status\n       vink-app outbox dead [<limit>]\n       vink-app outbox requeue <event-uuid> <consumer> <note>\n       vink-app post-review request|result|status|pending\n       vink-app maintenance prune";

#[derive(Debug, PartialEq, Eq)]
enum AdminCommand {
    Add(PrincipalId, Option<String>),
    Remove(PrincipalId, Option<String>),
    List,
}

fn parse_admin(args: &[String]) -> Result<AdminCommand, String> {
    let principal = |raw: Option<&String>| {
        raw.and_then(|raw| PrincipalId::parse(raw))
            .ok_or_else(|| "expected a principal UUID".to_string())
    };
    match args.first().map(String::as_str) {
        Some("add") if args.len() >= 2 => {
            let reason = if args.len() > 2 {
                Some(args[2..].join(" "))
            } else {
                None
            };
            Ok(AdminCommand::Add(principal(args.get(1))?, reason))
        }
        Some("remove") if args.len() >= 2 => {
            let reason = if args.len() > 2 {
                Some(args[2..].join(" "))
            } else {
                None
            };
            Ok(AdminCommand::Remove(principal(args.get(1))?, reason))
        }
        Some("list") if args.len() == 1 => Ok(AdminCommand::List),
        _ => Err(
            "expected `add <principal> [<reason>]`, `remove <principal> [<reason>]` or `list`"
                .into(),
        ),
    }
}

/// Operator command on the discussion outbox, parsed before any configuration is read.
#[derive(Debug, PartialEq, Eq)]
enum OutboxCommand {
    Status,
    Dead(i64),
    Requeue {
        event: uuid::Uuid,
        consumer: String,
        note: String,
    },
}

#[derive(Debug, PartialEq, Eq)]
enum PostReviewCommand {
    Request(RequestReview),
    Result(ReviewResult),
    Status,
    Pending,
}

fn parse_post_review(args: &[String]) -> Result<PostReviewCommand, String> {
    let id = |index: usize| {
        args.get(index)
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
            .ok_or_else(|| "expected UUID".to_string())
    };
    let note = || {
        GrantNote::parse(&args.get(4..).unwrap_or_default().join(" ")).ok_or_else(|| {
            "note must name operator and reason (one line, 1..200 characters)".to_string()
        })
    };
    match args.first().map(String::as_str) {
        Some("status") if args.len() == 1 => Ok(PostReviewCommand::Status),
        Some("pending") if args.len() == 1 => Ok(PostReviewCommand::Pending),
        Some("request") if args.len() >= 5 => Ok(PostReviewCommand::Request(RequestReview {
            comment: CommentId::from_uuid(id(1)?),
            expected: args[2].parse().ok().and_then(ModerationVersion::new).ok_or("expected moderation version >=1")?,
            key: id(3)?, note: note()?,
        })),
        Some("result") if args.len() >= 5 => Ok(PostReviewCommand::Result(ReviewResult {
            ticket: id(1)?, key: id(2)?,
            finding: match args[3].as_str() { "clear" => PostReviewFinding::Clear, "flagged" => PostReviewFinding::Flagged, "unresolved" => PostReviewFinding::Unresolved, _ => return Err("expected clear|flagged|unresolved".into()) },
            note: note()?,
        })),
        _ => Err("post-review request <comment> <version> <request-key-uuid> <operator-and-reason> | result <ticket> <result-key-uuid> <clear|flagged|unresolved> <operator-and-reason> | status | pending".into()),
    }
}

fn parse_outbox(args: &[String]) -> Result<OutboxCommand, String> {
    match args.first().map(String::as_str) {
        Some("status") if args.len() == 1 => Ok(OutboxCommand::Status),
        Some("dead") if args.len() <= 2 => {
            let limit = match args.get(1) {
                None => DEFAULT_HISTORY,
                Some(raw) => raw
                    .parse::<i64>()
                    .ok()
                    .filter(|n| (1..=1_000).contains(n))
                    .ok_or_else(|| "expected a limit between 1 and 1000".to_string())?,
            };
            Ok(OutboxCommand::Dead(limit))
        }
        Some("requeue") if args.len() >= 4 => {
            let event = uuid::Uuid::parse_str(&args[1])
                .map_err(|_| "expected an event UUID".to_string())?;
            let note = args[3..].join(" ");
            if note.trim().is_empty() || note.chars().count() > 500 {
                return Err("expected a note of 1 to 500 characters".into());
            }
            Ok(OutboxCommand::Requeue {
                event,
                consumer: args[2].clone(),
                note,
            })
        }
        _ => {
            Err("expected `status`, `dead [<limit>]` or `requeue <event> <consumer> <note>`".into())
        }
    }
}

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
        "serve"
            | "migrate"
            | "dev-seed"
            | "admin"
            | "moderator"
            | "principal"
            | "tag"
            | "outbox"
            | "post-review"
            | "maintenance"
    ) {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let admin_command = if command == "admin" {
        let args: Vec<String> = std::env::args().skip(2).collect();
        match parse_admin(&args) {
            Ok(parsed) => Some(parsed),
            Err(error) => {
                eprintln!("vink-app admin: {error}\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    } else {
        None
    };
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
    let outbox_command = if command == "outbox" {
        let args: Vec<String> = std::env::args().skip(2).collect();
        match parse_outbox(&args) {
            Ok(parsed) => Some(parsed),
            Err(error) => {
                eprintln!("vink-app outbox: {error}\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    } else {
        None
    };
    let post_review_command = if command == "post-review" {
        let args: Vec<String> = std::env::args().skip(2).collect();
        match parse_post_review(&args) {
            Ok(parsed) => Some(parsed),
            Err(error) => {
                eprintln!("vink-app: {error}");
                return ExitCode::from(2);
            }
        }
    } else {
        None
    };
    if command == "maintenance" {
        let args: Vec<String> = std::env::args().skip(2).collect();
        if args != ["prune"] {
            eprintln!("vink-app maintenance: expected `maintenance prune`\n{USAGE}");
            return ExitCode::from(2);
        }
    }
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
            Loaded::App(config) if command == "maintenance" => maintenance(&config).await,
            Loaded::App(config) if let Some(operation) = admin_command => {
                admin(&config, operation).await
            }
            Loaded::App(config) if let Some(operation) = moderator_command => {
                moderator(&config, operation).await
            }
            Loaded::App(config) if let Some(operation) = principal_command => {
                principal(&config, operation).await
            }
            Loaded::App(config) if let Some(operation) = tag_command => {
                tag(&config, operation).await
            }
            Loaded::App(config) if let Some(operation) = outbox_command => {
                outbox(&config, operation).await
            }
            Loaded::App(config) if let Some(operation) = post_review_command => {
                post_review(&config, operation).await
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

async fn admin(config: &Config, command: AdminCommand) -> Result<(), Failure> {
    let pool = connect(&config.database_url, "vink-app admin").await?;
    let operator = std::env::var("USER").unwrap_or_else(|_| "operator".to_string());
    let mut conn = pool
        .acquire()
        .await
        .map_err(|e| Failure::Runtime(e.to_string()))?;
    match command {
        AdminCommand::Add(principal, reason) => {
            persistence::moderation::add_admin_seat(
                &mut conn,
                &operator,
                principal,
                reason.as_deref(),
            )
            .await
            .map_err(|e| Failure::Runtime(format!("admin add failed: {e}")))?;
            println!("added admin seat for {principal}");
        }
        AdminCommand::Remove(principal, reason) => {
            persistence::moderation::remove_admin_seat(
                &mut conn,
                &operator,
                principal,
                reason.as_deref(),
            )
            .await
            .map_err(|e| Failure::Runtime(format!("admin remove failed: {e}")))?;
            println!("removed admin seat for {principal}");
        }
        AdminCommand::List => {
            let seats = persistence::moderation::list_admin_seats(&mut conn)
                .await
                .map_err(|e| Failure::Runtime(format!("admin list failed: {e}")))?;
            for seat in seats {
                println!(
                    "{}\t{}\t{}",
                    seat.principal_id, seat.display_name, seat.created_at
                );
            }
        }
    }
    pool.close().await;
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

/// `vink-app maintenance prune`: deletes sign-in rate-limit windows that ended over a day ago and
/// discussion events whose deliveries all finished over 30 days ago (as `vink-worker` does hourly).
async fn maintenance(config: &Config) -> Result<(), Failure> {
    let pool = connect(&config.database_url, "vink-app maintenance").await?;
    let pruned = Vink::new(PgStore::new(pool.clone()))
        .prune_rate_limits()
        .await
        .map_err(|_| Failure::Runtime("prune failed".into()));
    let events = persistence::outbox::prune(&pool, 30, 1_000)
        .await
        .map_err(|_| Failure::Runtime("outbox prune failed".into()));
    pool.close().await;
    println!("pruned {} expired rate-limit window(s)", pruned?);
    println!("pruned {} finished outbox event(s)", events?);
    Ok(())
}

/// `vink-app outbox …`: counts, codes and identifiers only; no comment content is read.
async fn outbox(config: &Config, command: OutboxCommand) -> Result<(), Failure> {
    use persistence::outbox::{self as queue, RequeueOutcome};
    use vink_app::domain::discussion::outbox::Consumer;
    let pool = connect(&config.database_url, "vink-app outbox").await?;
    let failed = |error: vink_app::application::ports::StoreError| {
        Failure::Runtime(format!("outbox command failed: {error}"))
    };
    let result = match command {
        OutboxCommand::Status => {
            let consumers: Vec<&str> = Consumer::ALL.iter().map(|c| c.code()).collect();
            let backlog = queue::backlog(&pool, &consumers).await.map_err(failed)?;
            let age =
                |seconds: Option<i64>| seconds.map_or_else(|| "-".to_string(), |s| format!("{s}s"));
            println!("eligible\t{}", backlog.eligible);
            println!("scheduled_retry\t{}", backlog.scheduled);
            println!("leased\t{}", backlog.leased);
            println!("dead\t{}", backlog.dead);
            println!("unhandled\t{}", backlog.unhandled);
            println!("oldest_eligible\t{}", age(backlog.oldest_eligible_seconds));
            println!("oldest_open\t{}", age(backlog.oldest_open_seconds));
            Ok(())
        }
        OutboxCommand::Dead(limit) => {
            for dead in queue::dead_letters(&pool, limit).await.map_err(failed)? {
                println!(
                    "{}\t{}\t{}\tv{}\t{}\tattempts={}\t{}",
                    dead.event_id,
                    dead.consumer,
                    dead.event_type,
                    dead.schema_version,
                    dead.outcome,
                    dead.attempts,
                    dead.finished_at
                );
            }
            Ok(())
        }
        OutboxCommand::Requeue {
            event,
            consumer,
            note,
        } => match queue::requeue(&pool, event, &consumer, &note)
            .await
            .map_err(failed)?
        {
            RequeueOutcome::Requeued { previous_attempts } => {
                println!(
                    "requeued {event} {consumer} (previous attempts {previous_attempts}); audited"
                );
                Ok(())
            }
            RequeueOutcome::Refused(state) => Err(Failure::Usage(format!(
                "refused: the delivery is {}; only a dead letter can be requeued and nothing was written",
                state.code()
            ))),
            RequeueOutcome::NotFound => Err(Failure::Usage(
                "no such delivery; nothing was written".into(),
            )),
        },
    };
    pool.close().await;
    result
}

async fn post_review(config: &Config, command: PostReviewCommand) -> Result<(), Failure> {
    let enabled = match std::env::var("VINK_POST_REVIEW_REQUESTS").ok().as_deref() {
        None | Some("disabled") => false,
        Some("enabled") => true,
        _ => {
            return Err(Failure::Usage(
                "VINK_POST_REVIEW_REQUESTS: expected enabled|disabled".into(),
            ));
        }
    };
    let pool = connect(&config.database_url, "vink-app post-review").await?;
    let vink = Vink::new(PgStore::new(pool.clone()));
    let failed = |_error| {
        Failure::Runtime("post-review database operation failed; obligation preserved".into())
    };
    let authority = ReviewAuthority::operator();
    let outcome = match command {
        PostReviewCommand::Request(request) => Some(
            vink.request_review(authority, enabled, &request)
                .await
                .map_err(failed)?,
        ),
        PostReviewCommand::Result(result) => Some(
            vink.record_review_result(authority, &result)
                .await
                .map_err(failed)?,
        ),
        PostReviewCommand::Status => {
            for (state, count, age) in persistence::post_review::status(&pool)
                .await
                .map_err(failed)?
            {
                println!("{state}\t{count}\t{}", dash(age));
            }
            None
        }
        PostReviewCommand::Pending => {
            for pending in persistence::post_review::pending(&pool, 100)
                .await
                .map_err(failed)?
            {
                println!(
                    "{}\t{}\t{}\t{}\t{}",
                    pending.ticket,
                    pending.comment,
                    pending.version,
                    pending.deadline,
                    pending.overdue
                );
            }
            for (event, reason) in persistence::post_review::quarantines(&pool, 100)
                .await
                .map_err(failed)?
            {
                println!("quarantined\t{event}\t{reason}");
            }
            None
        }
    };
    pool.close().await;
    match outcome {
        Some(ReviewWrite::Accepted(id)) => {
            println!("accepted\t{id}");
            Ok(())
        }
        Some(ReviewWrite::Replayed(id)) => {
            println!("replayed\t{id}");
            Ok(())
        }
        Some(other) => Err(Failure::Usage(format!(
            "post-review refused: {other:?}; nothing written"
        ))),
        None => Ok(()),
    }
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
    .with_sign_in_enabled(config.sign_in_enabled)
    .with_trusted_proxies(config.trusted_proxies.clone())
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
    if config.identity.is_some() && !config.sign_in_enabled {
        tracing::warn!(
            "sign-in is disabled (VINK_AUTH_SIGN_IN): no new session can start; existing sessions \
             are still verified and can log out"
        );
    }
    tracing::info!(
        bind = %config.bind,
        origin = %config.public_origin,
        profile = ?config.profile,
        sign_in = if config.sign_in_enabled { "enabled" } else { "disabled" },
        trusted_proxies = !config.trusted_proxies.is_empty(),
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
    fn outbox_arguments_are_validated_before_any_configuration() {
        let event = "0190f0c8-0000-7000-8000-000000000001";
        assert_eq!(parse_outbox(&args(&["status"])), Ok(OutboxCommand::Status));
        assert_eq!(parse_outbox(&args(&["dead"])), Ok(OutboxCommand::Dead(50)));
        assert_eq!(
            parse_outbox(&args(&["dead", "7"])),
            Ok(OutboxCommand::Dead(7))
        );
        assert_eq!(
            parse_outbox(&args(&[
                "requeue",
                event,
                "admission_shadow.v1",
                "Đã",
                "sửa"
            ])),
            Ok(OutboxCommand::Requeue {
                event: uuid::Uuid::parse_str(event).unwrap(),
                consumer: "admission_shadow.v1".into(),
                note: "Đã sửa".into(),
            })
        );
        let long = "x".repeat(501);
        for bad in [
            &["status", "x"][..],
            &["dead", "0"][..],
            &["dead", "1001"][..],
            &["requeue", event, "admission_shadow.v1"][..],
            &["requeue", "not-a-uuid", "admission_shadow.v1", "note"][..],
            &["requeue", event, "admission_shadow.v1", " "][..],
            &["requeue", event, "admission_shadow.v1", &long][..],
            &["purge"][..],
        ] {
            assert!(parse_outbox(&args(bad)).is_err(), "{bad:?}");
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
