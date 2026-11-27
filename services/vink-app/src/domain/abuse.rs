//! Abuse protection for the sign-in edge (D-05): which network a request comes from, and how
//! many requests a network may make per window. Pure: no clock, no I/O.
//!
//! **Attribution.** The TCP peer is the only fact the App observes. `X-Forwarded-For` is read
//! only when that peer is a configured trusted proxy (`VINK_TRUSTED_PROXIES`), walking the list
//! from the right and skipping trusted hops; the first untrusted address is the client. A
//! client-supplied header on a direct connection is ignored, so it cannot choose its own bucket.
//! The `Forwarded` header (RFC 7239) is not read: the proxy must append to `X-Forwarded-For`.
//!
//! **Bucket key.** IPv4 counts per address; IPv6 per /64, because one host normally controls a
//! whole /64 and could otherwise rotate addresses freely. The key stored in the App database is
//! a SHA-256 of the bucket and the network: pseudonymised (an IPv4 address can be recovered by
//! trying all of them), kept only for the window plus a day.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use sha2::{Digest, Sha256};

/// One `address/prefix` range. A bare address is a single host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr {
    network: IpAddr,
    prefix: u8,
}

impl Cidr {
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        let (address, prefix) = match raw.split_once('/') {
            Some((address, prefix)) => (address, Some(prefix)),
            None => (raw, None),
        };
        let address: IpAddr = address.parse().ok()?;
        let max = if address.is_ipv4() { 32 } else { 128 };
        let prefix = match prefix {
            Some(text) if !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) => {
                text.parse::<u8>().ok().filter(|p| *p <= max)?
            }
            Some(_) => return None,
            None => max,
        };
        Some(Self {
            network: mask(address, prefix),
            prefix,
        })
    }

    pub fn contains(&self, address: IpAddr) -> bool {
        let address = canonical(address);
        address.is_ipv4() == self.network.is_ipv4() && mask(address, self.prefix) == self.network
    }
}

/// An IPv4-mapped IPv6 address (`::ffff:a.b.c.d`) is the IPv4 address it maps.
fn canonical(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(IpAddr::V6(v6), IpAddr::V4),
        v4 => v4,
    }
}

fn mask(address: IpAddr, prefix: u8) -> IpAddr {
    match canonical(address) {
        IpAddr::V4(v4) => {
            let bits = u32::from(v4);
            let keep = if prefix == 0 {
                0
            } else {
                u32::MAX << (32 - u32::from(prefix))
            };
            IpAddr::V4(Ipv4Addr::from(bits & keep))
        }
        IpAddr::V6(v6) => {
            let bits = u128::from(v6);
            let keep = if prefix == 0 {
                0
            } else {
                u128::MAX << (128 - u32::from(prefix))
            };
            IpAddr::V6(Ipv6Addr::from(bits & keep))
        }
    }
}

/// The proxies whose `X-Forwarded-For` is believed. Empty by default: only the TCP peer counts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustedProxies(Vec<Cidr>);

impl TrustedProxies {
    /// A comma-separated list of addresses or CIDR ranges. `None` names the first bad entry.
    pub fn parse(raw: &str) -> Result<Self, String> {
        raw.split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(|entry| Cidr::parse(entry).ok_or_else(|| entry.to_string()))
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn trusts(&self, address: IpAddr) -> bool {
        self.0.iter().any(|cidr| cidr.contains(address))
    }
}

/// The client address of a request. `forwarded` are the `X-Forwarded-For` header values in
/// arrival order. Returns the peer unless it is trusted; then the right-most untrusted hop. An
/// unparsable hop stops the walk at the last trusted hop that sent it, so garbage never selects
/// a bucket and never lets a request escape into a fresh one.
pub fn client_address(peer: IpAddr, forwarded: &[&str], trusted: &TrustedProxies) -> IpAddr {
    let peer = canonical(peer);
    if !trusted.trusts(peer) {
        return peer;
    }
    let hops: Vec<&str> = forwarded
        .iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .collect();
    let mut nearest = peer;
    for hop in hops.iter().rev() {
        let Ok(address) = hop.parse::<IpAddr>() else {
            return nearest;
        };
        let address = canonical(address);
        if !trusted.trusts(address) {
            return address;
        }
        nearest = address;
    }
    nearest
}

/// The rate-limit unit of an address: IPv4 /32, IPv6 /64.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Network(IpAddr);

impl Network {
    pub fn of(address: IpAddr) -> Self {
        let address = canonical(address);
        Self(match address {
            IpAddr::V4(_) => address,
            IpAddr::V6(_) => mask(address, 64),
        })
    }

