# Security

Encore is a music player for your own media server, with an optional companion server you run yourself. It sends nothing anywhere else: there is no telemetry, no analytics and no account with the project.

## Reporting a vulnerability

Please report it privately through GitHub: the repository's **Security** tab, then **Report a vulnerability**. Don't open a public issue for it. Include what you found, how to reproduce it, and what it affects (the apps, the core, or the companion server). You'll get an answer there.

## What protects your data

- **Tokens**:
  - The apps keep media-server and companion tokens in the operating system's keychain or keystore. The core holds them in memory only and never writes them to disk or a log.
  - The companion server stores only SHA-256 hashes of its own tokens, which are 32 random bytes from the operating system. Refresh tokens rotate; reusing an old one signs the device out.
- **The network**:
  - https everywhere, with the operating system's certificate checks.
  - Plain http only to addresses on your home network.
  - A self-signed certificate is trusted only after you accept its fingerprint.
- **The companion server**:
  - It never stores music or your media-server token.
  - Devices only see devices of their own profile.
  - Sign-in is rate-limited.
  - The status page answers only on the home network.
  - The live link has size and rate limits.

  For use away from home, put it behind your own https reverse proxy or VPN. Behind a proxy, every request seems to come from the proxy's address, so the status page and the sign-in rate limit see one client.
- **Privacy**: Private listening, Guest and "Learn from my listening" off record nothing. History and likes stay on your devices and your own companion server.

## Supported versions

Encore is in development and has no releases yet. Fixes go to the main branch.
