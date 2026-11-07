# ADR core-0009: Companion server sign-in and tokens

Date: 7 November 2026
Status: accepted

## Context
Plan section 5: "no new passwords". A profile is (media server, media-server user). The app sends its Jellyfin token once, the companion checks it with Jellyfin and issues its own long-lived device token; revoking a device signs it out everywhere. Section 12 adds rate-limited sign-in and short-lived access tokens with refresh. The owner chose, on 7 November 2026, to use only crates already in the build (rusqlite, sha2, getrandom, ulid, url, encore-sources).

The read-only recording of 7 November (`tools/fixtures/jellyfin/recorded-phase4.json`) showed on the owner's Jellyfin 12.0.0:
- `GET /Users/Me` names the token's user, with its server ID;
- after sign-out the same token gets 401;
- user images are served with no token.

## Decision
- **Trusted media servers only.** The server is configured with `ENCORE_JELLYFIN_URLS` and never takes a Jellyfin address from a device: a device naming its own "Jellyfin" could claim any user ID. The device sends the media server's ID; the companion finds that server among its trusted ones by `/System/Info/Public`, then asks `/Users/Me` with the token. The user record's server ID must agree. The Jellyfin token is used for that one check and stored nowhere.
- **Two tokens.**
  - Access tokens (`eat_`, 32 random bytes) work for 1 hour and live only in the app's memory.
  - Refresh tokens (`ert_`) last 180 days, live in the app's keychain (`source_id` 0 in `ProfileSummary.credentials`), and rotate on every use.
  - The server keeps only SHA-256 hashes, so a copy of its database can't sign anyone in, and looking a token up by its hash leaks nothing by timing.
- **Stolen-token detection, with a retry window.** A refresh token used twice was copied, so its device is revoked. One exception: the same token again within 2 minutes while its replacement is still unused is a retry (the reply was lost, or the app crashed before saving the new token). The server then issues a new pair and the unused replacement stops working.
- **Every rotation reaches the keychain.** The core hands the app each new refresh token in a `CredentialChanged` event. One refresh at a time per device (a lock), because two concurrent refreshes would look like theft.
- **Revocation.** `DELETE /devices/{id}` signs a device out everywhere. Its next request gets 401, and after one failed refresh the core reports `SignedOut`, forgets the token and keeps all local data.
- **Hardening.**
  - Sign-in and refresh are rate-limited to 10 per minute per address, in memory. Behind a reverse proxy, all devices share one bucket; forwarded-for headers are not trusted.
  - Request bodies are limited to 4 MB and events to 64 KB.
  - The status page answers only loopback and private addresses.
- **The address the user types.** https anywhere; plain http only on the home network (the same test as Jellyfin's addresses); no certificate pinning for the companion yet.

## Consequences
- A companion serves the Jellyfin servers it was set up for, and no others.
- Losing the keychain entry means connecting again. That is one exchange with the Jellyfin token, which the app has anyway.
- Profile PINs never reach the server: they are a device's screen lock (core-0011).
- Not yet: mDNS discovery (the owner deferred it), the WebSocket (Phase 7), and an admin page beyond the status counts.
