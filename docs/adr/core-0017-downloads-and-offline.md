# ADR core-0017: Downloads and offline live in the core

Date: 8 November 2026
Status: accepted

## Context
Phase 8 (ROADMAP: "a downloaded playlist plays in airplane mode and in a car"). The plan, section 8, says to download any song, album or playlist, plus Liked songs and mixes as automatic options. Downloads live per device and are listed per profile. A storage page shows usage, and offline mode hides or dims what can't play. Boards 06, 08, 11, 13, 17 and 20 draw it. Board 14 says audio files may be shared between profiles but the lists that point to them never are. The owner approved the defaults on 8 November 2026: Original quality, Wi-Fi only, no limit, auto-remove off, Liked songs and Smart downloads off, a 1 GB rolling cache.

## Decision
**What the core keeps per profile.** Collections (a song, an album, a playlist, a mix, Liked songs, Smart downloads), the songs each holds now, and each wanted song's state, in migration 8.

**How it fetches.** The core does the transfer itself, through `Transport::download`: reqwest streaming chunks to `<data>/downloads/.partial`, with Range resume, a per-chunk timeout and a cancel flag. That way pinned self-signed servers, the Jellyfin URL rules and the quality settings stay in one place.

**What the platform does.** It only schedules `run_downloads`: a user-initiated data transfer job on Android, a background task elsewhere. It reports the network with `set_connectivity`.

**Shared files.** Files are named by a key made from the server ID, the item ID and the quality. The device registry (R2) counts how many profiles hold each file, but never which. A file goes when the count reaches 0. `collect_download_garbage` recounts from every profile's own list after a profile is removed or Remove all, so a crash between writing a file and recording it can't leak space.

**Offline.** Only saved songs and songs the player reports fully cached can play:
- the queue passes over the others, forward and back (`Queue::set_playable`);
- `play`, `play_next`, `add_to_up_next`, `smart_shuffle`, `jump_to` and `replay_from_history` refuse them with `NotAllowed`;
- smart picks get `only`.

**Streams and covers.** `stream_for` gives the saved file whether online or not. A downloaded album's cover is kept at 1200 px and `artwork` returns it.

## Alternatives considered
- **Each platform downloads with its own stack** (Media3 DownloadManager, URLSession, the desktop engine). That is three implementations of the queue, limits, sharing and offline rules, and it breaks the rule that behaviour lives in the core. Media3's cache format also can't be read by the core for "only downloaded songs". The Apple track may still need URLSession background transfers when the app is suspended. The core's state is designed so a platform could transfer a song itself and report it; that can be added without changing the model.
- **Media3's SimpleCache as the download store.** It is cache-shaped (LRU, no lists, no per-profile view), and the other platforms can't use it.
- **A foreign key from `download_songs` to `tracks`.** A song leaving the library would cascade the row away and lose its file key, leaking the file. Without it, reconciling sees the song gone and lets its file go.
- **Following a mix as it is replaced each morning.** That would delete yesterday's downloads unasked. Downloading a mix pins it instead, so it keeps its songs and its page.

## Consequences
- **Easier:**
  - one tested implementation for every platform (13 API tests on the recording, the queue's own tests, the store's);
  - Windows and Linux get downloads with no new code beyond scheduling;
  - offline behaviour is the same everywhere.
- **Harder:**
  - a download only runs while the platform gives the core time: a job on Android, background time on iOS;
  - very large libraries reconcile in one pass (cheap for dozens of collections; Liked songs with tens of thousands of songs is the heaviest case, and it runs only after likes, sync and runs).
- **To undo:** migration 8's tables can stay unused. Removing the feature means removing the calls (a CORE-API version note) and deleting `<data>/downloads`.
