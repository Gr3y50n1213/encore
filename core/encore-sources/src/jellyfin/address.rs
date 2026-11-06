//! The address rules of board 21, item 3.
//!
//! - A bare host or a host with a port is accepted; `https` is added when missing.
//! - Without a port, https on 443 and 8920 is tried, then plain http on 8096 for a
//!   local address only (Jellyfin's defaults are 8096 http and 8920 https).
//! - Plain http to a local address is allowed with a quiet warning; plain http to
//!   anything else is refused ("This address is not secure. Use https").
//! - A path after the host is kept, for servers behind a reverse proxy
//!   (`https://example.org/jellyfin`).
//!
//! "Local" means: private IPv4 (10/8, 172.16/12, 192.168/16), loopback, link-local,
//! the 100.64/10 range Tailscale uses, IPv6 loopback, unique-local and link-local,
//! `localhost`, single-label names (`nas`), and the private-use suffixes `.local`,
//! `.home.arpa`, `.lan` and `.internal`.

use std::net::{Ipv4Addr, Ipv6Addr};

use url::{Host, Url};

use crate::error::{Result, SourceError};

/// A warning to show quietly under the address field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressWarning {
    /// Plain http on the local network.
    PlainHttpOnLocalNetwork,
}

/// The addresses to try, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressPlan {
    pub candidates: Vec<Url>,
}

pub fn is_local_v4(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || (o[0] == 100 && (o[1] & 0xC0) == 64)
}

pub fn is_local_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    ip.is_loopback() || (first & 0xFE00) == 0xFC00 || (first & 0xFFC0) == 0xFE80
}

pub fn is_local_host(host: &Host<&str>) -> bool {
    match host {
        Host::Ipv4(ip) => is_local_v4(*ip),
        Host::Ipv6(ip) => is_local_v6(*ip),
        Host::Domain(d) => {
            let d = d.trim_end_matches('.').to_ascii_lowercase();
            d == "localhost"
                || !d.contains('.')
                || [".local", ".home.arpa", ".lan", ".internal"]
                    .iter()
                    .any(|s| d.ends_with(s))
        }
    }
}

/// Whether a URL may be used, and with which warning (board 21, item 3).
pub fn check_security(url: &Url) -> Result<Option<AddressWarning>> {
    match url.scheme() {
        "https" => Ok(None),
        "http" => {
            let host = url
                .host()
                .ok_or_else(|| SourceError::InvalidAddress("no host".into()))?;
            if is_local_host(&host) {
                Ok(Some(AddressWarning::PlainHttpOnLocalNetwork))
            } else {
                Err(SourceError::InsecureAddress)
            }
        }
        other => Err(SourceError::InvalidAddress(format!(
            "{other} is not a web address"
        ))),
    }
}

