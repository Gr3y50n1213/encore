//! Phase 4's recording (`--phase4`): what the companion server and the profile
//! switcher need from Jellyfin, read-only (the owner's answer, 7 November 2026).
//!
//! 1. `GET /Users/Me` with the session token: who a token belongs to (the
//!    companion's sign-in check).
//! 2. `GET /Users/{id}/Images/Primary` with no token at all: whether user images
//!    are public, as the switcher's avatars assume. The real image is never
//!    written: the fixture gets the project's own art (see main.rs).
//! 3. Sign out, then `GET /Users/Me` with the same token: the refusal a revoked
//!    token gets.
//!
//! Nothing here changes anything on the server.

use std::time::Duration;

use encore_sources::SourceError;
use encore_sources::jellyfin::JellyfinClient;
use encore_sources::transport::{Method, Request, Transport};

#[derive(Debug, Default)]
pub struct Notes {
    pub lines: Vec<String>,
    pub warnings: Vec<String>,
}

pub async fn run(client: &JellyfinClient, transport: &dyn Transport) -> Notes {
    let mut n = Notes::default();
    let me = match client.me().await {
        Ok(me) => {
            n.lines.push(format!(
                "`GET /Users/Me` named the signed-in user (server ID {}, image {}).",
                if me.server_id.is_empty() {
                    "absent"
                } else {
                    "present"
                },
                if me.image_tag.is_some() {
                    "set"
                } else {
                    "not set"
                }
            ));
            Some(me)
        }
        Err(e) => {
            n.warnings.push(format!("`GET /Users/Me` failed: {e}"));
            None
        }
    };
    match me
        .as_ref()
        .and_then(|m| m.image_tag.as_ref().map(|t| (m, t)))
    {
        Some((me, tag)) => {
            let url = url::Url::parse(&format!(
                "{}/Users/{}/Images/Primary?tag={tag}&fillWidth=256&fillHeight=256&quality=90",
                client.base_url().as_str().trim_end_matches('/'),
                me.user_id
            ));
            match url {
                Ok(url) => {
                    let reply = transport
                        .send(Request {
                            method: Method::Get,
                            url,
                            headers: vec![],
                            body: None,
                            timeout: Duration::from_secs(15),
                        })
                        .await;
                    match reply {
                        Ok(r) if r.status == 200 && r.body.starts_with(b"\x89PNG") => n.lines.push(
                            "The user image came back without any token (PNG): user images are public.".into(),
                        ),
                        Ok(r) if r.status == 200 => n.lines.push(format!(
                            "The user image came back without any token ({} bytes, not PNG).",
                            r.body.len()
                        )),
                        Ok(r) => n.warnings.push(format!(
                            "The user image without a token answered {}: avatars need the token.",
                            r.status
                        )),
                        Err(e) => n.warnings.push(format!("The user image failed: {e}")),
                    }
                }
                Err(e) => n.warnings.push(format!("No image address: {e}")),
            }
        }
        None => n
            .lines
            .push("The user has no image: the switcher shows initials.".into()),
    }
    n
}

/// After signing out: the same token is refused.
pub async fn after_sign_out(client: &JellyfinClient, n: &mut Notes) {
    match client.me().await {
        Err(SourceError::SessionExpired) => n.lines.push(
            "After signing out, `GET /Users/Me` with the old token was refused (401).".into(),
        ),
        Ok(_) => n
            .warnings
            .push("After signing out, the old token still worked.".into()),
        Err(e) => n.warnings.push(format!(
            "After signing out, `GET /Users/Me` failed another way: {e}"
        )),
    }
}