    /// Stored key for `bucket`: never the address itself.
    pub fn key(&self, bucket: &str) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"vink-app/rate-limit/v1\0");
        hasher.update(bucket.as_bytes());
        hasher.update([0]);
        match self.0 {
            IpAddr::V4(v4) => hasher.update(v4.octets()),
            IpAddr::V6(v6) => hasher.update(v6.octets()),
        }
        hasher.finalize().into()
    }
}

/// A ceiling of `limit` requests per `window_seconds` per network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimit {
    pub bucket: &'static str,
    pub limit: i64,
    pub window_seconds: i64,
}

/// Asking for a code: a network may ask for codes for several addresses (a shared household or
/// office), but not spray hundreds. Identity separately caps codes per address (3/15 min), per
/// account and globally.
pub const SIGN_IN_CHALLENGE: RateLimit = RateLimit {
    bucket: "sign_in.challenge",
    limit: 10,
    window_seconds: 15 * 60,
};

/// Answering codes: each challenge allows 5 attempts; this caps guessing across challenges.
pub const SIGN_IN_COMPLETE: RateLimit = RateLimit {
    bucket: "sign_in.complete",
    limit: 30,
    window_seconds: 15 * 60,
};

/// Answering username/password: caps password spraying across accounts from a single network.
pub const PASSWORD_LOGIN: RateLimit = RateLimit {
    bucket: "password.login",
    limit: 15,
    window_seconds: 15 * 60,
};

/// Registering with username/password: caps bulk account creation from a single network.
pub const PASSWORD_REGISTER: RateLimit = RateLimit {
    bucket: "password.register",
    limit: 5,
    window_seconds: 60 * 60,
};

/// Asking for the code that proves control of one's own mailbox before adding a username and
/// password. Authenticated, so a network is a household or office, not a spray source; Identity
/// separately caps codes per address (3/15 min) and per account.
pub const PASSWORD_ENROLL_CHALLENGE: RateLimit = RateLimit {
    bucket: "password.enroll.challenge",
    limit: 5,
    window_seconds: 15 * 60,
};

/// Answering that code with a username and password: each challenge allows 5 attempts and each
/// attempt costs one Argon2 hash, so this also caps CPU spent for one network.
pub const PASSWORD_ENROLL: RateLimit = RateLimit {
    bucket: "password.enroll",
    limit: 15,
    window_seconds: 15 * 60,
};

/// Decision for one request, given the window's count INCLUDING this request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    Admitted,
    Refused { retry_after_seconds: u32 },
}

