# ADR core-0012: The smart-picks engine, version 1

Date: 7 November 2026
Status: accepted

## Context
Plan sections 7, 7.1 and 21, and boards 15, 23, 37 and 38. One engine chooses every song the app chooses (autoplay, radio, mixes, Smart shuffle) with one set of rules, so a skip teaches every surface. Version 1 has no audio features: audio analysis comes with the companion server in Phase 9, and the trained ranker in Phase 10. The mixes must look the same on every device of a profile. The owner chose that they are built on the companion server, and by a device when there is none (core-0013).

## Decision
- **`encore-recs` is pure and deterministic.**
  - No I/O and no clock: the same history, library, time and seed give the same picks, byte for byte, on a phone, a desktop and the server.
  - The platform's maths library can differ by one bit between systems, which can reorder nearly equal scores. So the engine uses its own `exp` built from exact operations, its own PRNG (SplitMix64) and its own Beta draws (order statistics of uniforms), and breaks every tie by index.
- **Signals** are section 21's weights from the `smartPicks` tokens, in `Params`:
  - liked +3; added to a playlist +1.5; finished +1 (a little more when the app chose it); past 80% +0.5; mid skip −0.5; early skip of an app pick −1.5;
  - a skip of a song the user picked counts only after 3 skips in 14 days;
  - negatives fade with a 30-day half-life;
  - nothing before the learning epoch counts (Reset suggestions);
  - the media server's play counts from other apps are a prior.

  Plays that never count (interruptions, errors, accidents, too soon, ended by the system) only mark a song as heard. Guest, Private and Learn-from-my-listening-off write nothing, as before.
- **Version 1's "sounds like"** is played together (a co-listen graph from 30-minute sessions), same artist, genre overlap and era.
- **Ranking and shaping.**
  - Candidates come from the co-listened songs, the same artists, a sample of the same genres, rediscovery (loved, unheard for 60 days) and recent additions.
  - Hard filters: Not interested (by scope and reason), heard in the last 24 hours unless loved, queued, offline-only, explicit when hidden.
  - The score is section 21's hand-tuned one.
  - Exploration is Thompson-sampled: never the first slot, never two in a row, only after a likely finish.
  - Diversity: maximal marginal relevance at 0.7, and no artist twice within 3 songs.
  - In-session adjustment: 3 early skips in a row stop exploration and push the session away; a like pulls it toward that song.
- **Reasons** are data: the fact and its values ("finished X 9 of 10", "played together with X", "evening"), at most two. The app words them; the engine never guesses mood, health or place.
- **Mixes (section 7.1 version 1).**
  - Moods are clusters of enjoyed songs by when they are played (time of day, weekday or weekend), the mood family of their genres, and their era.
  - Slots: the two strongest moods; after 30 days, also time of day, drift and stretch (30% new).
  - Each mix is 25 to 40 songs, at most 2 per artist and 4 per album, most played at most 40%, overlap at most 20%, flow order by era, and at most 30% change per day.
  - Titles use a fixed vocabulary, with the section 7.1 rules enforced and tested on every title it can make.
  - Cold start follows section 21's table: a Starter mix at 0 plays, up to three family mixes under 50.
- **Small libraries.** At 2 songs per artist, a library with few artists can't fill 25 songs. A mix is then as long as the caps allow (at least 10). The caps themselves are never relaxed.
- **In the core.** The model is rebuilt only when a fingerprint of its inputs changes, or every 10 minutes. Autoplay refills itself in the background when a song starts. The next-song decision reads what was filled.

## Consequences
- Measured in CI (docs/core/REPLAY-RESULTS.md): autoplay's early-skip rate is 18% to 46% of shuffle's on three generated listeners.
- Budgets on 20,000 songs and 20,000 plays: the model and five mixes take 0.14 s and 7 MB at the peak; the next-song pick takes 20 ms.
- Every number is a starting value, to be tuned by replay on real histories.
- Phase 9 adds audio features to the same model (tempo and energy flow, real "sounds like"). Phase 10 replaces the hand-tuned score with a trained one that has the same inputs and outputs.
