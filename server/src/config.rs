//! Settings, from the environment (one container, no config file).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use url::Url;

/// Where the server listens unless `ENCORE_SERVER_BIND` says otherwise.
/// Loopback only by default: nothing is reachable from the network until the
/// owner chooses an address.
pub const DEFAULT_BIND: &str = "127.0.0.1:8097";

/// The environment variable that sets the listen address, e.g. `0.0.0.0:8097`.
pub const BIND_ENV: &str = "ENCORE_SERVER_BIND";
/// The folder for the database (one volume; backups are a copy of it).
pub const DATA_DIR_ENV: &str = "ENCORE_DATA_DIR";
/// The media servers this server trusts, comma-separated. A device can only sign
/// in with an account on one of these: the server never takes a Jellyfin address
/// from a device, or a fake Jellyfin could claim to be anyone.
pub const JELLYFIN_URLS_ENV: &str = "ENCORE_JELLYFIN_URLS";

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub data_dir: PathBuf,
    pub jellyfin_urls: Vec<Url>,
    /// How long an access token works before the device refreshes it.
    pub access_ttl: Duration,
    /// How long an unused refresh token stays valid.
    pub refresh_ttl: Duration,
    /// The longest a pull may wait for something new.
    pub max_wait: Duration,
}

impl Config {
    /// Read the settings; `get` is the environment (a closure, so tests needn't
    /// touch the process's).
    pub fn from_env(get: impl Fn(&str) -> Option<String>) -> Result<Config, String> {
        let bind = bind_address(get(BIND_ENV).as_deref())?;
        let data_dir = get(DATA_DIR_ENV)
            .map(|d| d.trim().to_owned())
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| "/data".to_owned());
        let urls = get(JELLYFIN_URLS_ENV).unwrap_or_default();
        let jellyfin_urls = jellyfin_urls(&urls)?;
        Ok(Config {
            bind,
            data_dir: PathBuf::from(data_dir),
            jellyfin_urls,
            access_ttl: Duration::from_secs(60 * 60),
            refresh_ttl: Duration::from_secs(180 * 24 * 60 * 60),
            max_wait: Duration::from_secs(25),
        })
    }
}

/// Reads the listen address from `value` (the environment variable), or the default.
pub fn bind_address(value: Option<&str>) -> Result<SocketAddr, String> {
    let text = value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(DEFAULT_BIND);
    text.parse()
        .map_err(|_| format!("{BIND_ENV}={text:?} is not an address like 127.0.0.1:8097"))
}

/// The trusted media servers: at least one, each an http or https address.
pub fn jellyfin_urls(value: &str) -> Result<Vec<Url>, String> {
    let urls: Vec<Url> = value
        .split(',')
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(|u| {
            Url::parse(u)
                .ok()
                .filter(|p| matches!(p.scheme(), "http" | "https") && p.host().is_some())
                .ok_or_else(|| {
                    format!(
                        "{JELLYFIN_URLS_ENV}: {u:?} is not an address like http://jellyfin:8096"
                    )
                })
        })
        .collect::<Result<_, _>>()?;
    if urls.is_empty() {
        return Err(format!(
            "{JELLYFIN_URLS_ENV} is not set: name your Jellyfin, for example http://jellyfin:8096"
        ));
    }
    Ok(urls)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_to_loopback_by_default() {
        let addr = bind_address(None).unwrap();
        assert!(addr.ip().is_loopback());
        assert_eq!(bind_address(Some("  ")).unwrap(), addr);
        assert_eq!(bind_address(Some("0.0.0.0:9000")).unwrap().port(), 9000);
        assert!(
            bind_address(Some("not-an-address"))
                .unwrap_err()
                .contains(BIND_ENV)
        );
    }

    #[test]
    fn the_trusted_media_servers_must_be_named() {
        assert!(jellyfin_urls("").unwrap_err().contains("not set"));
        assert!(jellyfin_urls("ftp://x").is_err());
        let u = jellyfin_urls(" http://jellyfin:8096 , https://music.example ").unwrap();
        assert_eq!(u.len(), 2);
        let env = |k: &str| match k {
            JELLYFIN_URLS_ENV => Some("http://jellyfin:8096".to_owned()),
            _ => None,
        };
        let c = Config::from_env(env).unwrap();
        assert_eq!(c.data_dir, PathBuf::from("/data"));
        assert!(c.bind.ip().is_loopback());
    }
}
