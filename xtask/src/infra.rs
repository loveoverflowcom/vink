//! Opt-in deployment boundary. Plans/checks do no writes or network I/O.
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const HELP: &str = "infra check <inventory.toml>\ninfra render-env <inventory.toml>\ninfra build-images <podman|docker> <linux/amd64|linux/arm64>\ninfra dns-plan <inventory.toml> <snapshot.json>\ninfra dns-check <plan.json> <fresh-snapshot.json>\ninfra dns-rollback <plan.json> <fresh-snapshot.json>\nNo command publishes, changes DNS, migrates or deploys. See infra/test/README.md.";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inventory {
    schema: u32,
    public_origin: String,
    platform: String,
    app_image: String,
    worker_image: String,
    identity_image: String,
    host_port: u16,
    uid: u32,
    socket_dir: String,
    secrets_dir: String,
    dns_target: String,
    dns_ttl: u32,
}

fn read<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    if path.ends_with(".toml") {
        toml::from_str(&text).map_err(|e| format!("invalid TOML: {e}"))
    } else {
        serde_json::from_str(&text).map_err(|e| format!("invalid JSON: {e}"))
    }
}
fn platform(value: &str) -> bool {
    matches!(value, "linux/amd64" | "linux/arm64")
}
fn digest_image(value: &str) -> bool {
    value
        .strip_prefix("ghcr.io/loveoverflowcom/")
        .is_some_and(|tail| {
            let Some((name, digest)) = tail.split_once("@sha256:") else {
                return false;
            };
            !name.is_empty()
                && name
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_/".contains(&b))
                && digest.len() == 64
                && digest
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        })
}
fn safe_path(value: &str) -> bool {
    value.starts_with('/')
        && !value.contains("..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_-.".contains(&b))
}
impl Inventory {
    fn validate(&self, runtime: bool) -> Result<(), String> {
        let origin_host = self.public_origin.strip_prefix("https://");
        if self.schema != 1 || !origin_host.is_some_and(hostname) {
            return Err("schema must be 1; public_origin requires an HTTPS hostname without userinfo/path/query/port".into());
        }
        if !platform(&self.platform) {
            return Err("set verified target platform linux/amd64 or linux/arm64".into());
        }
        if self.uid == 0
            || self.host_port < 1024
            || !safe_path(&self.socket_dir)
            || !safe_path(&self.secrets_dir)
        {
            return Err(
                "non-root UID, unprivileged port and absolute safe socket/secrets paths required"
                    .into(),
            );
        }
        if runtime
            && ![&self.app_image, &self.worker_image, &self.identity_image]
                .iter()
                .all(|v| digest_image(v))
        {
            return Err(
                "all images require actual GHCR repo@sha256 digests; never substitute local IDs"
                    .into(),
            );
        }
        Ok(())
    }
    fn env(&self) -> Result<String, String> {
        self.validate(true)?;
        Ok(format!(
            "VINK_PUBLIC_ORIGIN={}\nVINK_TEST_APP_IMAGE={}\nVINK_TEST_WORKER_IMAGE={}\nVINK_TEST_IDENTITY_IMAGE={}\nVINK_TEST_HOST_PORT={}\nVINK_TEST_UID={}\nVINK_TEST_SOCKET_DIR={}\nVINK_TEST_SECRETS_DIR={}\nVINK_TEST_PLATFORM={}\n",
            self.public_origin,
            self.app_image,
            self.worker_image,
            self.identity_image,
            self.host_port,
            self.uid,
            self.socket_dir,
            self.secrets_dir,
            self.platform
        ))
    }
}

/// Normalized COMPLETE provider snapshot, not a provider API response. Preserve provider IDs in
/// the original export alongside this plan. Unknown record types elsewhere are retained.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    domain: String,
    authoritative_provider: String,
    nameservers: Vec<String>,
    complete: bool,
    records: Vec<Record>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