pub fn decide(limit: &RateLimit, hits: i64, seconds_left: i64) -> Admission {
    if hits <= limit.limit {
        Admission::Admitted
    } else {
        Admission::Refused {
            retry_after_seconds: u32::try_from(seconds_left.clamp(1, limit.window_seconds))
                .unwrap_or(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(raw: &str) -> IpAddr {
        raw.parse().unwrap()
    }

    #[test]
    fn cidrs_parse_strictly_and_match_by_prefix() {
        let ten = Cidr::parse("10.0.0.0/8").unwrap();
        assert!(ten.contains(ip("10.200.3.4")));
        assert!(!ten.contains(ip("11.0.0.1")));
        assert!(ten.contains(ip("::ffff:10.1.2.3")), "mapped IPv4");
        let host = Cidr::parse("192.0.2.7").unwrap();
        assert!(host.contains(ip("192.0.2.7")) && !host.contains(ip("192.0.2.8")));
        let v6 = Cidr::parse("2001:db8::/32").unwrap();
        assert!(v6.contains(ip("2001:db8:1::1")) && !v6.contains(ip("2001:db9::1")));
        assert!(!v6.contains(ip("10.0.0.1")));
        assert!(
            Cidr::parse("0.0.0.0/0")
                .unwrap()
                .contains(ip("203.0.113.9"))
        );
        for bad in [
            "",
            "10.0.0.0/33",
            "10.0.0.0/",
            "10.0.0.0/+8",
            "10.0.0/8",
            "::/129",
            "proxy.local",
            "10.0.0.1 /8",
        ] {
            assert_eq!(Cidr::parse(bad), None, "{bad:?}");
        }
        assert_eq!(
            TrustedProxies::parse("10.0.0.0/8, ::1,").unwrap().0.len(),
            2
        );
        assert_eq!(
            TrustedProxies::parse("10.0.0.0/8,nope").unwrap_err(),
            "nope"
        );
    }

    /// The spoofing cases: a direct client cannot pick its address; behind trusted proxies the
    /// right-most untrusted hop wins, whatever the client prepended.
    #[test]
    fn forwarded_for_is_believed_only_from_trusted_proxies() {
        let none = TrustedProxies::default();
        let proxies = TrustedProxies::parse("10.0.0.0/8").unwrap();
        let attacker = ip("203.0.113.66");
        // Direct connection with a forged header: the header is ignored.
        assert_eq!(client_address(attacker, &["198.51.100.1"], &none), attacker);
        assert_eq!(
            client_address(attacker, &["198.51.100.1"], &proxies),
            attacker
        );
        // Through the proxy, a client-prepended entry is to the LEFT of what the proxy appended.
        let proxy = ip("10.0.0.5");
        assert_eq!(
            client_address(proxy, &["198.51.100.1, 203.0.113.66"], &proxies),
            attacker
        );
        // Several trusted hops are skipped; repeated headers are concatenated in order.
        assert_eq!(
            client_address(proxy, &["203.0.113.66", "10.1.1.1"], &proxies),
            attacker
        );
        // Missing header from a proxy: the proxy itself (stricter, never a fresh bucket).
        assert_eq!(client_address(proxy, &[], &proxies), proxy);
        // Garbage stops at the nearest trusted hop.
        assert_eq!(
            client_address(proxy, &["203.0.113.66, garbage"], &proxies),
            proxy
        );
        assert_eq!(
            client_address(proxy, &["garbage, 10.2.2.2"], &proxies),
            ip("10.2.2.2")
        );
        // Mapped addresses are normalised.
        assert_eq!(
            client_address(ip("::ffff:10.0.0.5"), &["203.0.113.66"], &proxies),
            attacker
        );
    }

    #[test]
    fn networks_group_ipv6_by_64_and_keys_are_opaque() {
        let a = Network::of(ip("2001:db8:1:2::1"));
        let b = Network::of(ip("2001:db8:1:2:ffff::9"));
        let c = Network::of(ip("2001:db8:1:3::1"));
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(
            Network::of(ip("::ffff:192.0.2.1")),
            Network::of(ip("192.0.2.1"))
        );
        let v4 = Network::of(ip("192.0.2.1"));
        assert_ne!(v4.key("a"), v4.key("b"), "buckets are independent");
        assert_ne!(v4.key("a"), Network::of(ip("192.0.2.2")).key("a"));
        assert!(!v4.key("a").windows(4).any(|w| w == [192, 0, 2, 1]));
    }

    #[test]
    fn admission_counts_this_request_and_bounds_retry_after() {
        let limit = SIGN_IN_CHALLENGE;
        for hits in 1..=limit.limit {
            assert_eq!(decide(&limit, hits, 100), Admission::Admitted);
        }
        assert_eq!(
            decide(&limit, limit.limit + 1, 100),
            Admission::Refused {
                retry_after_seconds: 100
            }
        );
        for left in [-5, 0, 1, limit.window_seconds + 50] {
            let Admission::Refused {
                retry_after_seconds,
            } = decide(&limit, i64::MAX, left)
            else {
                panic!()
            };
            assert!((1..=limit.window_seconds as u32).contains(&retry_after_seconds));
        }
    }
}
