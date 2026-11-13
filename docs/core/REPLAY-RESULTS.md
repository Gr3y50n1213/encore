# Smart picks: offline replay results

Made by `tools/recs-replay` (docs/adr/core-0014 and core-0020). Each listener is generated with hidden tastes: genres, favourite and disliked artists, a time of day, and (Phase 10) a taste for a direction in a hidden sound space that no tag says. The listeners with "messy tags" have half their artists mis-tagged, as a real library's tags often are. The engine sees only their history. In 480 sessions after it, the listener picks a song and then autoplay picks 10 with the version 1 engine (it does not know what songs sound like), with the version 2 engine (the same engine given every song's sound, and the learned ranker when the listener's history earns one), or plain shuffle picks 10, all scored on the same random draws.

**Checks.** Version 1's early-skip rate is at most 60% of shuffle's on every listener. Version 2 beats version 1 overall: a lower mean early-skip rate across the listeners, lower on at least four of the six, and never more than two points worse on any.

**What this proves and what it does not.** The listeners and the songs' sounds are generated, and the generator gives sound a real part in taste, so this shows that the engine uses sound when it is there and that the wiring works, not how much it will help on a real library. Differences under about a point are noise (about 4,800 picks per listener). Where tags are perfect, sound adds little or costs a point; where they are messy it helps a good deal. The learned ranker never switches on here: the generated history is plain shuffle, so it holds none of the engine's own picks (what the ranker learns from in real use), and its gate (it must beat the hand-tuned score on the person's newest plays) refuses it. Its learning and its gate are tested on their own (`encore-recs` `learn` tests). The real measure is the person's own How suggestions are doing page after the library is analysed.

| Listener | Library | Plays | Early skips: v1 | Early skips: v2 | v2 without the ranker | Early skips: shuffle | v1 / shuffle | v2 / v1 | Finished: v1 | Finished: v2 | Finished: shuffle | Learned ranker |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| light | 3600 | 200 | 13.4% | 12.9% | 12.9% | 43.5% | 0.31 | 0.96 | 81.0% | 81.6% | 49.1% | off (0 labelled plays: too few, or no better than the hand-tuned score) |
| heavy | 9000 | 20440 | 6.4% | 5.6% | 5.6% | 40.8% | 0.16 | 0.88 | 90.1% | 91.0% | 51.5% | off (6555 labelled plays: too few, or no better than the hand-tuned score) |
| many skips | 6000 | 3240 | 52.7% | 53.8% | 53.8% | 88.0% | 0.60 | 1.02 | 38.0% | 36.9% | 7.3% | off (1061 labelled plays: too few, or no better than the hand-tuned score) |
| light, messy tags | 3600 | 200 | 27.7% | 21.4% | 21.4% | 49.7% | 0.56 | 0.77 | 65.3% | 71.8% | 42.2% | off (0 labelled plays: too few, or no better than the hand-tuned score) |
| heavy, messy tags | 9000 | 20440 | 5.1% | 4.8% | 4.8% | 37.8% | 0.14 | 0.94 | 91.7% | 91.9% | 54.1% | off (6563 labelled plays: too few, or no better than the hand-tuned score) |
| many skips, messy tags | 6000 | 3240 | 19.1% | 16.2% | 16.2% | 74.0% | 0.26 | 0.85 | 74.0% | 77.5% | 20.4% | off (1073 labelled plays: too few, or no better than the hand-tuned score) |

Across the listeners: version 1 20.7%, version 2 19.1% (mean early-skip rate), version 2 lower on 5 of 6.

## Budgets (smartPicks.budgets; CI is not a phone)

On 20010 songs and 20160 plays, this run:

- model and five mixes: 0.08 s + 0.12 s, within the 20 s budget;
- memory at the peak of the build: 6.9 MB, within the 150 MB budget;
- the next-song pick (autoplay, 10 songs, median of 7): 25.2 ms, within the 50 ms budget.

The same library with every song analysed (Phase 10):

- model build: 0.08 s, within the 20 s budget;
- memory at the peak of the build: 6.6 MB, within the 150 MB budget;
- the next-song pick: 43.1 ms, within the 50 ms budget.
