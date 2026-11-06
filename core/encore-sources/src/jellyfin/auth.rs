//! The `Authorization` header Jellyfin 12 accepts.
//!
//! Jellyfin 12.0 turned off the legacy ways of sending a token (`X-Emby-Token`,
//! `X-Emby-Authorization`, `X-MediaBrowser-Token`, the `api_key` parameter and the
//! `Emby` scheme). Only `Authorization: MediaBrowser ...` and the `ApiKey` query
//! parameter remain (release notes v12.0; jellyfin PRs 13306 and 15559). The server
//! URL-decodes every value (AuthorizationContext.cs, `WebUtility.UrlDecode`), so
//! values are percent-encoded here.

use std::fmt;

/// Who is calling, as the server's Devices page shows it.
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub client: String,
    pub device_name: String,
    pub device_id: String,
    pub version: String,
}

impl fmt::Debug for DeviceInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceInfo")
            .field("client", &self.client)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

/// Percent-encode everything but unreserved characters (RFC 3986), so the
/// server's URL decoding gives back exactly the original text.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The header value. `token` is None before sign-in.
pub fn authorization_header(device: &DeviceInfo, token: Option<&str>) -> String {
    let mut h = format!(
        "MediaBrowser Client=\"{}\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\"",
        encode(&device.client),
        encode(&device.device_name),
        encode(&device.device_id),
        encode(&device.version)
    );
    if let Some(t) = token {
        h.push_str(&format!(", Token=\"{}\"", encode(t)));
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(name: &str) -> DeviceInfo {
        DeviceInfo {
            client: "Encore".into(),
            device_name: name.into(),
            device_id: "abc-123".into(),
            version: "0.1.0".into(),
        }
    }

    #[test]
    fn header_before_and_after_sign_in() {
        assert_eq!(
            authorization_header(&device("Pixel"), None),
            "MediaBrowser Client=\"Encore\", Device=\"Pixel\", DeviceId=\"abc-123\", Version=\"0.1.0\""
        );
        assert!(authorization_header(&device("Pixel"), Some("tok")).ends_with(", Token=\"tok\""));
    }

    #[test]
    fn names_with_spaces_quotes_commas_plus_and_non_latin_survive_url_decoding() {
        let h = authorization_header(&device("Sam's \"Pixel\", 2+2 東京"), None);
        assert!(
            h.contains("Device=\"Sam%27s%20%22Pixel%22%2C%202%2B2%20%E6%9D%B1%E4%BA%AC\""),
            "{h}"
        );
        // Four quoted values: no quote from the name leaks into the header syntax.
        assert_eq!(h.matches('"').count(), 8);
    }
}