struct Record {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    value: String,
    ttl: u32,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DnsPlan {
    domain: String,
    nameservers: Vec<String>,
    before: Vec<Record>,
    after: Vec<Record>,
}
fn scope(snapshot: &Snapshot) -> Result<Vec<Record>, String> {
    if snapshot.domain != "vink.forum"
        || snapshot.authoritative_provider != "spaceship"
        || !snapshot.complete
        || snapshot.nameservers.is_empty()
    {
        return Err("need complete vink.forum snapshot and externally verified Spaceship authority/nameservers".into());
    }
    // Require canonical relative names. FQDN variants cannot silently escape the scoped diff.
    if snapshot.records.iter().any(|r| {
        r.name.eq_ignore_ascii_case("test.vink.forum")
            || r.name.eq_ignore_ascii_case("test.vink.forum.")
    }) {
        return Err("normalize all record names to relative provider names before planning".into());
    }
    let mut records: Vec<_> = snapshot
        .records
        .iter()
        .filter(|r| r.name.eq_ignore_ascii_case("test"))
        .cloned()
        .collect();
    for r in &mut records {
        r.name = "test".into();
        r.kind.make_ascii_uppercase();
        if r.kind == "CNAME" {
            r.value = r.value.trim_end_matches('.').to_ascii_lowercase();
        }
        if r.kind == "A" {
            r.value = r
                .value
                .parse::<std::net::Ipv4Addr>()
                .map_err(|_| "invalid A in snapshot")?
                .to_string();
        }
    }
    records.sort();
    if records
        .iter()
        .any(|r| !matches!(r.kind.as_str(), "A" | "CNAME"))
    {
        return Err("test has records outside A/CNAME scope (including IPv6); resolve explicitly, never delete them implicitly".into());
    }
    Ok(records)
}
fn hostname(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && s.contains('.')
        && s.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}
fn desired(target: &str, ttl: u32) -> Result<Record, String> {
    let normalized = target.to_ascii_lowercase();
    let target = normalized.as_str();
    if !(60..=86400).contains(&ttl) {
        return Err("TTL outside conservative 60..86400 range; confirm provider limits".into());
    }
    let (kind, value) = if let Ok(ip) = target.parse::<std::net::Ipv4Addr>() {
        let o = ip.octets();
        if ip.is_private()
            || ip.is_loopback()
            || ip.is_unspecified()
            || ip.is_link_local()
            || ip.is_multicast()
            || ip.is_broadcast()
            || ip.is_documentation()
            || o[0] == 0
            || o[0] >= 240
            || (o[0] == 100 && (64..=127).contains(&o[1]))
            || (o[0] == 198 && (18..=19).contains(&o[1]))
        {
            return Err(
                "DNS requires a verified public IPv4, not a local/reserved/example address".into(),
            );
        }
        ("A", ip.to_string())
    } else if hostname(target)
        && !target.ends_with(".test")
        && !target.ends_with(".example")
        && !target.ends_with(".invalid")
        && !target.ends_with(".localhost")
        && !target.ends_with(".local")
        && target != "test.vink.forum"
        && target != "example.com"
        && !target.ends_with(".example.com")
        && target != "example.net"
        && !target.ends_with(".example.net")
        && target != "example.org"
        && !target.ends_with(".example.org")
        && !target.bytes().all(|b| b.is_ascii_digit() || b == b'.')
    {
        ("CNAME", target.to_ascii_lowercase())
    } else {
        return Err(
            "set verified public IPv4 or target hostname (no scheme/path/port/IPv6/placeholder)"
                .into(),
        );
    };
    Ok(Record {
        name: "test".into(),
        kind: kind.into(),
        value,
        ttl,
    })
}
fn plan(config: &Inventory, snapshot: &Snapshot) -> Result<DnsPlan, String> {
    // DNS planning is independent of platform and registry readiness.
    if config.public_origin != "https://test.vink.forum" {
        return Err("test scope only".into());
    }
    Ok(DnsPlan {
        domain: "vink.forum".into(),
        nameservers: snapshot.nameservers.clone(),
        before: scope(snapshot)?,
        after: vec![desired(&config.dns_target, config.dns_ttl)?],
    })
}
fn check_plan(plan: &DnsPlan, fresh: &Snapshot, rollback: bool) -> Result<DnsPlan, String> {
    if plan.domain != "vink.forum"
        || plan.nameservers != fresh.nameservers
        || plan.after.len() != 1
        || plan
            .before
            .iter()
            .chain(&plan.after)
            .any(|r| r.name != "test" || !matches!(r.kind.as_str(), "A" | "CNAME"))
    {
        return Err("plan scope or authoritative nameservers changed".into());
    }
    let canonical = desired(&plan.after[0].value, plan.after[0].ttl)?;
    if canonical != plan.after[0] {
        return Err("unsafe or noncanonical desired record".into());
    }
    let current = scope(fresh)?;
    let expected = if rollback { &plan.after } else { &plan.before };
    if &current != expected {
        return Err("scoped DNS drift; fresh-read and re-plan, do not force zone update".into());
    }
    Ok(DnsPlan {
        domain: plan.domain.clone(),
        nameservers: plan.nameservers.clone(),
        before: current,
        after: if rollback {
            plan.before.clone()
        } else {
            plan.after.clone()
        },
    })
}
fn print_json(value: &impl Serialize) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|e| e.to_string())?
    );
    Ok(())
}
struct ContextArchive(std::path::PathBuf);
impl Drop for ContextArchive {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn build_images(root: &Path, engine: &str, target: &str) -> Result<(), String> {
    if !matches!(engine, "podman" | "docker") || !platform(target) {
        return Err("explicit podman|docker and linux/amd64|linux/arm64 required".into());
    }
    let output = std::process::Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() || !output.stdout.is_empty() {
        return Err("image build requires clean tracked and untracked source; commit reviewed task paths first".into());
    }
    let output = std::process::Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| e.to_string())?;
    let sha = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    let sha = sha.trim();
    if !output.status.success() || sha.len() != 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("cannot resolve clean source SHA".into());
    }
    let context_dir = root.join("target/infra");
    std::fs::create_dir_all(&context_dir).map_err(|e| e.to_string())?;
    let context = context_dir.join(format!("context-{sha}.tar"));
    crate::process::run(
        std::process::Command::new("git")
            .current_dir(root)
            .args(["archive", "--format=tar", "--output"])
            .arg(&context)
            .arg(sha),
        "archive committed source only",
    )?;
    let _cleanup = ContextArchive(context.clone());
    for image in ["web", "worker", "identity"] {
        let mut cmd = std::process::Command::new(engine);
        cmd.stdin(std::fs::File::open(&context).map_err(|e| e.to_string())?)
            .current_dir(root)
            .args([
                "build",
                "--platform",
                target,
                "--file",
                "infra/test/Containerfile",
                "--target",
                image,
                "--build-arg",
                &format!("SOURCE_REVISION={sha}"),
                "--tag",
                &format!("ghcr.io/loveoverflowcom/vink-{image}:sha-{sha}"),
                "-",
            ]);
        crate::process::run(
            &mut cmd,
            &format!("build {image}; no push or service startup"),
        )?;
    }
    Ok(())
}
pub fn run(root: &Path, args: &[&str]) -> Result<(), String> {
    match args {
        ["--help"] | [] => {
            println!("{HELP}");
            Ok(())
        }
        ["check", path] => {
            let c: Inventory = read(path)?;
            c.validate(true)?;
            println!(
                "inventory valid; target, DNS authority, secrets and runtime still require operator verification"
            );
            Ok(())
        }
        ["render-env", path] => {
            let c: Inventory = read(path)?;
            print!("{}", c.env()?);
            Ok(())
        }
        ["build-images", engine, target] => build_images(root, engine, target),
        ["dns-plan", config, snapshot] => print_json(&plan(&read(config)?, &read(snapshot)?)?),
        ["dns-check", p, fresh] => print_json(&check_plan(&read(p)?, &read(fresh)?, false)?),
        ["dns-rollback", p, fresh] => print_json(&check_plan(&read(p)?, &read(fresh)?, true)?),
        _ => Err(HELP.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> Inventory {
        Inventory {
            schema: 1,
            public_origin: "https://test.vink.forum".into(),
            platform: "linux/arm64".into(),
            app_image: format!("ghcr.io/loveoverflowcom/vink-web@sha256:{}", "a".repeat(64)),
            worker_image: format!(
                "ghcr.io/loveoverflowcom/vink-worker@sha256:{}",
                "b".repeat(64)
            ),
            identity_image: format!(
                "ghcr.io/loveoverflowcom/vink-identity@sha256:{}",
                "c".repeat(64)
            ),
            host_port: 3300,
            uid: 10001,
            socket_dir: "/run/vink-test-identity".into(),
            secrets_dir: "/etc/vink-test".into(),
            dns_target: "8.8.8.8".into(),
            dns_ttl: 300,
        }
    }
    fn snapshot() -> Snapshot {
        Snapshot {
            domain: "vink.forum".into(),
            authoritative_provider: "spaceship".into(),
            nameservers: vec!["verified-ns".into()],
            complete: true,
            records: vec![Record {
                name: "@".into(),
                kind: "MX".into(),
                value: "mail".into(),
                ttl: 300,
            }],
        }
    }
    #[test]
    fn runtime_rejects_unknown_platform_missing_digest_and_injection() {
        let mut c = config();
        assert!(c.validate(true).is_ok());
        c.platform = "".into();
        assert!(c.validate(true).is_err());
        c.platform = "linux/amd64".into();
        c.app_image = "ghcr.io/loveoverflowcom/vink-web:latest".into();
        assert!(c.env().is_err());
        c.app_image = config().app_image;
        c.socket_dir = "/run/a\nBAD=yes".into();
        assert!(c.env().is_err());
    }
    #[test]
    fn dns_is_idempotent_and_scoped_rollback_preserves_other_records() {
        let c = config();
        let mut s = snapshot();
        let p = plan(&c, &s).unwrap();
        assert!(check_plan(&p, &s, false).is_ok());
        s.records.extend(p.after.clone());
        let second = plan(&c, &s).unwrap();
        assert_eq!(second.before, second.after);
        s.records.push(Record {
            name: "www".into(),
            kind: "CNAME".into(),
            value: "changed".into(),
            ttl: 600,
        });
        let rollback = check_plan(&p, &s, true).unwrap();
        assert!(rollback.after.is_empty());
        assert_eq!(rollback.before, p.after);
        s.records.retain(|r| r.name != "test");
        assert_eq!(s.records.len(), 2);
    }
    #[test]
    fn provider_case_and_absolute_cname_normalize_for_idempotency() {
        let mut c = config();
        c.dns_target = "origin.real-host.org".into();
        let mut s = snapshot();
        s.records.push(Record {
            name: "TEST".into(),
            kind: "cname".into(),
            value: "ORIGIN.REAL-HOST.ORG.".into(),
            ttl: 300,
        });
        let p = plan(&c, &s).unwrap();
        assert_eq!(p.before, p.after);
    }
    #[test]
    fn dns_refuses_drift_wrong_zone_incomplete_and_ipv6() {
        let c = config();
        let mut s = snapshot();
        let p = plan(&c, &s).unwrap();
        s.records.extend(p.after.clone());
        assert!(check_plan(&p, &s, false).is_err());
        s.records[1].kind = "AAAA".into();
        assert!(plan(&c, &s).is_err());
        s = snapshot();
        s.records.push(Record {
            name: "TEST.VINK.FORUM.".into(),
            kind: "A".into(),
            value: "8.8.8.8".into(),
            ttl: 300,
        });
        assert!(plan(&c, &s).is_err());
        s = snapshot();
        s.complete = false;
        assert!(plan(&c, &s).is_err());
        s.complete = true;
        s.domain = "cantos.cc".into();
        assert!(plan(&c, &s).is_err());
    }
    #[test]
    fn targets_reject_local_scheme_path_port_and_placeholder() {
        for t in [
            "",
            "127.0.0.1",
            "10.0.0.1",
            "192.0.2.1",
            "100.64.0.1",
            "198.18.0.1",
            "https://vps.test",
            "vps.test/path",
            "vps.test:443",
            "::1",
            "vps.invalid",
            "test.vink.forum",
            "TEST.VINK.FORUM",
            "VPS.INVALID",
            "vps.test",
            "999.999.999.999",
        ] {
            assert!(desired(t, 300).is_err(), "{t}");
        }
        assert_eq!(desired("vps.real-host.org", 300).unwrap().kind, "CNAME");
    }
    #[test]
    fn cname_replaces_a_only_inside_test_and_ttl_is_compared() {
        let mut s = snapshot();
        s.records.push(desired("8.8.8.8", 600).unwrap());
        let mut c = config();
        c.dns_target = "vps.real-host.org".into();
        let p = plan(&c, &s).unwrap();
        assert_ne!(p.before, p.after);
        assert_eq!(p.after[0].kind, "CNAME");
        assert_eq!(p.before.len(), 1);
    }
    #[test]
    fn origin_is_runtime_only_and_reuses_exact_image() {
        let mut c = config();
        let images = (
            c.app_image.clone(),
            c.worker_image.clone(),
            c.identity_image.clone(),
        );
        c.public_origin = "https://another-test.vink.forum".into();
        assert!(c.env().unwrap().contains("https://another-test.vink.forum"));
        assert_eq!(
            images,
            (
                c.app_image.clone(),
                c.worker_image.clone(),
                c.identity_image.clone()
            )
        );
        assert!(plan(&c, &snapshot()).is_err()); // DNS remains explicitly scoped to test.
    }
    #[test]
    fn unknown_fields_rejected_and_no_migrator_or_auth_unlock_rendered() {
        let c = config();
        let env = c.env().unwrap();
        assert!(!env.contains("DATABASE_URL"));
        assert!(!env.contains("AUTH_SIGN_IN"));
        assert!(!env.contains("MIGRATE"));
        let bad = include_str!("../../infra/test/test.example.toml").to_string()
            + "\npassword='secret'\n";
        assert!(toml::from_str::<Inventory>(&bad).is_err());
    }
}
