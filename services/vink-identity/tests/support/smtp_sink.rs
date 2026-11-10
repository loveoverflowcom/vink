//! A minimal SMTP relay for tests, on loopback TCP. It speaks enough of RFC 5321 for a real
//! client (EHLO, AUTH PLAIN, MAIL, RCPT, DATA, RSET, QUIT) and can be told to refuse, defer or
//! hang, so the transport and the delivery worker are exercised against a real conversation. It
//! is NOT a deliverability check: no TLS, DNS, SPF/DKIM or remote mailbox is involved.

#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

/// How the relay answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reply {
    Accept,
    /// `RCPT TO` answered with this status (4xx defers, 5xx refuses).
    RejectRcpt(u16),
    /// `DATA` content answered with this status.
    RejectData(u16),
    /// Greets with `421` (service not available), as an overloaded relay does.
    Unavailable,
    /// Accepts the connection and never says anything.
    Hang,
}

/// One accepted message.
#[derive(Debug, Clone)]
pub struct Received {
    pub rcpt: Vec<String>,
    pub data: String,
    /// The decoded `AUTH PLAIN` password, when the client authenticated.
    pub password: Option<String>,
}

impl Received {
    /// The message with a quoted-printable body decoded (headers unchanged).
    pub fn text(&self) -> String {
        let (head, body) = self
            .data
            .split_once("\r\n\r\n")
            .unwrap_or((self.data.as_str(), ""));
        if !head.to_ascii_lowercase().contains("quoted-printable") {
            return self.data.clone();
        }
        let joined = body.replace("=\r\n", "");
        let bytes = joined.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'='
                && i + 2 < bytes.len()
                && let Ok(byte) = u8::from_str_radix(&joined[i + 1..i + 3], 16)
            {
                out.push(byte);
                i += 3;
                continue;
            }
            out.push(bytes[i]);
            i += 1;
        }
        format!("{head}\r\n\r\n{}", String::from_utf8_lossy(&out))
    }
}

#[derive(Clone)]
pub struct Sink {
    port: u16,
    reply: Arc<Mutex<Reply>>,
    received: Arc<Mutex<Vec<Received>>>,
    connections: Arc<AtomicUsize>,
}

impl Sink {
    pub async fn start(reply: Reply) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let sink = Self {
            port: listener.local_addr().unwrap().port(),
            reply: Arc::new(Mutex::new(reply)),
            received: Arc::default(),
            connections: Arc::default(),
        };
        let shared = sink.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                shared.connections.fetch_add(1, Ordering::SeqCst);
                let session = shared.clone();
                tokio::spawn(async move {
                    let _ = session.serve(stream).await;
                });
            }
        });
        sink
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Changes how later commands are answered (a relay recovering or degrading).
    pub fn set(&self, reply: Reply) {
        *self.reply.lock().unwrap() = reply;
    }

    pub fn messages(&self) -> Vec<Received> {
        self.received.lock().unwrap().clone()
    }

    pub fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    /// A loopback port nothing listens on (bound, then released).
    pub fn unused_port() -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    }

    fn reply(&self) -> Reply {
        *self.reply.lock().unwrap()
    }

    async fn serve(&self, stream: tokio::net::TcpStream) -> std::io::Result<()> {
        let (read, mut write) = stream.into_split();
        let mut lines = BufReader::new(read);
        match self.reply() {
            Reply::Hang => {
                tokio::time::sleep(Duration::from_secs(30)).await;
                return Ok(());
            }
            Reply::Unavailable => {
                write.write_all(b"421 4.3.2 sink busy\r\n").await?;
                return Ok(());
            }
            _ => write.write_all(b"220 sink ESMTP\r\n").await?,
        }
        let mut rcpt = Vec::new();
        let mut password = None;
        let mut line = String::new();
        loop {
            line.clear();
            if lines.read_line(&mut line).await? == 0 {
                return Ok(());
            }
            let command = line.trim_end().to_string();
            let upper = command.to_ascii_uppercase();
            let answer: String = if upper.starts_with("EHLO") || upper.starts_with("HELO") {
                "250-sink\r\n250-AUTH PLAIN\r\n250 8BITMIME\r\n".into()
            } else if upper.starts_with("AUTH PLAIN") {
                let encoded = command.split_whitespace().nth(2).unwrap_or_default();
                password = decode_plain(encoded);
                "235 2.7.0 ok\r\n".into()
            } else if upper.starts_with("MAIL FROM") {
                rcpt.clear();
                "250 ok\r\n".into()
            } else if upper.starts_with("RCPT TO") {
                let address = command[8..].trim().to_string();
                match self.reply() {
                    // Real relays quote the recipient; the client must not keep this text.
                    Reply::RejectRcpt(status) => format!("{status} 5.1.1 {address} refused\r\n"),
                    _ => {
                        rcpt.push(address);
                        "250 ok\r\n".into()
                    }
                }
            } else if upper == "DATA" {
                write.write_all(b"354 end with .\r\n").await?;
                let mut data = String::new();
                loop {
                    line.clear();
                    if lines.read_line(&mut line).await? == 0 {
                        return Ok(());
                    }
                    if line == ".\r\n" || line == ".\n" {
                        break;
                    }
                    data.push_str(&line);
                }
                match self.reply() {
                    Reply::RejectData(status) => format!("{status} 5.7.1 content refused\r\n"),
                    _ => {
                        self.received.lock().unwrap().push(Received {
                            rcpt: rcpt.clone(),
                            data,
                            password: password.clone(),
                        });
                        "250 2.0.0 queued\r\n".into()
                    }
                }
            } else if upper == "RSET" || upper == "NOOP" {
                "250 ok\r\n".into()
            } else if upper == "QUIT" {
                write.write_all(b"221 bye\r\n").await?;
                return Ok(());
            } else {
                "502 5.5.2 not implemented\r\n".into()
            };
            write.write_all(answer.as_bytes()).await?;
        }
    }
}

fn decode_plain(encoded: &str) -> Option<String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    let text = String::from_utf8(bytes).ok()?;
    text.split('\0').nth(2).map(str::to_string)
}
