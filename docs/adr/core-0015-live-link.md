# ADR core-0015: The live link (GET /ws), presence and the nudge

Date: 7 November 2026
Status: accepted

## Context
Plan section 5 gives each device "one WebSocket per device for live updates (handoff, a like on another device, new recommendations)". Section 11 needs presence, push, pull and remote control. In Phase 4 the owner chose long-poll for sync and the WebSocket for Phase 7. For Phase 7 the owner approved `tokio-tungstenite`.

## Decision
- **One WebSocket per device:** `GET /ws` on the companion server, signed in with the access token like every route.
  - The server is axum's `ws` feature (tokio-tungstenite 0.29).
  - The core uses tokio-tungstenite 0.29 without its TLS features, over the core's own rustls and the operating system's verifier (`encore_sources::http::tls_config`), so there is one TLS stack and one set of certificate rules.
  - Every new crate's licence was already reviewed (`tools/ci/licences.json`). Older copies of base64, rand, getrandom and digest now sit beside the newer ones; they are small and permissively licensed.
- **Profile isolation is the server's job.** The hub is keyed by profile, then device:
  - `welcome`, `presence` and `nudge` reach only the same profile's devices;
  - a `relay` to any other ID is `undeliverable`, whatever the ID.

  Shared household devices, which would cross profiles, are later (the owner's choice).
- **Messages** are JSON text frames with a `type` (the full list is in `server/src/live.rs` and `docs/api/openapi.yaml`). Handoff and remote control are relay bodies (core-0016); the server only routes them and logs no bodies.
- **Presence is kept in memory only:** state, song, position by the server's clock, volume, Private.
  - A device that disconnects is announced offline.
  - `last_seen` is written when a link opens and closes.
  - `GET /devices` adds `online` and `presence`.
  - The core leaves devices unseen for 30 days out of the picker (board 12, item 6).
- **Clock:** `ping {t}` gets `pong {t, serverMs}`. The core keeps the sample with the shortest round trip (NTP's rule) as its offset to the server's clock, which both devices of a handoff measure against.
- **Nudge:** `/sync/push` sends `nudge` to the profile's other links. The long-poll pull still answers at once, so sync works without the link (behind a proxy that blocks WebSockets, say). The nudge is a hint, not a dependency.
- **Limits:**
  - 256 KB per message (a long queue's snapshot fits);
  - 40 messages a second per device (more closes the link);
  - a ping every 15 s, and a link silent for 45 s is dropped;
  - a newer link of the same device replaces the older one.
- **Sign-out:**
  - revoking a device or deleting the profile closes its link at once;
  - an open link re-checks its sign-in every minute;
  - an expired access token can't open a new link;
  - an open link outlives the token's hour only while the device stays signed in.
- **The core's loop:** a third task beside the pusher and the puller.
  - It reconnects with backoff (1 s up to 60 s), refreshing the token as sync does.
  - The output picker reports Unreachable, with the reason, while the link is down.

## Consequences
- Server tests over a real socket cover each message, the isolation, the limits, garbage, replacement and sign-out.
- The two-core end-to-end test runs handoff through it.
- Long-poll stays: one more open connection per device on the home network.
