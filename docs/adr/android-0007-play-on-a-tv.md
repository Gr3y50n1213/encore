# ADR android-0007: Play on a TV

- **Status:** accepted
- **Date:** 27 November 2026
- **Owner's decision:** mock-up 50 approved in full on 27 November 2026, including Google's Cast library in the GitHub build.
- **Plan:** ROADMAP "Play on a TV"; `docs/plan-tv-and-watches.md`

## Context

The owner wants music sent from the phone to a TV the way Spotify Connect or Chromecast does. The TV plays the music and shows its own Encore screen, and the phone stays the remote. The roadmap's privacy rule is that the TV gets a short-lived pass that can only play, never the sign-in.

Encore's stream addresses carry the media server's key: Jellyfin and Emby use `api_key`, Plex uses `X-Plex-Token`, and Subsonic uses its salted token. Handing such an address to a TV would give the TV, and anything on it, lasting access to the whole library. Some TVs also log or cache the addresses they play.

## Decision

**The phone relays the music.** While music plays on a TV, Encore runs a small HTTP server on the phone, bound to its Wi-Fi address only.

- Every session gets a new random 128-bit pass. Every address the TV gets has the form `http://<phone>:<port>/<pass>/song/<n>` or `/<pass>/art/<n>`.
- The pass works only for the songs of the queue it was made for.
- The pass ends when the music comes back to the phone, stops, or is idle for 10 minutes, and whenever Encore's playback service stops.
- The phone fetches each song itself, from the media server with the key, or from a download or a file on the phone. It hands the bytes on with Range support, so the TV can seek.
- The media server's key, the profile and the history never leave the phone.

**Which TVs, in this order:**

1. **Google Cast.** Media3's `media3-cast` (`CastPlayer`, Apache-2.0) on Google's Cast framework (`play-services-cast-framework`, closed, in Google Play services). It starts with Google's Default Media Receiver, which needs no registration and plays the music with the cover and title, without Encore's screen. Encore's own receiver page (the Encore TV look of mock-up 45, with synced lyrics) follows once the owner has registered a Cast app ID. That costs a one-time US$5, and only the owner can pay it. The receiver page is served over https by the companion server. A build without Play services (an F-Droid build, if one ever comes) leaves Cast out.
2. **The Encore TV app** (Android TV, Google TV, projectors). It is listed under "Your devices" through the companion server, and handoff (Phase 7) already moves the music to it. It plays from its own sign-in, as any Encore device does.
3. **Roku.** The Roku is found on the network through SSDP and controlled through its External Control Protocol (ECP). An Encore channel, written in BrightScript and SceneGraph, is installed through the Roku's developer mode. Encore starts the channel with the relay's address, and the channel plays from the relay and shows what is playing.
4. **DLNA renderers** ("Music only"). They are found through SSDP (`urn:schemas-upnp-org:device:MediaRenderer:1`) and driven through `AVTransport` (`SetAVTransportURI`, `Play`, `Pause`, `Seek`) and `RenderingControl` for volume. Encore writes the SOAP by hand, with no library.

**On the phone,** the TV's playback is a Media3 player (`CastPlayer`, or Encore's own `SimpleBasePlayer` for Roku and DLNA). Encore's media session switches to it, so the lock screen, the notification, Bluetooth buttons and the volume keys drive the TV. Now Playing shows "Playing on <TV>" (mock-up 50, screen 2).

## Consequences

- The phone must stay on the same Wi-Fi while a TV plays. Its playback service keeps running, as it already does while playing.
- Battery: the phone passes on the bytes of each song once. That is about the same as playing them itself.
- The pass travels over the home network in plain http, because TVs refuse self-signed certificates. It is useless outside the session, and the media key is never in it.
- Google's Cast library would report its own usage to Google through Play services' data-transport "cct" backend. Encore removes that backend's registration from the app's manifest (its classes stay, because Cast refers to them), so the transport finds no backend and drops those events on the phone. Cast starts only when Play on is opened, never when the app starts. To check on a real Cast device: that casting works without the backend, and that the phone sends nothing to Google's logging hosts.
- Cast needs Google Play services. The phone app already needs them for Android Auto, so nothing changes for the GitHub build.
- Not verifiable without the hardware: Cast needs a Chromecast or Google TV, and Roku needs the owner's Roku in developer mode. Discovery and the relay are tested in CI and on the phones; the rest is checked with the owner.
- Found on the owner's network (27 November 2026): the router doesn't pass multicast between the tablet's 5 GHz Wi-Fi and the Roku, while a search sent straight to the Roku is answered. When the multicast search finds nothing, Encore sends the same search to each address of the phone's /24 network once, only while Play on is looking. The owner's Roku also refuses `query/media-player` (403, "Control by mobile apps" set to Limited); then Encore keeps its own clock of what it told the channel.
