# ADR core-0011: Profiles on one device: registry, PIN, Guest

Date: 7 November 2026
Status: accepted

## Context
Plan section 6 and board 14:
- each person gets a fully separate profile;
- switching takes two taps, and either keeps playing or stops;
- locked profiles ask for biometrics, then a 4 to 6 digit PIN; five wrong tries lock it for 30 s, then longer; after biometrics change, the PIN is needed once;
- removing a profile from the device is not deleting it;
- Guest records nothing and ends after 4 hours idle.

"Two people share an iPad with fully separate history" is Phase 4's "done when". The PIN is a screen lock, not encryption.

## Decision
- **Each profile stays a folder with its own database** (as before). The switcher's facts, which must be known before a profile opens, live in a **device registry**, `device.sqlite` in the data folder:
  - order and display names;
  - avatars (the media server's user image address, picked up at library sync; the recording showed user images need no token);
  - PIN hashes and lock-out state;
  - the last active profile and the Guest session.

  It holds nothing from anyone's listening, and no token.
- **PINs.**
  - Hashed with a salt (160 random bits) and 100,000 rounds of SHA-256 (sha2, already in the build), compared without early exit.
  - With 10^6 PINs nothing makes a stolen hash safe, so the guard is the lock-out: `profiles` tokens, 5 tries, then 30 s doubling to an hour, and the right PIN resets it.
  - `unlock_profile(None)` means the app's biometric check passed. The core can't check biometrics; it can refuse that path after `biometrics_changed`.
  - An unlock is used by the next opening, so one left by a sign-in can't open a profile after a PIN is set.
- **Switching** saves the queue (`saved_queue`, migration 6) and loads the other profile's queue, paused. With `keep_playing` the queue moves over, and the player reports `playback_started` again so the rest of the song counts for the new profile. Unsent sync goes first (core-0010).
- **Guest** is a mode of the open profile, not a second sign-in, because the music still comes from that profile's media server:
  - nothing is recorded, recommended or synced;
  - likes and playlists are refused;
  - the queue starts empty, and the profile's own queue is kept aside and restored at the end.

  Guest ends with `end_guest`, after `profiles.guestIdleHours` without playing (checked on each play call), or when a profile opens, so a guest's queue never meets the next listener.
- **Removing and deleting.** `remove_profile_from_device` deletes the local folder only. `delete_profile_everywhere` (board 19) needs the profile's name typed, deletes it on the companion server first and refuses while that server can't be reached (nothing is deleted), then deletes the local folder.
- **"Sign in again".** When a library sync gets `SessionExpired`, the profile records it, and `profiles()` reports `needs_sign_in` until a sign-in succeeds.

## Consequences
- The registry has its own migrations (version 1).
- A deletion queued while the companion is offline ("Waiting to finish on server", board 19) is not built: the core refuses instead, and says so. Recorded in the Phase 4 review.
- The avatar's size is unverified: the recording got a 2.7 MB original for a 256 px request. The address now asks for the size every way Jellyfin takes it.
