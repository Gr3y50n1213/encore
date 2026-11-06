//! Turns tokens.json keys into identifiers that are legal in Swift, Kotlin and Dart.

/// `railWidth` stays `railWidth`; `rail-width` and `rail width` become `railWidth`;
/// a leading digit gets an `n` in front (`3d` becomes `n3d`).
pub fn camel(key: &str) -> String {
    let mut out = String::new();
    for (i, part) in key
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .enumerate()
    {
        let mut chars = part.chars();
        let first = chars.next().expect("parts are not empty");
        if i == 0 {
            out.push(first.to_ascii_lowercase());
        } else {
            out.push(first.to_ascii_uppercase());
        }
        out.extend(chars);
    }
    if out.is_empty() {
        out.push_str("unnamed");
    }
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, 'n');
    }
    out
}

/// `androidToggleShape` becomes `AndroidToggleShape`.
pub fn pascal(key: &str) -> String {
    let camel = camel(key);
    let mut chars = camel.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

const SWIFT_RESERVED: &[&str] = &[
    "Any",
    "Self",
    "Type",
    "as",
    "associatedtype",
    "break",
    "case",
    "catch",
    "class",
    "continue",
    "default",
    "defer",
    "deinit",
    "do",
    "else",
    "enum",
    "extension",
    "fallthrough",
    "false",
    "fileprivate",
    "for",
    "func",
    "guard",
    "if",
    "import",
    "in",
    "init",
    "inout",
    "internal",
    "is",
    "let",
    "nil",
    "open",
    "operator",
    "precedencegroup",
    "private",
    "protocol",
    "public",
    "repeat",
    "rethrows",
    "return",
    "self",
    "static",
    "struct",
    "subscript",
    "super",
    "switch",
    "throw",
    "throws",
    "true",
    "try",
    "typealias",
    "var",
    "where",
    "while",
];

const KOTLIN_RESERVED: &[&str] = &[
    "as",
    "break",
    "class",
    "continue",
    "do",
    "else",
    "false",
    "for",
    "fun",
    "if",
    "in",
    "interface",
    "is",
    "null",
    "object",
    "package",
    "return",
    "super",
    "this",
    "throw",
    "true",
    "try",
    "typealias",
    "typeof",
    "val",
    "var",
    "when",
    "while",
];

const DART_RESERVED: &[&str] = &[
    "assert", "await", "break", "case", "catch", "class", "const", "continue", "default", "do",
    "else", "enum", "extends", "false", "final", "finally", "for", "if", "in", "is", "new", "null",
    "rethrow", "return", "super", "switch", "this", "throw", "true", "try", "var", "void", "while",
    "with", "yield",
];

/// A Swift member name; reserved words are wrapped in backticks.
pub fn swift_member(key: &str) -> String {
    let name = camel(key);
    if SWIFT_RESERVED.contains(&name.as_str()) {
        format!("`{name}`")
    } else {
        name
    }
}

/// Names Swift refuses for a nested type unless they are wrapped in backticks
/// (`Type` would clash with the `Foo.Type` metatype expression).
const SWIFT_RESERVED_TYPES: &[&str] = &["Any", "Protocol", "Self", "Type"];

/// A Swift nested type name; `type` becomes `` `Type` ``, used as `` Recap.`Type` ``.
pub fn swift_type_name(key: &str) -> String {
    let name = pascal(key);
    if SWIFT_RESERVED_TYPES.contains(&name.as_str()) {
        format!("`{name}`")
    } else {
        name
    }
}

/// Types the generated Swift, Kotlin and Dart refer to by name. A group with one of
/// these names would shadow it inside its own body (`enum String` would turn every
/// `String` constant in it into a reference to itself), so the model rejects it.
pub const BUILTIN_TYPES: &[&str] = &[
    "Bool",
    "Boolean",
    "Double",
    "Equatable",
    "Int",
    "List",
    "Long",
    "Sendable",
    "String",
];

/// A Kotlin member name; hard keywords are wrapped in backticks.
pub fn kotlin_member(key: &str) -> String {
    let name = camel(key);
    if KOTLIN_RESERVED.contains(&name.as_str()) {
        format!("`{name}`")
    } else {
        name
    }
}

/// A Dart member name. Dart has no escape for reserved words, so they get a
/// `Value` suffix (`default` becomes `defaultValue`).
pub fn dart_member(key: &str) -> String {
    let name = camel(key);
    if DART_RESERVED.contains(&name.as_str()) {
        format!("{name}Value")
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camel_keeps_camel_case() {
        assert_eq!(camel("railWidth"), "railWidth");
        assert_eq!(camel("ios"), "ios");
    }

    #[test]
    fn camel_joins_separated_words() {
        assert_eq!(camel("rail-width"), "railWidth");
        assert_eq!(camel("rail width"), "railWidth");
        assert_eq!(camel("Rail_width"), "railWidth");
    }

    #[test]
    fn camel_guards_leading_digits_and_empty_keys() {
        assert_eq!(camel("3d"), "n3d");
        assert_eq!(camel("--"), "unnamed");
    }

    #[test]
    fn pascal_capitalises() {
        assert_eq!(pascal("androidToggleShape"), "AndroidToggleShape");
        assert_eq!(pascal("ios"), "Ios");
    }

    #[test]
    fn reserved_words_are_escaped_per_language() {
        assert_eq!(swift_member("default"), "`default`");
        assert_eq!(kotlin_member("default"), "default");
        assert_eq!(kotlin_member("object"), "`object`");
        assert_eq!(dart_member("default"), "defaultValue");
        assert_eq!(dart_member("min"), "min");
    }

    #[test]
    fn swift_type_names_that_clash_get_backticks() {
        assert_eq!(swift_type_name("type"), "`Type`");
        assert_eq!(swift_type_name("self"), "`Self`");
        assert_eq!(swift_type_name("types"), "Types");
        assert_eq!(swift_type_name("recap"), "Recap");
    }
}
