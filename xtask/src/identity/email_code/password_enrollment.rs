//! App facade and Identity enrollment over the real TCP and Unix sockets with both databases.

use serde_json::json;

use super::{Client, Flow, OsStr, PermissionsExt, Reply, Run, exchange, operator, sql};

const ENROLL: &str = "/api/v1/auth/password/enroll";
const CHALLENGES: &str = "/api/v1/auth/password/enroll/challenges";
const SESSION: &str = "/api/v1/auth/session";
const SESSION_COOKIE: &str = "__Host-vink_session";
const PROOF_COOKIE: &str = "__Host-vink_enroll";
const MAIL: &str = "enroll.tac.gia@vi-du.vn";
const COMMENT_PATH: &str = "/api/v1/works/33333333-3333-4333-8333-333333333333/discussion";

impl Run<'_> {
    pub(in crate::identity) fn password_enrollment_end_to_end(&self) -> Result<(), String> {
        let s = self.scratch;
        let uid = self.uid()?;
        let outbox = self.dir.join("enrollment-outbox");
        std::fs::create_dir(&outbox).map_err(|e| e.to_string())?;
        std::fs::set_permissions(&outbox, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        let identity = self.start_with(
            "enrollment-identity",
            &s.identity_runtime,
            Some(uid),
            &[
                ("VINK_IDENTITY_AUTHORITY", OsStr::new("native")),
                ("VINK_IDENTITY_LOCAL_OUTBOX", outbox.as_os_str()),
            ],
        )?;
        let (app, port) = self.start_app_with(
            "enrollment-app",
            &s.app_runtime,
            uid,
            &[
                ("VINK_AUTH_SIGN_IN", "enabled"),
                ("VINK_TRUSTED_PROXIES", "127.0.0.1"),
            ],
        )?;
        let mut flow = Flow {
            run: self,
            port,
            origin: format!("http://127.0.0.1:{port}"),
            outbox: &outbox,
            client: "198.51.100.210".into(),
            smtp: None,
            secrets: Vec::new(),
        };
        let checked = flow.check_enrollment();
        self.stop(app)?;
        self.stop(identity)?;
        let _ = std::fs::remove_dir_all(&outbox);
        checked?;
        for name in ["enrollment-identity", "enrollment-app"] {
            let log = std::fs::read_to_string(self.log(name)).map_err(|e| e.to_string())?;
            if flow.secrets.iter().any(|secret| log.contains(secret)) {
                return Err(format!("{name}.log contains enrollment input"));
            }
        }
        Ok(())
    }
}

