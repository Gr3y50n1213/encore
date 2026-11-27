# Installing the companion server

The companion server (`encore-server`) runs beside your media server (Jellyfin, Emby, Navidrome or another OpenSubsonic server, or Plex), on any system those run on: Windows, Linux, macOS, or Docker (NAS boxes included). It keeps your listening history, profiles and learned taste in sync between your devices, carries music from one device to another, builds your mixes, and shares sound analysis. It never stores or plays music. It talks only to your own media server, and for Plex also to plex.tv, to learn which Plex account a sign-in belongs to.

The companion is optional: the app works without it. You need it only to share history, likes and mixes between devices, or to hand music from one device to another.

It is small: one program and one SQLite database.

## 1. Get it

- **Docker (any system, NAS):** the image is `ghcr.io/gr3y50n1213/encore-server` (amd64 and arm64), tagged with each release's version and `latest`. Put that name in `server/compose.yaml` in place of `encore-server:local`, then run `docker compose -f server/compose.yaml up -d`. Or build the image yourself: `docker build -f server/Dockerfile -t encore-server:local .` from the repository root.
- **Windows, macOS, Linux:** download `encore-server-<version>-<system>` from the [Releases page](https://github.com/Gr3y50n1213/encore/releases): Windows x64, macOS on Apple silicon, Linux x64 and arm64. Check it against `SHA256SUMS` on the same page. Or build it yourself: `cd server && cargo build --release --locked`.

## 2. Settings

Everything is set with environment variables.

| Variable | What it is | Default |
| --- | --- | --- |
| `ENCORE_MEDIA_URLS` | Your media server's address as this machine reaches it, for example `http://127.0.0.1:8096` (Jellyfin or Emby), `http://127.0.0.1:4533` (Navidrome) or `http://127.0.0.1:32400` (Plex). Several, comma-separated, of any kind. Required (the older name `ENCORE_JELLYFIN_URLS` still works). Devices can only sign in with an account on one of these. | none |
| `ENCORE_SERVER_BIND` | Where it listens. | `127.0.0.1:8097` (this machine only) |
| `ENCORE_DATA_DIR` | The database folder. Back this up. | Windows `C:\ProgramData\Encore Server`, macOS `~/Library/Application Support/Encore Server`, Linux `/var/lib/encore-server`, Docker `/data` |

Listening on `127.0.0.1` is the safe start: nothing on the network can reach it until you put Tailscale, a tunnel or a LAN address in front of it (step 4).

## 3. Keep it running

- **Windows:** run it as a service, like Jellyfin, with NSSM (Jellyfin's Windows installer ships it):
  ```
  nssm install EncoreServer "C:\path\to\encore-server.exe"
  nssm set EncoreServer AppEnvironmentExtra ENCORE_MEDIA_URLS=http://127.0.0.1:8096 ENCORE_DATA_DIR=D:\EncoreServer\data
  nssm set EncoreServer AppExit Default Restart
  nssm start EncoreServer
  ```
- **Linux (systemd):** `/etc/systemd/system/encore-server.service`:
  ```
  [Unit]
  Description=Encore companion server
  After=network-online.target

  [Service]
  ExecStart=/usr/local/bin/encore-server
  Environment=ENCORE_MEDIA_URLS=http://127.0.0.1:8096
  StateDirectory=encore-server
  DynamicUser=yes
  Restart=on-failure

  [Install]
  WantedBy=multi-user.target
  ```
  then `systemctl enable --now encore-server`.
- **macOS (launchd):** a LaunchAgent plist with `ProgramArguments` set to the program, `EnvironmentVariables` for the settings and `KeepAlive` true, loaded with `launchctl load`.
- **Docker:** `restart: unless-stopped` in `server/compose.yaml`.

`encore-server health` asks a running server whether it is well (exit code 0 when it is).

## 4. Reach it from your devices

The app accepts `https://` anywhere, and plain `http://` only on the home network.

- **Tailscale (recommended):** HTTPS on your tailnet, on every device signed in to Tailscale, at home or away, with nothing opened to the internet:
  ```
  tailscale serve --bg --https=8443 http://127.0.0.1:8097
  ```
  In the app: `https://<machine>.<tailnet>.ts.net:8443`. Use a different port from anything already served (Jellyfin through Funnel often has 443).
- **Home network:** set `ENCORE_SERVER_BIND=0.0.0.0:8097`, allow port 8097 in the machine's firewall for the private network only, and use `http://<machine's LAN address>:8097`. Tokens travel unencrypted on the LAN this way, so prefer Tailscale when you can.
- **A public tunnel (Cloudflare Tunnel or similar):** point the tunnel at `http://127.0.0.1:8097` and use its `https://` address. The server is then reachable from the internet. Sign-in still needs a real account on your media server, and attempts are rate-limited. Every request through the tunnel seems to come from this machine, so all devices share one sign-in limit. The status page (`/admin/status`) refuses any request that came through a proxy or tunnel, so it answers only on the machine itself and the home network.

## 5. Connect each device

Settings › Servers and sources › Companion server address, then Connect. Each device signs in with its own media-server sign-in, so there is no new password. The Devices list there shows every device and can sign one out.

**A lost phone:** sign it out in the Devices list on another device, and sign it out on your media server too (Jellyfin and Emby: Dashboard › Devices; Navidrome: change the password; Plex: Settings › Authorized Devices). The companion trusts a sign-in the media server still accepts, so a phone still signed in there could sign in to the companion again.

## 6. Back it up

`encore-server backup <file>` writes a consistent copy of the database while it runs. Copy that file somewhere safe (a nightly task is enough). To restore, stop the server and put the copy back as `encore-server.sqlite` in the data folder.
