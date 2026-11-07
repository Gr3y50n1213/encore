# Jellyfin fixtures: Phase 4

`recorded-phase4.json` is a read-only recording against a real Jellyfin 12.0.0 server,
made with `tools/fixture-recorder --phase4`, for the companion server's sign-in check and
the profile switcher's avatars. Anonymized like `recorded.json` (with its own random key,
not kept), and a real user image is replaced by the project's own art.

- `GET /Users/Me` named the signed-in user (server ID present, image set).
- The user image came back without any token (2729610 bytes, not PNG).
- After signing out, `GET /Users/Me` with the old token was refused (401).