impl Flow<'_> {
    fn check_enrollment(&mut self) -> Result<(), String> {
        for (method, path) in [("GET", ENROLL), ("POST", CHALLENGES), ("POST", ENROLL)] {
            no_store(&self.call(method, path, None, None)?)?.expect(
                "guest enrollment",
                401,
                "unauthenticated",
            )?;
        }
        let (reply, author) = self.sign_in(MAIL)?;
        reply.expect("email-code sign-in", 200, "needs_principal")?;
        let author = author.ok_or("missing original session")?;
        let subject = self.subject_of(&author)?;
        let session = author.cookie();
        let status = |flow: &Self, cookie: &str, enrolled: bool| {
            let reply = flow.call("GET", ENROLL, Some(cookie), None)?;
            no_store(&reply)?.expect("authenticated enrollment readback", 200, "enrolled")?;
            if reply.json() != json!({ "enrolled": enrolled }) || !reply.cookies.is_empty() {
                return Err("enrollment readback did not return only the account's status".into());
            }
            Ok::<_, String>(())
        };
        status(self, &session, false)?;

        let registration = r#"{"username":"enroll_other","password":"１２３４５６７８９０"}"#;
        let register =
            |flow: &Self, path, cookie| flow.call("POST", path, cookie, Some(registration));
        let before = sql(
            &self.run.scratch.admin_identity,
            "SELECT count(*) FROM accounts",
        )?;
        let unbound = register(self, "/api/v1/auth/register", Some(&session))?;
        no_store(&unbound)?.expect(
            "unbound account cannot register again",
            409,
            "already_signed_in",
        )?;
        if !unbound.cookies.is_empty()
            || sql(
                &self.run.scratch.admin_identity,
                "SELECT count(*) FROM accounts",
            )? != before
        {
            return Err("registration with a session created an account or set a cookie".into());
        }
        // An invalid Bearer must never shadow the browser account during registration.
        let shadowed = exchange(
            self.port,
            "POST",
            "/api/v1/auth/password/register",
            &[
                ("Origin", &self.origin),
                ("Cookie", &session),
                ("Authorization", "Bearer invalid-session"),
            ],
            Some(registration),
        )?;
        no_store(&shadowed)?.expect("browser session is protected", 409, "already_signed_in")?;
        if !shadowed.cookies.is_empty() {
            return Err("refused registration changed a cookie".into());
        }
        for origin in [None, Some("https://foreign.example")] {
            let mut headers = vec![("Cookie", session.as_str())];
            if let Some(origin) = origin {
                headers.push(("Origin", origin));
            }
            for path in [CHALLENGES, ENROLL] {
                let blocked = exchange(self.port, "POST", path, &headers, None)?;
                no_store(&blocked)?.expect("same-origin guard", 403, "forbidden")?;
            }
        }

        // A NeedsPrincipal account can request proof. Also exercise the Bearer entry point.
        let started = exchange(
            self.port,
            "POST",
            CHALLENGES,
            &[
                ("Origin", &self.origin),
                ("Authorization", &format!("Bearer {}", author.session)),
            ],
            None,
        )?;
        no_store(&started)?.expect("fresh proof issued", 202, "expires_at")?;
        let handle = started
            .cookie(PROOF_COOKIE)
            .ok_or("no enrollment proof cookie")?;
        let set_cookie = started
            .head
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                (name.eq_ignore_ascii_case("set-cookie") && value.contains(PROOF_COOKIE))
                    .then_some(value)
            })
            .ok_or("no enrollment Set-Cookie header")?;
        if ["Path=/", "Secure", "HttpOnly", "SameSite=Strict"]
            .iter()
            .any(|flag| !set_cookie.contains(flag))
            || started.body.contains(handle)
            || started.cookie(SESSION_COOKIE).is_some()
        {
            return Err("enrollment proof cookie is not private or changed the session".into());
        }
        self.secrets.push(handle.to_string());
        let code = self.code(handle)?;
        let proof = format!("{session}; {PROOF_COOKIE}={handle}");
        let username = "enroll_tac_gia";
        let password = "ệ".repeat(128);
        let nfd = "e\u{0323}\u{0302}".repeat(128);
        self.secrets
            .extend([username.to_owned(), password.clone(), nfd.clone()]);
        let complete = |flow: &Self, code: &str, username: &str, password: &str| {
            let body = json!({ "code": code, "username": username, "password": password });
            flow.call("POST", ENROLL, Some(&proof), Some(&body.to_string()))
        };

        let provisioned = self.call(
            "POST",
            "/api/v1/auth/principal",
            Some(&session),
            Some(r#"{"pen_name":"Tác Giả Gốc"}"#),
        )?;
        provisioned.expect("provision original principal", 201, "principal_id")?;
        let principal = field(&provisioned, "principal_id")?;
        let created = self.call("POST", "/api/v1/drafts", Some(&session), None)?;
        created.expect("create draft before enrollment", 201, "work_id")?;
        let work = format!("/api/v1/drafts/{}", field(&created, "work_id")?);
        let comment = self.call(
            "POST",
            &format!("{COMMENT_PATH}/comments"),
            Some(&session),
            Some(r#"{"body":"Bình luận của tác giả gốc.","command_key":"enrollment-comment-01"}"#),
        )?;
        comment.expect("submit comment before enrollment", 201, "comment_id")?;
        let comment_id = field(&comment, "comment_id")?;

        // Existing guest registration still creates an independent password-only account.
        let other = register(self, "/api/v1/auth/password/register", None)?;
        other.expect("guest registration", 200, "needs_principal")?;
        let other = Client {
            session: other
                .cookie(SESSION_COOKIE)
                .ok_or("guest session missing")?
                .to_string(),
        };
        self.secrets.push(other.session.clone());
        if self.subject_of(&other)? == subject {
            return Err("guest registration linked to the author's account".into());
        }
        status(self, &other.cookie(), true)?;
        self.call(
            "POST",
            "/api/v1/auth/principal",
            Some(&other.cookie()),
            Some(r#"{"pen_name":"Người Viết Khác"}"#),
        )?
        .expect("independent principal", 201, "principal_id")?;
        self.call("GET", &work, Some(&other.cookie()), None)?
            .expect("other account cannot read the draft", 404, "not_found")?;

        let app_db = [
            ("DATABASE_URL", self.run.scratch.app_runtime.as_str()),
            ("VINK_PUBLIC_ORIGIN", self.origin.as_str()),
        ];
        operator(
            self.run,
            "vink-app",
            &app_db,
            &["principal", "suspend", &principal, "enrollment test"],
        )?;
        no_store(&complete(self, &code, username, &password)?)?.expect(
            "suspended principal",
            403,
            "principal_suspended",
        )?;
        operator(
            self.run,
            "vink-app",
            &app_db,
            &["principal", "reinstate", &principal],
        )?;
        status(self, &session, false)?;

        let mut wrong = code.clone();
        wrong.replace_range(..1, if code.starts_with('0') { "1" } else { "0" });
        complete(self, &wrong, username, &password)?.expect("wrong proof", 422, "code_rejected")?;
        complete(self, &code, "enroll_other", &password)?.expect(
            "username collision",
            409,
            "username_taken",
        )?;
        complete(self, &code, username, &"a".repeat(4097))?.expect(
            "raw input bound",
            422,
            r#""field":"password""#,
        )?;
        let marker = "enroll-request-private-marker";
        let malformed = json!({
            "code": code, "username": username, "password": password, (marker): true
        });
        let refused = self.call("POST", ENROLL, Some(&proof), Some(&malformed.to_string()))?;
        no_store(&refused)?.expect("unknown field", 422, "validation_failed")?;
        if refused.body.contains(marker) {
            return Err("body validation returned private input".into());
        }
        self.secrets.push(marker.into());

        let before = enrollment_counts(self.run)?;
        let enrolled = complete(self, &code, username, &password)?;
        no_store(&enrolled)?.expect("enroll on the original account", 200, r#""enrolled":true"#)?;
        if enrolled.cookie(SESSION_COOKIE).is_some()
            || enrolled.cookie(PROOF_COOKIE) != Some("")
            || enrolled.json() != json!({"enrolled":true})
            || enrollment_counts(self.run)? != before
        {
            return Err(
                "enrollment changed accounts, sessions, principal bindings or response shape"
                    .into(),
            );
        }
        status(self, &session, true)?;
        complete(self, &code, username, &password)?.expect(
            "duplicate enrollment",
            409,
            "already_enrolled",
        )?;
        self.call("GET", &work, Some(&session), None)?.expect(
            "original session still reads the draft",
            200,
            "version",
        )?;

        self.next_client();
        for _ in 0..5 {
            self.call("POST", CHALLENGES, Some(&session), None)?
                .expect("no more proof needed", 409, "already_enrolled")?;
        }
        let limited = self.call("POST", CHALLENGES, Some(&session), None)?;
        no_store(&limited)?.expect("App challenge rate limit", 429, "rate_limited")?;
        if limited.retry_after.is_none() {
            return Err("network limit did not include Retry-After".into());
        }
        for _ in 0..15 {
            self.call("POST", ENROLL, Some(&session), Some("{}"))?
                .expect("invalid completion body", 422, "validation_failed")?;
        }
        let limited = self.call("POST", ENROLL, Some(&session), Some("{}"))?;
        no_store(&limited)?.expect("App completion rate limit", 429, "rate_limited")?;
        if limited.retry_after.is_none() {
            return Err("completion network limit did not include Retry-After".into());
        }
        self.call("DELETE", SESSION, Some(&session), None)?
            .expect("logout", 204, "")?;
        self.call("GET", ENROLL, Some(&session), None)?.expect(
            "revoked session cannot read back",
            401,
            "unauthenticated",
        )?;
        let login = json!({ "username": username.to_ascii_uppercase(), "password": nfd });
        let logged_in = self.call("POST", "/api/v1/auth/login", None, Some(&login.to_string()))?;
        logged_in.expect("password signs in to the same principal", 200, &principal)?;
        let password_client = Client {
            session: logged_in
                .cookie(SESSION_COOKIE)
                .ok_or("password session missing")?
                .to_string(),
        };
        self.secrets.push(password_client.session.clone());
        if self.subject_of(&password_client)? != subject {
            return Err("password signed in to a different subject".into());
        }
        let session = password_client.cookie();
        self.call("GET", "/api/v1/auth/session/account", Some(&session), None)?
            .expect("original pen name preserved", 200, "Tác Giả Gốc")?;
        self.call(
            "PUT",
            &work,
            Some(&session),
            Some(
                r#"{
            "expected_version":1,"title":"Nháp của tác giả gốc",
            "document":{"schema_version":1,"blocks":[{"lines":["Một dòng."]}]}
        }"#,
            ),
        )?
        .expect("existing draft remains editable", 200, r#""version":2"#)?;
        self.call("GET", &format!("{COMMENT_PATH}/me"), Some(&session), None)?
            .expect(
                "existing comment belongs to the same principal",
                200,
                &comment_id,
            )?;
        self.call("DELETE", SESSION, Some(&session), None)?
            .expect("logout again", 204, "")?;
        let (reply, author) = self.sign_in(MAIL)?;
        reply.expect(
            "email-code still reaches the same principal",
            200,
            &principal,
        )?;
        let author = author.ok_or("missing session on email-code return")?;
        if self.subject_of(&author)? != subject {
            return Err("email-code returned a different subject after enrollment".into());
        }
        self.call("GET", &work, Some(&author.cookie()), None)?
            .expect(
                "saved draft after switching methods",
                200,
                "Nháp của tác giả gốc",
            )
    }
}

fn enrollment_counts(run: &Run<'_>) -> Result<[i64; 4], String> {
    let s = run.scratch;
    Ok([
        sql(&s.admin_identity, "SELECT count(*) FROM accounts")?,
        sql(&s.admin_identity, "SELECT count(*) FROM sessions")?,
        sql(&s.admin_app, "SELECT count(*) FROM principal_bindings")?,
        sql(&s.admin_app, "SELECT count(*) FROM principals")?,
    ])
}

fn no_store(reply: &Reply) -> Result<&Reply, String> {
    if reply.head.lines().any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("cache-control") && value.trim() == "no-store"
        })
    }) {
        Ok(reply)
    } else {
        Err("auth response missing Cache-Control: no-store".into())
    }
}

fn field(reply: &Reply, name: &str) -> Result<String, String> {
    reply.json()[name]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("missing {name}"))
}
