# ADR core-0001: Product name and package identifiers

Date: 6 November 2026
Status: accepted (identifier prefix confirmed by the owner; the product name is still a placeholder)

## Context
"Encore" is a placeholder codename (docs/DECISIONS.md section 4). CLAUDE.md asks for the name to live in one place so a rename is a one-file change. The Kotlin bindings need a package name now, and app identifiers become permanent at the first store upload. The owner owns the domain `gcartee.uk` and asked to keep "Encore" for now.

## Decision
- The name people see is `encore_core::brand::APP_NAME` (`core/encore-core/src/brand.rs`). Every platform reads it through `app_name()` instead of typing it.
- Identifiers use the owner's domain reversed: the Kotlin bindings are `uk.gcartee.encore.core` (`core/encore-core/uniffi.toml`), the Kotlin token constants are `uk.gcartee.encore.tokens` (`tools/encore-tokens/src/lib.rs`, `KOTLIN_PACKAGE`). The Swift module is `EncoreCore`. App identifiers belong to the platform tracks; the natural choice is `uk.gcartee.encore`.
- Crate, module and package names that contain "encore" are internal identifiers, not the product name. They stay when the product is renamed.

## Alternatives considered
- A placeholder prefix such as `org.example`: it would need changing in every track before any store upload, so using the owner's real domain now saves a rename.
- Deriving the Kotlin package from the brand constant at build time: UniFFI reads the package from `uniffi.toml`, so this would need a build-script step for little gain.

## Consequences
- Renaming the product: edit `brand.rs` (and `meta.codename` in `design/tokens.json`, which only documents it). No binding changes.
- Changing the identifier prefix before the first store upload: edit the two places above, run `make bindings tokens`, and tell the platform tracks through a request. After a store upload the app identifier cannot change.
- Owning `gcartee.uk` is all the identifiers need; nothing is hosted on it. It could later host the privacy policy and account-deletion pages (plan section 19), which is the owner's choice.
