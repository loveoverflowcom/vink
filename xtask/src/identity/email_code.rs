//! D-05/S-03 spike, end to end with the real `vink-identity` and `vink-app` binaries on the
//! provisioned scratch databases: sign-in through the App's API (codes read from Identity's local
//! outbox like a mailbox), principal provisioning, account isolation, forged headers, logout,
//! verified email change, App suspension, Identity disable and an Identity outage.
//!
//! This is real process/socket/database evidence for the spike. It is **not** production
//! authentication: the outbox is a local directory, there is no mail transport, no edge rate limit
//! and no browser runs here (cookies are sent by this client exactly as the App set them).

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::Value;

use super::{Run, expect, request, sql};
use crate::web;

/// One HTTP/1.1 exchange over loopback TCP: status, `Set-Cookie` pairs and body.
struct Reply {
    status: u16,
    cookies: Vec<(String, String)>,
    body: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or(Value::Null)
    }

    fn cookie(&self, name: &str) -> Option<&str> {
        self.cookies
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn expect(&self, what: &str, status: u16, needle: &str) -> Result<(), String> {
        if self.status == status && self.body.contains(needle) {
            Ok(())
        } else {
            Err(format!(
                "{what}: expected {status} with `{needle}`, got {}: {}",
                self.status, self.body
            ))
        }
    }
}

fn exchange(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> Result<Reply, String> {
    let mut stream =
        std::net::TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    let body = body.unwrap_or("");
    let mut head = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    if !body.is_empty() {
        head.push_str("Content-Type: application/json\r\n");
    }
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(body.as_bytes()))
        .map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).map_err(|e| e.to_string())?;
    let raw = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((&raw, ""));
    let status = head
        .get(9..12)
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| format!("not an HTTP response: {raw:?}"))?;
    let cookies = head
        .lines()
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("set-cookie").then(|| {
                let pair = value.trim().split(';').next().unwrap_or_default();
                let (n, v) = pair.split_once('=').unwrap_or((pair, ""));
                (n.to_string(), v.to_string())
            })
        })
        .collect();
    // Chunked bodies are small JSON here; strip chunk-size lines if present.
    let body = if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        body.lines()
            .filter(|line| usize::from_str_radix(line.trim(), 16).is_err())
            .collect::<Vec<_>>()
            .join("")
    } else {
        body.to_string()
    };
    Ok(Reply {
        status,
        cookies,
        body,
    })
}

/// A signed-in client: its session cookie as the App set it.
struct Client {
    session: String,
}

impl Client {
    fn cookie(&self) -> String {
        format!("__Host-vink_session={}", self.session)
    }
}

struct Flow<'a> {
    run: &'a Run<'a>,
    port: u16,
    origin: String,
    outbox: &'a Path,
    /// Every credential, handle and code seen; none may appear in a log or public page.
    secrets: Vec<String>,
}

