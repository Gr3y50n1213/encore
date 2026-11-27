# Changelog

## 1.0.0 (2026)

The first release: the Android app, Encore TV and the companion server. The release notes are `docs/releases/v1.0.0.md`.

Security work before this release (an independent review of the companion server, 27 November 2026):

- **Sign-in:**
  - An Emby check uses a fresh device ID every time, so a token can't be taken for another person's earlier sign-in.
  - A Navidrome API key is named by the server, never by the name sent beside it.
  - A Navidrome profile's server is named by the companion, not by the device.
- **Rate limits:**
  - Sign-in is rate-limited per address, with IPv6 limited by its /64.
  - A second limit covers all sign-in attempts together.
  - Refreshes have their own limit.
  - Sign-in bodies are read only after the limit.
- **The status page** refuses requests that came through a proxy or tunnel.
- **Limits:**
  - A profile may have at most 100 devices.
  - Presence messages may be at most 16 KB.
  - Live messages are shared rather than copied for each device.
  - Each profile may run one mix build and one training at a time.
  - Every request has a two-minute limit.
- **Clearing listening history** can't be undone by a mix build or training that was running at the time.
- **Sound analysis** rows must have a version from 1 to 1000.
- **Android:** only Android Auto, the system's media controls, and apps you have trusted with media (watch apps, for example) may connect to Encore's media session. Other apps can no longer browse the library or the listening history through it.
