# ADR core-0019: What a song sounds like: YAMNet in plain Rust, plus a small hand-built profile

Date: 13 November 2026
Status: accepted (the owner approved the Phase 10 plan on 13 November 2026 and left the choices to me)

## Context
Phase 10 (plan sections 7, 7.1 and 21; core-0012) adds audio understanding to the smart-picks engine: "sounds like" for radio, mixes, Why this song and Browse by mood, and tempo and energy for flow. The engine is pure and deterministic, so it needs each song's sound as stored numbers, made once.

Constraints:
- **Licences.** Nothing linked into the apps may be stronger than MPL-2.0 (the core's own licence), and the Play Store build may be commercial, so no non-commercial model weights.
- **Devices.** Most users have no companion server. The server keeps no media-server token (core-0009). So the analysis has to run on a phone, on the original file, without making the user's media server transcode (the owner's Jellyfin runs on a weak PC).
- **No native runtime if it can be avoided.** ONNX Runtime and TFLite ship native libraries per ABI (and an Android 16 KB page-size question, PLAN-PRODUCT's not-checked list).

## What was tried
A labelled sample: 200 Creative Commons clips of 30 s from the Free Music Archive's small set, 25 for each of 8 genres (fetched with HTTP range requests; the clips are not stored in the repository). The measure is a stand-in for "sounds like": how often a clip's 5 nearest neighbours share its genre (chance is 12.5%), and the accuracy of a 5-nearest-neighbour genre vote. Genre labels are noisy and genre is not taste, so only the order of the rows means anything.

| Fingerprint | Same genre in the top 5 | 5-NN genre | Cost per 30 s clip |
| --- | --- | --- | --- |
| Random control | 12.2% | 9.5% | none |
| Hand-built texture (48 numbers, `encore-analysis`) | 22.4% | 31.5% | 0.18 s, this PC |
| Texture with tempo, loudness and energy added | 23.9% | 30.0% | |
| OpenL3-music, 512 numbers (CC BY 4.0 weights) | 37.2% (40.1% standardised) | 45.0% | 3.1 s, 4-core CI runner |
| YAMNet class scores, 521 numbers (MediaPipe's TFLite) | 34.5% (36.9%) | 44.5% | 0.11 s |
| **YAMNet 1024-number embedding, TFLite, 30 s** | **42.4%** | **55.0%** | 0.13 s |
| YAMNet 1024 numbers, TFLite, 10 s | 37.7% | 46.0% | 0.04 s |
| **YAMNet 1024 numbers, this crate's Rust network, 30 s** | **43.0%** | **53.5%** | 2.4 s, this PC, everything included |
| The Rust numbers projected to 256 / 128 / 64 | 43.0% / 39.7% / 36.3% | 53.5% / 52.0% / 42.0% | |

Adding the hand-built texture or tempo to YAMNet lowered precision (38.5% and 36.0%), so similarity uses the embedding alone.

## Decision
1. **The sound fingerprint is YAMNet** (Google, Apache-2.0): a MobileNet-v1 over a log-mel spectrogram, 3.7 million weights, trained on AudioSet. Its 1024-number embedding is the song's "sound"; its 521 class scores (AudioSet's genres, instruments, "singing", "sad music", "exciting music") are kept beside it and feed moods and titles (core-0020).
2. **It is run by plain Rust**, `core/encore-analysis/src/yamnet.rs` (about 400 lines), over weights converted from MediaPipe's TFLite file by `tools/sound-spike/export_yamnet.py`: int8 with a scale per output channel, 3.9 MB, turned into floats once at load. No runtime library, the same numbers on every platform, nothing for the NPU or the page-size question. It reproduces the original: 43.0% against 42.4% on the same clips, and a test compares it with the TFLite network's own outputs on a generated waveform.
3. **The model file ships with the app, not inside the core library**, so it exists once per install and not once per ABI. The platform gives the core its path with `set_sound_model`.
4. **A small hand-built profile stays** (`SoundProfile`: tempo, key, loudness (EBU R128), energy, brightness, dynamics): 58 bytes, from signal processing only, for what YAMNet doesn't give: flow between songs (tempo and energy "one step at a time") and cold-start mixes. It claims nothing it cannot measure: no happy or sad, no acoustic, no danceability from signal processing. Relative major and minor keys are treated as close, because pitch content alone often confuses them.
5. **One window of about 30 seconds per song, from 35% of the way in**, read with HTTP range requests on the original file (`RangeReader`, 256 KB blocks, a growing wait on a busy server). A seek jumps to the window, so about 1 MB comes over the network, the server does no transcoding, and the song is never downloaded whole. Songs the decoder cannot read (Symphonia has no Opus decoder) are marked unavailable with the reason; a song the server failed to serve is not marked and is tried again.
6. **Devices analyse; the server stores and shares.** This departs from PLAN-PRODUCT section 5 (the server reads audio) because of core-0009 and the many users with no server. A read-only music folder or API key for the server is a possible later addition, not part of Phase 10.
7. **The analysis is the person's choice.** It is off until they turn it on, shows what it will cost first, and runs only on an unmetered network while charging, pausing for battery saver, heat and a media server that keeps failing.

## Not chosen
- **OpenL3-music** (CC BY 4.0 weights, MIT code): 3 points behind on precision at 20 times the compute (3.1 s against 0.13 s per clip), and its larger network would make phones slower. Kept as the fallback if YAMNet's quality proves too low on real libraries.
- **Essentia / MTG models** (Discogs-EffNet, MusiCNN): CC BY-NC-SA, non-commercial. **MuQ**: weights CC BY-NC. **LAION-CLAP**: the music checkpoints' terms were not clear.
- **ONNX Runtime or TFLite on the device**: not needed once the network runs in Rust, and each brings a native library per ABI.
- **A trained mood classifier** (valence, acousticness): not built; YAMNet's own class scores give soft versions ("Sad music", "Acoustic guitar", "Singing") and nothing else is claimed.

## Consequences
- **Cost.** About 2.4 s of work per song on a weak desktop CPU, and about 1 MB over the network, so a library of 8,000 songs is about 5 hours of work and 8 GB, once, while charging on Wi-Fi, in order of usefulness (liked, played, downloaded, newest). The status page shows an estimate from the device's own measurements. A hop of a whole patch instead of half halves the work for a slightly rougher average, and 10 s instead of 30 s costs 5 points of precision.
- **Storage.** 1546 bytes per song in the profile's database (the 1024 numbers and 521 scores, one byte each, within 0.012 of the original), 58 bytes for the basic profile. For the engine in memory a fixed projection to 256 numbers keeps the quality (43.0%) at a quarter of the size.
- **Licences.** YAMNet is Apache-2.0: its licence and a notice travel with the model file (`core/encore-analysis/model/NOTICE`), and `THIRD_PARTY_LICENSES.md` lists the crates added (realfft and ebur128, MIT; Symphonia, MPL-2.0, which is the ceiling the plan allows). The CI scan passes on all three workspaces.
- **What is unproven.** Genre is not taste, the sample is 200 clips, and a library is not 8 genres of 30-second clips. The measure ranks the options; whether "sounds like" feels right is for the owner's own library at the first install. Every number here (the window, the hop, the 35% start, 256 dimensions) is a starting value.
- **Reproducing it.** `tools/sound-spike/` and the manual `sound-spike` workflow (fetch the clips, compare the models, write the reference outputs); `tools/sound-spike/export_yamnet.py` makes the model file.
