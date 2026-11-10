//! A loopback SMTP relay for the two-binary check (std threads, no TLS). It accepts AUTH PLAIN,
//! records each message and the password presented, and can be switched to answer `421` at the
//! greeting, as an unavailable relay does. It proves that the real `vink-identity` binary submits
//! codes over SMTP with its configured credential; it is NOT a deliverability check (no TLS, DNS,
//! SPF/DKIM, Resend or remote mailbox is involved).

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Received {
    pub data: String,
    pub password: Option<String>,
}

impl Received {
    /// The message with its quoted-printable body decoded.
    pub fn text(&self) -> String {
        let joined = self.data.replace("=\r\n", "");
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
        String::from_utf8_lossy(&out).into_owned()
    }

    /// The six-digit code of the body sentence "… của bạn là NNNNNN."
    pub fn code(&self) -> Option<String> {
        let text = self.text();
        let marker = "của bạn là ";
        let start = text.find(marker)? + marker.len();
        text.get(start..start + 6).map(str::to_string)
    }
}

#[derive(Clone)]
pub struct Sink {
    pub port: u16,
    pub received: Arc<Mutex<Vec<Received>>>,
    pub unavailable: Arc<AtomicBool>,
}

impl Sink {
    pub fn start() -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let sink = Self {
            port: listener.local_addr().map_err(|e| e.to_string())?.port(),
            received: Arc::default(),
            unavailable: Arc::default(),
        };
        let shared = sink.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let session = shared.clone();
                std::thread::spawn(move || {
                    let _ = session.serve(stream);
                });
            }
        });
        Ok(sink)
    }

    pub fn set_unavailable(&self, unavailable: bool) {
        self.unavailable.store(unavailable, Ordering::SeqCst);
    }

    pub fn messages(&self) -> Vec<Received> {
        self.received.lock().map(|m| m.clone()).unwrap_or_default()
    }

    fn serve(&self, stream: TcpStream) -> std::io::Result<()> {
        let mut write = stream.try_clone()?;
        let mut lines = BufReader::new(stream);
        if self.unavailable.load(Ordering::SeqCst) {
            return write.write_all(b"421 4.3.2 relay busy\r\n");
        }
        write.write_all(b"220 sink ESMTP\r\n")?;
        let mut password = None;
        let mut line = String::new();
        loop {
            line.clear();
            if lines.read_line(&mut line)? == 0 {
                return Ok(());
            }
            let command = line.trim_end().to_string();
            let upper = command.to_ascii_uppercase();
            if upper.starts_with("EHLO") || upper.starts_with("HELO") {
                write.write_all(b"250-sink\r\n250-AUTH PLAIN\r\n250 8BITMIME\r\n")?;
            } else if upper.starts_with("AUTH PLAIN") {
                password = command
                    .split_whitespace()
                    .nth(2)
                    .and_then(decode_plain_password);
                write.write_all(b"235 2.7.0 ok\r\n")?;
            } else if upper == "DATA" {
                write.write_all(b"354 end with .\r\n")?;
                let mut data = String::new();
                loop {
                    line.clear();
                    if lines.read_line(&mut line)? == 0 {
                        return Ok(());
                    }
                    if line == ".\r\n" || line == ".\n" {
                        break;
                    }
                    data.push_str(&line);
                }
                if let Ok(mut received) = self.received.lock() {
                    received.push(Received {
                        data,
                        password: password.clone(),
                    });
                }
                write.write_all(b"250 2.0.0 queued\r\n")?;
            } else if upper == "QUIT" {
                return write.write_all(b"221 bye\r\n");
            } else {
                write.write_all(b"250 ok\r\n")?;
            }
        }
    }
}

fn decode_plain_password(encoded: &str) -> Option<String> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bits = 0u32;
    let mut count = 0;
    let mut out = Vec::new();
    for byte in encoded.bytes().filter(|b| *b != b'=') {
        let value = ALPHABET.iter().position(|c| *c == byte)? as u32;
        bits = (bits << 6) | value;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    let text = String::from_utf8(out).ok()?;
    text.split('\0').nth(2).map(str::to_string)
}