impl Flow<'_> {
    fn call(
        &self,
        method: &str,
        path: &str,
        cookie: Option<&str>,
        body: Option<&str>,
    ) -> Result<Reply, String> {
        let mut headers = vec![("Origin", self.origin.as_str())];
        if let Some(cookie) = cookie {
            headers.push(("Cookie", cookie));
        }
        exchange(self.port, method, path, &headers, body)
    }

    /// The code mailed for `handle`, read from the outbox file named by the challenge id.
    fn code(&mut self, handle: &str) -> Result<String, String> {
        let id = handle
            .strip_prefix("vink_c1.")
            .and_then(|rest| rest.split('.').next())
            .ok_or("challenge handle has an unexpected shape")?;
        let path = self.outbox.join(format!("{id}.txt"));
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("outbox {}: {e}", path.display()))?;
        let code = text
            .lines()
            .find_map(|line| line.strip_prefix("X-Vink-Code: "))
            .ok_or("outbox message without a code")?
            .to_string();
        self.secrets.push(code.clone());
        Ok(code)
    }

    /// Asks for a code; returns the challenge cookie value.
    fn challenge(&mut self, email: &str) -> Result<String, String> {
        let reply = self.call(
            "POST",
            "/api/v1/auth/sign-in/challenges",
            None,
            Some(&format!(r#"{{"email":"{email}"}}"#)),
        )?;
        reply.expect("sign-in challenge", 202, "expires_at")?;
        let handle = reply
            .cookie("__Host-vink_sign_in")
            .ok_or("no challenge cookie")?
            .to_string();
        if reply.body.contains(&handle) {
            return Err("the challenge handle reached the response body".into());
        }
        self.secrets.push(handle.clone());
        Ok(handle)
    }

    /// Full sign-in; returns the reply of the completion and, when issued, the session.
    fn sign_in(&mut self, email: &str) -> Result<(Reply, Option<Client>), String> {
        let handle = self.challenge(email)?;
        let code = self.code(&handle)?;
        let reply = self.call(
            "POST",
            "/api/v1/auth/sign-in/complete",
            Some(&format!("__Host-vink_sign_in={handle}")),
            Some(&format!(r#"{{"code":"{} {}"}}"#, &code[..3], &code[3..])),
        )?;
        let session = reply
            .cookie("__Host-vink_session")
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string());
        if let Some(session) = &session {
            if reply.body.contains(session.as_str()) {
                return Err("the session credential reached the response body".into());
            }
            self.secrets.push(session.clone());
        }
        // Replaying the consumed challenge never yields another session.
        let replay = self.call(
            "POST",
            "/api/v1/auth/sign-in/complete",
            Some(&format!("__Host-vink_sign_in={handle}")),
            Some(&format!(r#"{{"code":"{code}"}}"#)),
        )?;
        if replay
            .cookie("__Host-vink_session")
            .is_some_and(|v| !v.is_empty())
        {
            return Err("a consumed challenge issued another session".into());
        }
        Ok((reply, session.map(|session| Client { session })))
    }

    fn drafts(&self, client: &Client) -> Result<Reply, String> {
        self.call("GET", "/api/v1/drafts", Some(&client.cookie()), None)
    }

    /// Identity's own view of a session, asked directly over the socket as the App would.
    fn subject_of(&self, client: &Client) -> Result<String, String> {
        let (status, body) = request(
            &self.run.socket,
            "POST",
            "/identity/v1/verify-credential",
            &[("Content-Type", "application/json")],
            format!(r#"{{"credential":"{}"}}"#, client.session).as_bytes(),
        )?;
        let value: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
        match (status, value["subject"]["id"].as_str()) {
            (200, Some(id)) => Ok(id.to_string()),
            _ => Err(format!("verify-credential: {status} {body}")),
        }
    }

    fn identity_rpc(&self, path: &str, body: &str) -> Result<(u16, String), String> {
        request(
            &self.run.socket,
            "POST",
            path,
            &[("Content-Type", "application/json")],
            body.as_bytes(),
        )
    }
}

fn operator(
    run: &Run<'_>,
    binary: &str,
    env: &[(&str, &str)],
    args: &[&str],
) -> Result<String, String> {
    let mut command = Command::new(web::binary(run.scratch.root, binary));
    command
        .args(args)
        .current_dir(run.scratch.root)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("VINK_LOG_FORMAT", "text")
        .stdin(Stdio::null());
    for (key, value) in env {
        command.env(key, value);
    }
    let output = command.output().map_err(|e| format!("{binary}: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if output.status.success() {
        Ok(stdout)
    } else {
        Err(format!(
            "{binary} {args:?} exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

impl Run<'_> {
    pub(super) fn email_code_end_to_end(&self) -> Result<(), String> {
        let uid = self.uid()?;
        let s = self.scratch;
        let outbox = self.dir.join("outbox");
        let _ = std::fs::remove_dir_all(&outbox);
        std::fs::create_dir_all(&outbox).map_err(|e| e.to_string())?;
        std::fs::set_permissions(&outbox, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        let authority: [(&str, &OsStr); 2] = [
            ("VINK_IDENTITY_AUTHORITY", OsStr::new("email-code")),
            ("VINK_IDENTITY_LOCAL_OUTBOX", outbox.as_os_str()),
        ];
        let mut identity =
            Some(self.start_with("email-code", &s.identity_runtime, Some(uid), &authority)?);
        let (mut app, port) = self.start_app("email-code-app", &s.app_runtime, uid)?;
        let mut flow = Flow {
            run: self,
            port,
            origin: format!("http://127.0.0.1:{port}"),
            outbox: &outbox,
            secrets: Vec::new(),
        };
        let checked = (|| {
            expect(
                "identity readiness with the email-code authority",
                request(&self.socket, "GET", "/health/ready", &[], b""),
                200,
                r#""mail_transport":"local_outbox""#,
            )?;
            expect(
                "production authentication stays not implemented",
                request(&self.socket, "GET", "/health/ready", &[], b""),
                200,
                r#""production_authentication":"not_implemented""#,
            )?;

            // 1. First sign-in: authenticated subject, no principal, no authority yet.
            let (reply, author) = flow.sign_in("Tac.Gia@Vi-Du.vn")?;
            reply.expect("first sign-in", 200, r#""state":"needs_principal""#)?;
            let author = author.ok_or("no session cookie after sign-in")?;
            flow.drafts(&author)?.expect(
                "unbound subject cannot use drafts",
                401,
                "unauthenticated",
            )?;
            let provisioned = flow.call(
                "POST",
                "/api/v1/auth/principal",
                Some(&author.cookie()),
                Some(r#"{"pen_name":"Tác Giả Thử"}"#),
            )?;
            provisioned.expect("provision principal", 201, r#""created":true"#)?;
            let principal = provisioned.json()["principal_id"]
                .as_str()
                .ok_or("no principal id")?
                .to_string();
            flow.call(
                "POST",
                "/api/v1/auth/principal",
                Some(&author.cookie()),
                Some(r#"{"pen_name":"Tên Khác"}"#),
            )?
            .expect("provisioning is idempotent", 200, &principal)?;
            let created = flow.call("POST", "/api/v1/drafts", Some(&author.cookie()), None)?;
            created.expect("bound principal creates a draft", 201, "work_id")?;
            let work = created.json()["work_id"]
                .as_str()
                .ok_or("no work id")?
                .to_string();

            // 2. Another account is isolated; forged headers name nobody.
            let (reply, reader) = flow.sign_in("nguoi.doc@vi-du.vn")?;
            reply.expect("second account", 200, "needs_principal")?;
            let reader = reader.ok_or("no session for the second account")?;
            flow.call(
                "POST",
                "/api/v1/auth/principal",
                Some(&reader.cookie()),
                Some(r#"{"pen_name":"Người Đọc"}"#),
            )?
            .expect("second principal", 201, "principal_id")?;
            flow.call(
                "GET",
                &format!("/api/v1/drafts/{work}"),
                Some(&reader.cookie()),
                None,
            )?
            .expect("another account cannot read the draft", 404, "not_found")?;
            let forged = exchange(
                port,
                "GET",
                "/api/v1/drafts",
                &[
                    ("Cookie", reader.cookie().as_str()),
                    ("x-vink-principal", principal.as_str()),
                    ("x-user-id", principal.as_str()),
                ],
                None,
            )?;
            forged.expect(
                "forged principal headers change nothing",
                200,
                r#""drafts":[]"#,
            )?;
            let bearer = format!("Bearer {}", author.session);
            exchange(
                port,
                "GET",
                "/api/v1/drafts",
                &[("Authorization", bearer.as_str())],
                None,
            )?
            .expect("the same session as a bearer credential", 200, &work)?;

            // 3. Same mailbox, other spelling: same principal; logout is per session.
            let (reply, second_device) = flow.sign_in("  tac.gia@VI-DU.vn ")?;
            reply.expect("same mailbox signs in", 200, &principal)?;
            let second_device = second_device.ok_or("no second session")?;
            let logout = flow.call(
                "DELETE",
                "/api/v1/auth/session",
                Some(&author.cookie()),
                None,
            )?;
            logout.expect("logout", 204, "")?;
            if logout.cookie("__Host-vink_session") != Some("") {
                return Err("logout did not clear the session cookie".into());
            }
            flow.drafts(&author)?.expect(
                "a logged-out session is refused",
                401,
                "unauthenticated",
            )?;
            flow.call("POST", "/api/v1/drafts", Some(&author.cookie()), None)?
                .expect("a logged-out session cannot write", 401, "unauthenticated")?;
            flow.drafts(&second_device)?
                .expect("the other device stays signed in", 200, &work)?;

            // 4. Verified email change (Identity contract, as the App would call it): same
            // subject, same principal; the freed address proves a new, unlinked account.
            let subject = flow.subject_of(&second_device)?;
            let (status, body) = flow.identity_rpc(
                "/identity/v1/email-change/challenges",
                &format!(
                    r#"{{"credential":"{}","email":"tac.gia.moi@vi-du.vn"}}"#,
                    second_device.session
                ),
            )?;
            let handle =
                serde_json::from_str::<Value>(&body).map_err(|e| e.to_string())?["challenge"]
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| format!("email change: {status} {body}"))?;
            flow.secrets.push(handle.clone());
            let code = flow.code(&handle)?;
            expect(
                "email change completes",
                flow.identity_rpc(
                    "/identity/v1/email-change/complete",
                    &format!(
                        r#"{{"credential":"{}","challenge":"{handle}","code":"{code}"}}"#,
                        second_device.session
                    ),
                ),
                200,
                r#""outcome":"changed""#,
            )?;
            if flow.subject_of(&second_device)? != subject {
                return Err("the subject changed with the address".into());
            }
            flow.drafts(&second_device)?
                .expect("principal kept after email change", 200, &work)?;
            let (reply, recycled) = flow.sign_in("tac.gia@vi-du.vn")?;
            reply.expect("freed address is a new subject", 200, "needs_principal")?;
            let recycled = recycled.ok_or("no session for the recycled address")?;
            if flow.subject_of(&recycled)? == subject {
                return Err("a freed address inherited the old subject".into());
            }
            flow.drafts(&recycled)?.expect(
                "recycled address has no principal",
                401,
                "unauthenticated",
            )?;
            // That address has now asked for three codes in the window: the fourth is refused.
            flow.call(
                "POST",
                "/api/v1/auth/sign-in/challenges",
                None,
                Some(r#"{"email":"tac.gia@vi-du.vn"}"#),
            )?
            .expect("per-address throttle", 429, "rate_limited")?;

            // 5. Vink suspension: closes the principal whatever Identity says; lifting restores.
            let origin = flow.origin.clone();
            let app_db = [
                ("DATABASE_URL", s.app_runtime.as_str()),
                ("VINK_PUBLIC_ORIGIN", origin.as_str()),
            ];
            let out = operator(
                self,
                "vink-app",
                &app_db,
                &["principal", "suspend", &principal, "Kiểm", "thử", "S-03"],
            )?;
            if !out.contains("suspended") {
                return Err(format!("suspend: {out}"));
            }
            flow.drafts(&second_device)?.expect(
                "suspended principal is refused",
                401,
                "unauthenticated",
            )?;
            flow.call(
                "GET",
                "/api/v1/auth/session",
                Some(&second_device.cookie()),
                None,
            )?
            .expect(
                "session reports the suspension",
                200,
                r#""state":"suspended""#,
            )?;
            let (reply, none) = flow.sign_in("tac.gia.moi@vi-du.vn")?;
            reply.expect(
                "suspended principal gets no session",
                403,
                "principal_suspended",
            )?;
            if none.is_some() {
                return Err("a suspended principal received a session cookie".into());
            }
            operator(
                self,
                "vink-app",
                &app_db,
                &["principal", "reinstate", &principal],
            )?;
            flow.drafts(&second_device)?
                .expect("reinstated principal acts again", 200, &work)?;
            let (reply, _) = flow.sign_in("tac.gia.moi@vi-du.vn")?;
            reply.expect(
                "the new address reaches the same principal",
                200,
                &principal,
            )?;
            // Test harness only: move earlier challenges out of the throttle window.
            sql(
                &s.admin_identity,
                "UPDATE email_challenges SET created_at = created_at - interval '1 hour', \
                 expires_at = expires_at - interval '1 hour'",
            )?;

            // 6. Identity disable: every session of the account stops, sign-in is refused.
            let identity_db = [("VINK_IDENTITY_DATABASE_URL", s.identity_runtime.as_str())];
            let out = operator(
                self,
                "vink-identity",
                &identity_db,
                &["account", "disable", &subject],
            )?;
            if !out.contains("session(s) revoked") {
                return Err(format!("disable: {out}"));
            }
            flow.drafts(&second_device)?.expect(
                "disabled account's session is refused",
                401,
                "unauthenticated",
            )?;
            let (reply, none) = flow.sign_in("tac.gia.moi@vi-du.vn")?;
            reply.expect("disabled account cannot sign in", 403, "account_disabled")?;
            if none.is_some() {
                return Err("a disabled account received a session cookie".into());
            }

            // 7. Identity outage: protected requests fail closed, public reading stays up.
            if let Some(server) = identity.take() {
                self.stop(server)?;
            }
            flow.drafts(&reader)?.expect(
                "Identity down: presented session is undecidable",
                503,
                "dependency_unavailable",
            )?;
            flow.call(
                "POST",
                "/api/v1/auth/sign-in/challenges",
                None,
                Some(r#"{"email":"ai.do@vi-du.vn"}"#),
            )?
            .expect(
                "Identity down: no code is sent",
                503,
                "dependency_unavailable",
            )?;
            exchange(port, "GET", "/api/v1/works", &[], None)?.expect(
                "public reading during the outage",
                200,
                "works",
            )?;
            self.public_reading(port)?;
            identity = Some(self.start_with(
                "email-code-restarted",
                &s.identity_runtime,
                Some(uid),
                &authority,
            )?);
            flow.drafts(&reader)?
                .expect("sessions survive an Identity restart", 200, "drafts")?;
            for path in ["/", "/tac-pham/33333333-3333-4333-8333-333333333333"] {
                let page = exchange(port, "GET", path, &[], None)?;
                if flow
                    .secrets
                    .iter()
                    .any(|secret| page.body.contains(secret.as_str()))
                {
                    return Err(format!("public HTML at {path} contains a secret"));
                }
            }
            Ok::<_, String>(())
        })();
        let _ = app.terminate(Duration::from_secs(15));
        if let Some(server) = identity {
            self.stop(server)?;
        }
        checked?;
        // No code, handle or credential in any log of this step.
        for name in ["email-code", "email-code-restarted", "email-code-app"] {
            let text = std::fs::read_to_string(self.log(name)).unwrap_or_default();
            if let Some(secret) = flow
                .secrets
                .iter()
                .find(|secret| text.contains(secret.as_str()))
            {
                let shape = &secret[..secret.len().min(8)];
                return Err(format!("{name}.log contains a secret ({shape}…)"));
            }
        }
        let _ = std::fs::remove_dir_all(&outbox);
        println!(
            "==> S-03 spike: sign-in → needs_principal → provision; isolation; logout per session; email change keeps the subject, freed address unlinked; suspension and disable close authority; Identity down is 503, public reading 200"
        );
        Ok(())
    }
}
