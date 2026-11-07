# Jellyfin fixtures: Phase 5

`recorded-phase5.json` holds the shapes of what smart picks reads from a real Jellyfin 12.0.0
server (`tools/fixture-recorder --phase5`, read-only): one page of Jellyfin's own user data, the
plugin list, the Playback Reporting query when the plugin is there, and one similar-items reply;
three rows each (the plugin list whole, every name scrambled), with play counts and dates replaced, anonymized like the other recordings.
The owner's history itself stays on the owner's PC (`tools/target/private-history`, git-ignored).

- Library: 184 songs; 138 of them played in some app, 686 plays in all (Jellyfin's own counts).
- Plugins: 17 installed; Playback Reporting is not.
- Similar items: 50 for 5 songs.