/// Work out which addresses to try for what the user typed.
pub fn plan_address(input: &str, https_ports: &[u16], http_port: u16) -> Result<AddressPlan> {
    let typed = input.trim();
    if typed.is_empty() {
        return Err(SourceError::InvalidAddress(
            "type the server's address".into(),
        ));
    }
    let has_scheme = typed.contains("://");
    let parsed = Url::parse(&if has_scheme {
        typed.to_owned()
    } else {
        format!("https://{typed}")
    })
    .map_err(|e| SourceError::InvalidAddress(e.to_string()))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| SourceError::InvalidAddress("no host".into()))?
        .to_owned();
    if parsed.username() != "" || parsed.password().is_some() {
        return Err(SourceError::InvalidAddress(
            "leave the user name out of the address".into(),
        ));
    }
    let path = parsed.path().trim_end_matches('/').to_owned();
    let local = parsed.host().map(|h| is_local_host(&h)).unwrap_or(false);
    let base = |scheme: &str, port: Option<u16>| -> Result<Url> {
        let host_part = if host.contains(':') && !host.starts_with('[') {
            format!("[{host}]")
        } else {
            host.clone()
        };
        let port_part = port.map(|p| format!(":{p}")).unwrap_or_default();
        Url::parse(&format!("{scheme}://{host_part}{port_part}{path}"))
            .map_err(|e| SourceError::InvalidAddress(e.to_string()))
    };

    let mut candidates = Vec::new();
    if has_scheme {
        check_security(&parsed)?;
        candidates.push(base(parsed.scheme(), parsed.port())?);
    } else if let Some(port) = parsed.port() {
        candidates.push(base("https", Some(port))?);
        if local {
            candidates.push(base("http", Some(port))?);
        }
    } else {
        for &p in https_ports {
            candidates.push(base("https", if p == 443 { None } else { Some(p) })?);
        }
        if local {
            candidates.push(base("http", Some(http_port))?);
        }
    }
    Ok(AddressPlan { candidates })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(s: &str) -> Vec<String> {
        plan_address(s, &[443, 8920], 8096)
            .unwrap()
            .candidates
            .iter()
            .map(|u| u.as_str().trim_end_matches('/').to_owned())
            .collect()
    }

    #[test]
    fn a_bare_public_host_tries_https_only() {
        assert_eq!(
            plan("music.example.org"),
            [
                "https://music.example.org",
                "https://music.example.org:8920"
            ]
        );
    }

    #[test]
    fn a_bare_local_host_also_tries_plain_http_on_8096() {
        assert_eq!(
            plan("192.168.1.20"),
            [
                "https://192.168.1.20",
                "https://192.168.1.20:8920",
                "http://192.168.1.20:8096"
            ]
        );
        assert_eq!(plan("nas.local").last().unwrap(), "http://nas.local:8096");
        assert_eq!(plan("nas").last().unwrap(), "http://nas:8096");
    }

    #[test]
    fn a_typed_port_is_kept() {
        assert_eq!(
            plan("10.0.0.5:9000"),
            ["https://10.0.0.5:9000", "http://10.0.0.5:9000"]
        );
        assert_eq!(
            plan("music.example.org:8443"),
            ["https://music.example.org:8443"]
        );
    }

    #[test]
    fn a_typed_scheme_is_used_as_is() {
        assert_eq!(plan("http://localhost:8096"), ["http://localhost:8096"]);
        assert_eq!(
            plan("https://music.example.org/jellyfin/"),
            ["https://music.example.org/jellyfin"]
        );
    }

    #[test]
    fn plain_http_to_a_public_address_is_refused() {
        assert_eq!(
            plan_address("http://music.example.org", &[443], 8096),
            Err(SourceError::InsecureAddress)
        );
        assert_eq!(
            plan_address("http://8.8.8.8:8096", &[443], 8096),
            Err(SourceError::InsecureAddress)
        );
    }

    #[test]
    fn plain_http_on_the_local_network_warns() {
        let u = Url::parse("http://192.168.0.2:8096").unwrap();
        assert_eq!(
            check_security(&u).unwrap(),
            Some(AddressWarning::PlainHttpOnLocalNetwork)
        );
        let u = Url::parse("https://music.example.org").unwrap();
        assert_eq!(check_security(&u).unwrap(), None);
    }

    #[test]
    fn local_ranges() {
        for ip in [
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.0.1",
            "127.0.0.1",
            "169.254.1.1",
            "100.64.0.1",
            "100.127.255.254",
        ] {
            assert!(is_local_v4(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["8.8.8.8", "172.32.0.1", "100.128.0.1", "1.1.1.1"] {
            assert!(!is_local_v4(ip.parse().unwrap()), "{ip}");
        }
        assert!(is_local_v6("::1".parse().unwrap()));
        assert!(is_local_v6("fd12:3456::1".parse().unwrap()));
        assert!(is_local_v6("fe80::1".parse().unwrap()));
        assert!(!is_local_v6("2606:4700::1111".parse().unwrap()));
        assert_eq!(plan("[fd00::5]").last().unwrap(), "http://[fd00::5]:8096");
    }

    #[test]
    fn nonsense_is_explained() {
        assert!(matches!(
            plan_address("  ", &[443], 8096),
            Err(SourceError::InvalidAddress(_))
        ));
        assert!(matches!(
            plan_address("ftp://x", &[443], 8096),
            Err(SourceError::InvalidAddress(_))
        ));
        assert!(matches!(
            plan_address("https://user:pw@host", &[443], 8096),
            Err(SourceError::InvalidAddress(_))
        ));
    }
}
