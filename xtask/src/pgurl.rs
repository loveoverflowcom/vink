//! Minimal PostgreSQL URL handling for scratch credentials: swap user, password and database,
//! and translate the connection into libpq variables for `psql`. TCP hosts only; the password
//! never appears in output.

pub struct PgUrl {
    host: String,
    port: u16,
    user: String,
    password: Option<String>,
    database: String,
    sslmode: Option<String>,
}

impl PgUrl {
    pub fn parse(url: &str) -> Result<Self, String> {
        let rest = url
            .strip_prefix("postgres://")
            .or_else(|| url.strip_prefix("postgresql://"))
            .ok_or("not a postgres:// or postgresql:// URL")?;
        let (rest, query) = rest.split_once('?').unwrap_or((rest, ""));
        let (authority, database) = rest.split_once('/').unwrap_or((rest, ""));
        let (userinfo, hostport) = match authority.rsplit_once('@') {
            Some((userinfo, hostport)) => (Some(userinfo), hostport),
            None => (None, authority),
        };
        let (user, password) = match userinfo.map(|u| u.split_once(':').unwrap_or((u, ""))) {
            Some((user, password)) => (
                decode(user)?,
                (!password.is_empty())
                    .then(|| decode(password))
                    .transpose()?,
            ),
            None => (String::new(), None),
        };
        let (host, port) = match hostport.strip_prefix('[') {
            Some(v6) => {
                let (host, after) = v6.split_once(']').ok_or("unterminated IPv6 host")?;
                (host.to_string(), after.strip_prefix(':'))
            }
            None => match hostport.rsplit_once(':') {
                Some((host, port)) => (host.to_string(), Some(port)),
                None => (hostport.to_string(), None),
            },
        };
        if host.is_empty() {
            return Err("the URL needs a TCP host".into());
        }
        let port = match port {
            Some(port) => port.parse().map_err(|_| "invalid port".to_string())?,
            None => 5432,
        };
        let sslmode = query
            .split('&')
            .find_map(|pair| pair.strip_prefix("sslmode="))
            .map(str::to_string);
        Ok(Self {
            host,
            port,
            user,
            password,
            database: decode(database)?,
            sslmode,
        })
    }

    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    /// The same server with another role and database.
    pub fn with(&self, user: &str, password: &str, database: &str) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        let query = self
            .sslmode
            .as_ref()
            .map(|mode| format!("?sslmode={}", encode(mode)))
            .unwrap_or_default();
        format!(
            "postgres://{}:{}@{host}:{}/{}{query}",
            encode(user),
            encode(password),
            self.port,
            encode(database)
        )
    }

    /// Same connection with only the database replaced.
    pub fn with_database(&self, database: &str) -> String {
        self.with(
            &self.user,
            self.password.as_deref().unwrap_or_default(),
            database,
        )
    }

    /// libpq environment for `psql`.
    pub fn psql_env(&self) -> Vec<(&'static str, String)> {
        let mut env = vec![
            ("PGHOST", self.host.clone()),
            ("PGPORT", self.port.to_string()),
            ("PGUSER", self.user.clone()),
            ("PGDATABASE", self.database.clone()),
        ];
        if let Some(password) = &self.password {
            env.push(("PGPASSWORD", password.clone()));
        }
        if let Some(mode) = &self.sslmode {
            env.push(("PGSSLMODE", mode.clone()));
        }
        env
    }
}

fn decode(text: &str) -> Result<String, String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = text
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
                .ok_or("invalid percent-encoding in URL")?;
            out.push(hex);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| "URL component is not UTF-8".into())
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swaps_credentials_and_database_but_keeps_the_server() {
        let url = PgUrl::parse("postgres://vink:p%40ss@127.0.0.1:55432/postgres?sslmode=disable")
            .unwrap();
        assert!(url.psql_env().contains(&("PGDATABASE", "postgres".into())));
        assert_eq!(
            url.with("r", "a b", "x"),
            "postgres://r:a%20b@127.0.0.1:55432/x?sslmode=disable"
        );
        assert_eq!(
            url.with_database("y"),
            "postgres://vink:p%40ss@127.0.0.1:55432/y?sslmode=disable"
        );
        let env = url.psql_env();
        assert!(env.contains(&("PGPASSWORD", "p@ss".into())));
        assert!(env.contains(&("PGSSLMODE", "disable".into())));
    }

    #[test]
    fn defaults_and_ipv6() {
        let url = PgUrl::parse("postgresql://u@[::1]/d").unwrap();
        assert_eq!(url.with("a", "b", "c"), "postgres://a:b@[::1]:5432/c");
        assert!(!url.psql_env().iter().any(|(k, _)| *k == "PGPASSWORD"));
        assert!(PgUrl::parse("mysql://u@h/d").is_err());
        assert!(PgUrl::parse("postgres:///d").is_err());
    }
}
