# ADR core-0020: Sound in the engine, and a ranker that learns how much to trust it

Date: 13 November 2026
Status: accepted

## Context
core-0019 gives every analysed song a sound. This decides how the smart-picks engine (core-0012) uses it, and how it learns, per person, how far to trust it. Phase 10's "done when" is that version 2 beats version 1's early-skip rate in an offline replay.

## Decision
1. **A song's sound reaches the engine as a `Sound`** on `Song` (optional, and absent from the JSON when absent): a fixed ±1 random projection of the 1024-number embedding to 128 integers, plus tempo, energy, brightness, dynamics and key. 128 numbers keep the quality (43.6% against 45.4% for all 1024 on the test clips) at 256 bytes a song. The projection is integer arithmetic on a seeded hash, so a phone, the server and a desktop make the same numbers (`encore_analysis::project_encoded`, pinned by a test).
2. **Similarity is exact.** The catalog centres every projection by the library's mean (a measured gain, 43.2% to 45.4%) and compares songs by cosine in 32-bit integers, with the square roots made once. `Model::similarity` is the only definition of "sounds like": 0.45 sound + 0.25 played together + 0.10 same artist + 0.10 genre + 0.10 era when both songs are analysed, version 1's weights otherwise. These are starting values.
3. **Where it is used.** Candidates (the 60 songs that sound most like each seed; the 100 never-played songs that best fit the person's taste), the session fit and diversity (through similarity), a small flow cost for jumps in tempo and energy from the song before, and the "sounds like" songs of Why this song.
4. **A taste in sound.** The model keeps a direction in sound space, built from every analysed song the person has a verdict on (finished and liked pull toward it, skips push away; each song a unit vector), once it has learned from eight. A never-played song's liking leans on how well it fits (a quarter of a full lean, never above what a song finished once or twice earns). A guess never outranks what the person has shown.
5. **The learned ranker** (`encore-recs/src/learn.rs`): gradient-boosted trees, depth 3, 80 rounds, in plain deterministic Rust (no randomness, ties by position), about 60 KB.
   - It predicts "this app pick is not skipped in its first 30 seconds", the plan's success measure.
   - It starts from the hand-tuned score put on a log-odds scale (a one-variable regression), and the trees only learn corrections, and only from the signals the hand-tuned score cannot see (taste fit, whether it is known, likeness to the song before, the jump from it). Letting them split on the hand-tuned signals too made picks worse where picks live.
   - **Training examples are made the way picks are:** the history is replayed from 16 points, and each play is described by a model built from only what came before its chunk, so nothing leaks from the play being predicted. Only plays the app chose count.
   - **The gate:** it trains on the older three quarters and is used only if it beats the hand-tuned score by a point of AUC on the newest quarter, on the plays the person likes most (where picks come from). Under 300 labelled plays it never trains. A corrupt or too-new ranker is ignored and the hand-tuned score runs.
   - Not ONNX or Core ML (core-0019's reasoning): ranking takes microseconds, and this keeps the engine identical on every platform.
6. **The server trains the same model** from the synced log nightly and serves it; a device with no server trains it while charging and idle. (The wiring is the next work package.)

## What the replay shows (docs/core/REPLAY-RESULTS.md)
Six generated listeners, 480 sessions each, version 1 against version 2 on the same random draws. The generator gives sound a real part in taste; three listeners have clean genre tags and three have half their artists mis-tagged.

| | Early skips, v1 | v2 | Change |
| --- | --- | --- | --- |
| mean of six | 20.7% | 19.1% | lower on 5 of 6 |
| light, messy tags | 27.7% | 21.4% | 23% fewer |
| many skips, messy tags | 19.1% | 16.2% | 15% fewer |
| heavy (both) and light | 5% to 13% | 0.3 to 0.8 points lower | |
| many skips, clean tags | 52.7% | 53.8% | 1.1 points worse (about one and a half standard errors) |

The check: version 2 has a lower mean, is lower on at least four of six, and is never more than two points worse on any.

**Honest limits.** The listeners and the sounds are generated. Where tags are perfect sound adds little or costs a point; where they are messy it helps a good deal, and real libraries' tags are usually messy, which is why sound leads a little. The learned ranker stays off in the replay: its history is plain shuffle, so it holds none of the engine's own picks, which is what it learns from in real use. Its learning and its gate are tested on constructed data instead. Whether it helps a real person is read from their How suggestions are doing page.

**What the experiments taught** (kept so they are not repeated):
- A sound-based guess for never-played songs at full strength cost the clean-tag listener two points; a quarter strength does not.
- Taking out the never-played taste pool cost the short-history listeners three to five points.
- A ranker trained on every signal had better AUC on held-back plays but worse picks: it was judged across all songs, while picks come from the top of the range. Hence the pick-like gate and the sound-only splits.
- Under about a point is noise at this size (4,800 picks a listener); the first replay of 1,200 picks was too coarse to compare versions.

## Consequences
- **Budgets** (20,010 songs, 20,160 plays, every song analysed): model build 0.08 s, 6.6 MB, the next-song pick 43 ms against a 50 ms budget on a CI machine; on a phone the margin is thin and is the first thing to watch.
- `Song`, `Model` and `Catalog` gained fields; an older device's catalog without `sound` still loads.
- Every number above is a starting value to tune on real histories.
