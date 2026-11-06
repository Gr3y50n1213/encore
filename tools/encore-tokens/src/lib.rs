//! Design-token codegen: `design/tokens.json` to Swift, Kotlin and Dart constants
//! (plan section 3; `design/README.md`). One source of truth, so no platform types
//! a colour, size or duration by hand.

pub mod emit;
pub mod model;
pub mod names;

/// The Kotlin package for the token constants: the owner's domain, gcartee.uk,
/// reversed (docs/adr/core-0001-names-and-identifiers.md).
pub const KOTLIN_PACKAGE: &str = "uk.gcartee.encore.tokens";

/// The generated files, as (path inside `core/bindings/`, contents), in a fixed order.
/// Dart goes inside the core's Dart package, so the Flutter app imports bindings and
/// tokens from one package (`package:encore_core/encore_tokens.dart`).
pub fn generate(tokens_json: &str) -> model::Result<Vec<(&'static str, String)>> {
    let root = model::parse(tokens_json)?;
    Ok(vec![
        ("tokens/EncoreTokens.swift", emit::swift(&root)),
        (
            "tokens/EncoreTokens.kt",
            emit::kotlin(&root, KOTLIN_PACKAGE),
        ),
        ("dart/lib/encore_tokens.dart", emit::dart(&root)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_real_tokens_file_generates_all_three_languages() {
        let files = generate(include_str!("../../../design/tokens.json")).unwrap();
        let names: Vec<_> = files.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            [
                "tokens/EncoreTokens.swift",
                "tokens/EncoreTokens.kt",
                "dart/lib/encore_tokens.dart"
            ]
        );

        let (_, swift) = &files[0];
        assert!(swift.contains("public enum Breakpoints {"));
        assert!(swift.contains("public static let railWidth: Int = 80"));
        assert!(swift.contains("public static let `default`: String = \"balanced\""));

        let (_, kotlin) = &files[1];
        assert!(kotlin.contains("package uk.gcartee.encore.tokens"));
        assert!(kotlin.contains("const val reduceMotionCrossfadeMs: Int = 120"));

        let (_, dart) = &files[2];
        assert!(dart.contains(
            "static const EncoreTokensBreakpoints breakpoints = EncoreTokensBreakpoints._();"
        ));
        assert!(dart.contains("final String defaultValue = 'balanced';"));
    }

    #[test]
    fn output_is_deterministic() {
        let json = include_str!("../../../design/tokens.json");
        assert_eq!(generate(json).unwrap(), generate(json).unwrap());
    }

    #[test]
    fn output_uses_unix_line_endings_and_ends_with_one_newline() {
        for (name, text) in generate(include_str!("../../../design/tokens.json")).unwrap() {
            assert!(!text.contains('\r'), "{name} has a carriage return");
            assert!(
                text.ends_with("}\n") && !text.ends_with("\n\n"),
                "{name} must end with one newline"
            );
        }
    }
}
