# ADR core-0014: Measuring smart picks by offline replay

Date: 7 November 2026
Status: accepted

## Context
Phase 5's "done when" is *autoplay's early-skip rate is clearly lower than shuffle on your own history*. Section 24 asks for offline replay on at least three test histories (light, heavy, many skips) beating plain shuffle, with the numbers saved in CI. An offline replay can't know how the user would have reacted to a song they never heard: a counterfactual. No app has logged plays yet. The owner chose Jellyfin's own data for "your own history", and asked that the history never be committed.

## Decision
- **Generated listeners in CI** (`tools/recs-replay`). Each listener has hidden tastes:
  - genre taste;
  - favourite and disliked artists;
  - a genre's time of day;
  - per-song noise.

  The engine sees only the history generated from those tastes, so every pick can be scored, including songs never heard. In 120 sessions after the history, the listener picks a song, and then autoplay picks 10, or shuffle picks 10. Both are scored on the same random draws.
- **CI check.** `--check` fails unless autoplay's early-skip rate is at most 60% of shuffle's on every listener, and the budgets hold: model and five mixes under 20 s and 150 MB, and the next-song pick under 50 ms, on 20,000 songs.
- **The owner's own history, on the owner's PC only** (`--private`, from `fixture-recorder --phase5`). Names are hashed with a key that is not kept, and the history is never committed; only the summary numbers go in the review.
  - **With Playback Reporting's per-play log:** a play is finished at 80% of its length or more, and an early skip under 30 s. Train on the first 80% of plays. In each later session, seed autoplay with its first song. Then compare the early-skip rate of autoplay's picks the owner actually played in that period against shuffle's. Also run a ranking check: the chance a finished song outscores an early skip (0.5 is chance).

    This is the counterfactual problem solved the only honest way: it counts only songs with known outcomes, and says how many.
  - **With play counts only:** hold out the most recently played fifth, and measure how often autoplay seeded by one of them picks another, against shuffle. It is a hit rate, not a skip rate, and is labelled so.

## Consequences
- Generated tastes are not real ones, so the CI numbers prove the engine learns what is there. They don't prove the starting weights are right; real histories tune those (DECISIONS, Unverified).
- The owner's numbers are only as strong as Jellyfin's data allows; the review says which check ran and how many plays it rests on.
