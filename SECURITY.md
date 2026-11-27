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

  For use away from home, put it behind your own https reverse proxy or VPN. Behind a proxy, every request seems to come from the proxy's address, so all devices share one sign-in limit, and the status page refuses requests that came through a proxy.
  - A lost phone: sign it out in the app's Devices list, and on your media server too. The companion trusts any sign-in the media server still accepts.
- **The phone app**: only Android Auto, the system's media controls and apps you trusted with media (such as a watch app with notification access) can connect to Encore's media session.
- **Privacy**: Private listening, Guest and "Learn from my listening" off record nothing. History and likes stay on your devices and your own companion server.

## Supported versions

Fixes go to the main branch and the next release. Only the latest release on the Releases page is supported: update to it before reporting.

## Checking a download

Every release lists the SHA-256 of each file in `SHA256SUMS`. The Android APKs are signed with the project's own key, so Android refuses an APK signed by anyone else as an update to the one you have.
