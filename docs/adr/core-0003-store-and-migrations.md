# ADR core-0003: One SQLite file per profile, migrated by `user_version`

Date: 6 November 2026
Status: accepted

## Context
Plan section 6 gives every profile its own database: "A profile is a folder: its own SQLite database". Section 4 says to mirror the library into it, "so browsing and search are instant and work offline", and section 5 lists the play-event fields that "must exist from the first build". The store must survive app updates without losing history, which cannot be rebuilt.

## Decision
- `encore-store` uses rusqlite 0.41 with SQLite bundled (public domain), so every platform gets the same SQLite version with FTS5 and the trigram tokenizer.
- One file per profile: `<data_dir>/profiles/<profile_id>/library.sqlite`. WAL journal, foreign keys on.
- Migrations are a list of SQL scripts in `encore-store/src/schema.rs`. `PRAGMA user_version` holds how many have run. Each runs once, in order, in a transaction. A released migration is never edited; a change is a new script at the end. A database from a newer app version is refused (`NewerSchema`), not damaged.
- The mirror, the play log, lyric jumps, recent searches and settings share the file. Play events copy the title, artist and album and point at songs by source and item ID, without a foreign key, so history survives a song leaving the library.
- Search uses an FTS5 table with the trigram tokenizer (`remove_diacritics 1`), keyed by `item id × 4 + kind`, plus an `fts5vocab` table so the typo pass starts from rare trigrams. The budget test keeps it under 50 ms on 50,000 songs.
- Tokens are never stored here (plan section 12).

## Alternatives considered
- sqlx: async and compile-checked queries, but it needs a database at build time or an offline cache, and it adds an async runtime to every query. The store is local and its queries take milliseconds.
- refinery or rusqlite_migration for migrations: a dependency for what `user_version` and a list already do.
- A shared database with a profile column: one bug could leak one person's history into another's, and deleting a profile could not simply delete a folder.

## Consequences
- Each new schema change is a new migration plus a forward-migration test.
- Backing up a profile is copying a folder; deleting one is removing it.
- Undoing this would mean exporting and reimporting every profile, so the format should stay.
