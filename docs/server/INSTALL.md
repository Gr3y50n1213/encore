# Installing the companion server

The companion server (`encore-server`) runs beside your Jellyfin, on any system Jellyfin runs on: Windows, Linux, macOS, or Docker (NAS boxes included). It keeps your listening history, profiles and learned taste in sync between your devices, carries music from one device to another, builds your mixes, and shares sound analysis. It never stores or plays music, and it only talks to your own Jellyfin.

It is small: one program and one SQLite database.

## 1. Get it

- **Docker (any system, NAS):** build the image from this repository, `docker compose -f server/compose.yaml up -d` (it is not published to a registry yet).
- **Windows, macOS, Linux:** the `encore-server-<system>` files from the latest `core` workflow run (Windows x64, macOS Apple silicon, Linux x64 and arm64), or build it yourself: `cd server && cargo build --release --locked`.

## 2. Settings

Everything is set with environment variables.

| Variable | What it is | Default |
| --- | --- | --- |
| `ENCORE_MEDIA_URLS` | Your Jellyfin's or Navidrome's address as this machine reaches it, for example `http://127.0.0.1:8096`. Several, comma-separated, either kind. Required (the older name `ENCORE_JELLYFIN_URLS` still works). Devices can only sign in with an account on one of these. | none |
| `ENCORE_SERVER_BIND` | Where it listens. | `127.0.0.1:8097` (this machine only) |
| `ENCORE_DATA_DIR` | The database folder. Back this up. | Windows `C:\ProgramData\Encore Server`, macOS `~/Library/Application Support/Encore Server`, Linux `/var/lib/encore-server`, Docker `/data` |

Listening on `127.0.0.1` is the safe start: nothing on the network can reach it until you put Tailscale, a tunnel or a LAN address in front of it (step 4).

## 3. Keep it running

- **Windows:** run it as a service, like Jellyfin, with NSSM (Jellyfin's Windows installer ships it):
  ```
  nssm install EncoreServer "C:\path\to\encore-server.exe"
  nssm set EncoreServer AppEnvironmentExtra ENCORE_JELLYFIN_URLS=http://127.0.0.1:8096 ENCORE_DATA_DIR=D:\EncoreServer\data
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
  Environment=ENCORE_JELLYFIN_URLS=http://127.0.0.1:8096
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
- **A public tunnel (Cloudflare Tunnel or similar):** point the tunnel at `http://127.0.0.1:8097` and use its `https://` address. The server is then reachable from the internet: sign-in still needs a real account on your Jellyfin, attempts are rate-limited, and the admin page stays on the LAN.

## 5. Connect each device

Settings › Servers and sources › Companion server address, then Connect. Each device signs in with its own Jellyfin sign-in; no new password. The Devices list there shows every device and can sign one out.

## 6. Back it up

`encore-server backup <file>` writes a consistent copy of the database while it runs. Copy that file somewhere safe (a nightly task is enough). To restore, stop the server and put the copy back as `encore-server.sqlite` in the data folder.
