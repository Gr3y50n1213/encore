# Encore for Roku

A small Roku channel that plays the music the Encore app on your phone sends it, and shows the song, the artist and the cover. The phone is the remote. The channel never signs in anywhere and stores nothing. It plays only from the phone's relay: a per-session address that can play nothing but the queue it was sent (ADR android-0007).

## Install it on your Roku (once)

Roku channels outside the Channel Store are installed through the Roku's developer mode.

1. **Let the phone control the Roku.** On the Roku, open Settings, System, Advanced system settings, Control by mobile apps, and choose **Enabled** (under Network access on some Rokus). With "Limited", the Roku refuses what Encore sends it. "Enabled" answers devices on your own network only; you don't need "Permissive".
2. **Turn on developer mode.** On the Roku remote, press Home three times, Up twice, then Right, Left, Right, Left, Right. Accept the terms, choose a password, and let the Roku restart. It shows its address, for example `http://192.168.1.x`.
3. **Get the channel.** Download `encore-roku.zip` from the Releases page, or from the `roku` workflow's run in GitHub Actions.
4. **Install it.** Open the Roku's address in a browser on the same Wi-Fi. Sign in as `rokudev` with the password from step 2, choose Upload, pick `encore-roku.zip`, then Install.

Encore appears among the Roku's channels. You don't need to open it: the phone starts it.

## Use it

In the Encore app, open Play on and pick the Roku under TVs. The channel opens on the TV and plays. Play, pause, skip and seek on the phone, or play and pause with the Roku remote. Pick "This phone" to bring the music back.

## How it talks to the phone

The phone uses Roku's External Control Protocol (ECP) on port 8060:

- `POST /launch/dev?url=…&title=…&artist=…&album=…&art=…&pos=…` starts the channel on a song;
- `POST /input?action=pause` (or `play`, `stop`, `seek&pos=…`, or a new `url=…`) drives it;
- `GET /query/media-player` reads back what is playing and where.
