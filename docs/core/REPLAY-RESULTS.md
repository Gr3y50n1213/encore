# Smart picks: offline replay results

Made by `tools/recs-replay` (docs/adr/core-0014). Each listener is generated with hidden tastes; the engine sees only their history. In 120 sessions after it, the listener picks a song and then autoplay picks 10, or plain shuffle picks 10, scored on the same random draws. The check: autoplay's early-skip rate is at most 60% of shuffle's on every listener.

| Listener | Library | Plays | Early skips: autoplay | Early skips: shuffle | Autoplay / shuffle | Finished: autoplay | Finished: shuffle |
| --- | --- | --- | --- | --- | --- | --- | --- |
| light | 3600 | 200 | 20.8% | 60.6% | 0.34 | 74.1% | 31.8% |
| heavy | 9000 | 20440 | 9.0% | 49.7% | 0.18 | 85.2% | 42.1% |
| many skips | 6000 | 3240 | 37.1% | 81.5% | 0.46 | 53.0% | 13.3% |

## Budgets (smartPicks.budgets; CI is not a phone)

On 20010 songs and 20160 plays, this run:

- model and five mixes: 0.05 s + 0.09 s, within the 20 s budget;
- memory at the peak of the build: 6.7 MB, within the 150 MB budget;
- the next-song pick (autoplay, 10 songs, median of 7): 20.2 ms, within the 50 ms budget.
