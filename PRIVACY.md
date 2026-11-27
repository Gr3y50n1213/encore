# Privacy

Encore is a music player for your own music. The project runs no servers and collects nothing: no analytics, no crash reporting, no ads, no account with the project.

## What Encore keeps, and where

- **On your device:**
  - your library's list of songs;
  - your listening history, likes and playlists;
  - what Encore has learned about your taste;
  - downloads and settings.

  Each profile keeps its own. Android protects them with its file encryption. Sign-ins are kept in the Android Keystore, never in plain files or logs. Nothing is backed up to Google's cloud or copied to a new phone.
- **On your media server** (Jellyfin, Emby, Navidrome or another OpenSubsonic server, or Plex): what any player of that server sends, such as what is playing, play counts and likes. That server is yours.
- **On your companion server**, if you run one: your profiles' history, likes, mixes, devices and sound analysis, so your devices can share them. That server is also yours. It never stores music or your media-server password.

## What Encore connects to

- The media servers and the companion server you add. Lyrics and covers come from your media server or from the files themselves, never from an outside service.
- **Plex only:** plex.tv, which Plex itself uses for signing in and for finding your server. A companion server checking a Plex sign-in also asks plex.tv which account it belongs to.
- Nothing else.

Plain http is used only for addresses on your home network. Everywhere else it is https.

## Music on this phone

Encore reads only the folders you choose, and never changes or deletes a file. Nothing from those folders leaves the phone. "Stop using" a folder removes its songs from Encore and gives back Encore's access to it.

## Your choices

- **Private listening and Guest mode** record nothing.
- **"Learn from my listening"** can be turned off.
- **Settings › Privacy and data** lets you:
  - export everything (history, likes, playlists) as a file;
  - clear your listening history on every device;
  - delete a profile and all its data, here and on the companion server.

  Your media server's account and music are never touched.

## Changes

This page changes with the app. Its history is in the project's repository.
