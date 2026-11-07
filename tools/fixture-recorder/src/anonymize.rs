//! Turns a recording from the owner's real library into one that is safe to publish.
//!
//! Names, titles, paths and IDs are replaced with consistent stand-ins: the same
//! input always gives the same output within one recording, so links between items
//! survive (an album's ID in a song still points at the album). Stand-ins keep the
//! length and the kind of each character (upper case, lower case, digit, script
//! block), so odd metadata stays odd: a long Japanese title stays a long string of
//! CJK characters, a right-to-left name stays right to left. The key is random per
//! run and never written down, so the mapping cannot be reversed.
//!
//! Removed outright: file paths, BlurHash previews (a tiny picture of the artwork),
//! and anything that looks like a token.

use std::collections::{BTreeSet, HashMap};

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// The only keys whose string values are kept as they are: item kinds, formats,
/// dates and versions, which say nothing about whose library this is. Every other
/// string is replaced: an ID-shaped one keeps its hex shape, anything else gets a
/// same-shape stand-in. Replacing by default means a field Jellyfin adds later is
/// anonymized too.
const SAFE_KEYS: &[&str] = &[
    "Type",
    "MediaType",
    "CollectionType",
    "LocationType",
    "Container",
    "Version",
    "ProductName",
    "DateCreated",
    "DateLastSaved",
    "PremiereDate",
    "DateAdded",
    "DateLastMediaAdded",
    "EndDate",
    "PlayAccess",
    "ImageOrientation",
    "ExtraType",
    "VideoType",
    "IsoType",
    "ChannelType",
];

/// Query parameters kept as they are; every other value is replaced. These are
/// Jellyfin's own vocabulary and numbers (a filter name, an image format, a size,
/// a position), never names or IDs from the library.
pub const SAFE_PARAMS: &[&str] = &[
    "filters",
    "format",
    "maxWidth",
    "maxHeight",
    "fillWidth",
    "fillHeight",
    "quality",
    "position",
    "includeItemTypes",
    "recursive",
    "fields",
    "enableImageTypes",
    "imageTypeLimit",
    "enableUserData",
    "enableTotalRecordCount",
    "enableImages",
    "sortBy",
    "sortOrder",
    "startIndex",
    "limit",
    "minDateLastSaved",
    "static",
];

/// Hex digits and dashes, at least 8 long: a GUID, a tag, a hash.
pub fn looks_like_id(s: &str) -> bool {
    s.len() >= 8
        && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
        && s.chars().any(|c| c.is_ascii_hexdigit())
}

/// Keys dropped entirely.
const DROP_KEYS: &[&str] = &[
    "Path",
    "ImageBlurHashes",
    "LocalAddress",
    "OperatingSystem",
    "MediaSources",
    "MediaStreams",
];

#[derive(Debug)]
pub struct Anonymizer {
    key: [u8; 32],
    seen: HashMap<String, String>,
    /// Every original value replaced, for the leak check.
    originals: BTreeSet<String>,
}

impl Anonymizer {
    pub fn new(key: [u8; 32]) -> Anonymizer {
        Anonymizer {
            key,
            seen: HashMap::new(),
            originals: BTreeSet::new(),
        }
    }

    /// A keyed byte stream for one value. Borrows nothing, so the memo can be
    /// updated while it is in use.
    fn stream(&self, kind: &str, s: &str) -> impl Iterator<Item = u8> + use<> {
        let mut block = Sha256::new();
        block.update(self.key);
        block.update(kind.as_bytes());
        block.update([0]);
        block.update(s.as_bytes());
        let seed: [u8; 32] = block.finalize().into();
        (0u64..).flat_map(move |counter| {
            let mut h = Sha256::new();
            h.update(seed);
            h.update(counter.to_le_bytes());
            let out: [u8; 32] = h.finalize().into();
            out.into_iter()
        })
    }

    /// A stand-in with the same shape: case, digits, punctuation and script kept.
    pub fn text(&mut self, kind: &str, s: &str) -> String {
        if s.trim().is_empty() {
            return s.to_owned();
        }
        let memo = format!("{kind}\u{0}{s}");
        if let Some(done) = self.seen.get(&memo) {
            return done.clone();
        }
        let mut bytes = self.stream(kind, s);
        let mut next = move || u32::from(bytes.next().unwrap_or(0));
        let out: String = s
            .chars()
            .map(|c| {
                if c.is_ascii_lowercase() {
                    char::from(b'a' + (next() % 26) as u8)
                } else if c.is_ascii_uppercase() {
                    char::from(b'A' + (next() % 26) as u8)
                } else if c.is_ascii_digit() {
                    char::from(b'0' + (next() % 10) as u8)
                } else if c.is_ascii() || c.is_whitespace() || !c.is_alphanumeric() {
                    c
                } else {
                    same_block(c, &mut next)
                }
            })
            .collect();
        // Short values ("Pop", "The") identify nothing alone and would match key
        // names in the leak check; whole names are what it looks for.
        if s.chars().count() >= 5 {
            self.originals.insert(s.to_owned());
        }
        self.seen.insert(memo, out.clone());
        out
    }

    /// Any value: ID-shaped ones keep their hex shape, others get a text stand-in.
    pub fn value(&mut self, s: &str) -> String {
        if looks_like_id(s) {
            self.id(s)
        } else {
            self.text("name", s)
        }
    }

    /// An ID stand-in: hex digits stay hex digits (so a MusicBrainz ID still looks
    /// like one), the length and the dashes stay.
    pub fn id(&mut self, s: &str) -> String {
        if s.trim().is_empty() {
            return s.to_owned();
        }
        let memo = format!("id\u{0}{s}");
        if let Some(done) = self.seen.get(&memo) {
            return done.clone();
        }
        let mut bytes = self.stream("id", s);
        let mut next = move || u32::from(bytes.next().unwrap_or(0));
        let out: String = s
            .chars()
            .map(|c| {
                if c.is_ascii_hexdigit() {
                    let d = char::from_digit(next() % 16, 16).unwrap_or('0');
                    if c.is_ascii_uppercase() {
                        d.to_ascii_uppercase()
                    } else {
                        d
                    }
                } else if c.is_ascii_alphabetic() {
                    let base = if c.is_ascii_uppercase() { b'G' } else { b'g' };
                    char::from(base + (next() % 20) as u8)
                } else {
                    c
                }
            })
            .collect();
        if s.len() >= 4 {
            self.originals.insert(s.to_owned());
        }
        self.seen.insert(memo, out.clone());
        out
    }

    /// Walk a reply and replace what identifies the library. Object keys are kept
    /// (so `ProviderIds.MusicBrainzRecording` is still that key), values are not.
    pub fn json(&mut self, v: &Value) -> Value {
        self.walk(None, v)
    }

    fn walk(&mut self, key: Option<&str>, v: &Value) -> Value {
        match v {
            Value::Object(map) => {
                let mut out = Map::new();
                for (k, child) in map {
                    if DROP_KEYS.contains(&k.as_str()) {
                        continue;
                    }
                    if k == "AccessToken" {
                        out.insert(k.clone(), Value::String("anonymized-token".into()));
                        continue;
                    }
                    out.insert(k.clone(), self.walk(Some(k.as_str()), child));
                }
                Value::Object(out)
            }
            Value::Array(items) => Value::Array(items.iter().map(|i| self.walk(key, i)).collect()),
            Value::String(s) => match key {
                Some(k) if SAFE_KEYS.contains(&k) => Value::String(s.clone()),
                _ => Value::String(self.value(s)),
            },
            other => other.clone(),
        }
    }

    /// Where in `published` an original value survives, by key path (never the
    /// value itself). None when it is clean.
    pub fn find_leak(&self, published: &Value) -> Option<String> {
        fn walk(a: &Anonymizer, v: &Value, path: &str) -> Option<String> {
            match v {
                Value::Object(m) => m
                    .iter()
                    .find_map(|(k, c)| walk(a, c, &format!("{path}.{k}"))),
                Value::Array(items) => items.iter().find_map(|c| walk(a, c, &format!("{path}[]"))),
                Value::String(s) => a
                    .originals
                    .iter()
                    .any(|o| s.contains(o.as_str()))
                    .then(|| path.to_owned()),
                _ => None,
            }
        }
        walk(self, published, "")
    }

    /// Fail if any replaced original still appears in `published` text.
    #[cfg(test)]
    pub fn leak_check(&self, published: &str) -> Result<(), String> {
        for o in &self.originals {
            if published.contains(o.as_str()) {
                let shown: String = o.chars().take(3).collect();
                return Err(format!(
                    "an original value starting \"{shown}\" is still in the output"
                ));
            }
        }
        Ok(())
    }
}

/// A letter from the same run of 64 code points, so the script is kept. If that
/// run has no other letter, the character itself is kept (a single character is
/// not a name; whole names are what the leak check looks for).
fn same_block(c: char, next: &mut impl FnMut() -> u32) -> char {
    let base = (c as u32) & !0x3F;
    for _ in 0..48 {
        if let Some(candidate) = char::from_u32(base + next() % 64)
            && candidate.is_alphanumeric()
            && candidate != c
        {
            return candidate;
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn anon() -> Anonymizer {
        Anonymizer::new([7; 32])
    }

    #[test]
    fn names_keep_their_shape_and_script() {
        let mut a = anon();
        let latin = a.text("name", "Golden Hour Static (Live, 2024)");
        assert_ne!(latin, "Golden Hour Static (Live, 2024)");
        assert_eq!(latin.chars().count(), 31);
        assert_eq!(&latin[11..12], " ");
        assert!(latin.ends_with(")"));
        assert!(latin[..1].chars().all(|c| c.is_ascii_uppercase()));
        let cjk = a.text("name", "東京の夜");
        assert_eq!(cjk.chars().count(), 4);
        assert!(cjk.chars().all(|c| !c.is_ascii()));
        let hebrew = a.text("name", "שלום");
        assert!(
            hebrew
                .chars()
                .all(|c| ('\u{05C0}'..='\u{05FF}').contains(&c)),
            "{hebrew}"
        );
        assert_ne!(hebrew, "שלום");
    }

    #[test]
    fn ids_stay_hex_so_musicbrainz_ids_still_look_like_ids() {
        let mut a = anon();
        let mbid = a.id("5b11f4ce-a62d-471e-81fc-a69a8278c7da");
        assert_ne!(mbid, "5b11f4ce-a62d-471e-81fc-a69a8278c7da");
        let parts: Vec<&str> = mbid.split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            [8, 4, 4, 4, 12]
        );
        assert!(mbid.chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
    }

    #[test]
    fn the_same_value_always_gets_the_same_stand_in() {
        let mut a = anon();
        assert_eq!(a.id("abc123"), a.id("abc123"));
        assert_ne!(a.id("abc123"), a.id("abc124"));
        let id = a.id("0f8fad5b-d9cb-469f-a165-70867728950e");
        assert_eq!(id.len(), 36);
        assert_eq!(id.matches('-').count(), 4);
    }

    #[test]
    fn replies_lose_names_ids_paths_tokens_and_previews() {
        let mut a = anon();
        let reply = json!({
            "AccessToken": "real-token",
            "User": {"Name": "Sam", "Id": "u1u1u1u1"},
            "Items": [{"Id": "t1t1t1t1", "Name": "Slow Tide", "AlbumId": "a1a1a1a1", "Path": "D:\\Music\\x.flac",
                       "ImageBlurHashes": {"Primary": {"t": "LEHV6nWB2yk8"}}, "ImageTags": {"Primary": "tagtagtag"},
                       "ProviderIds": {"MusicBrainzRecording": "5b11f4ce-a62d-471e-81fc-a69a8278c7da"},
                       "ArtistItems": [{"Name": "Ember Cove", "Id": "r1r1r1r1"}], "Artists": ["Ember Cove"],
                       "RunTimeTicks": 2224000000i64, "Container": "flac"}]
        });
        let out = a.json(&reply);
        let text = out.to_string();
        assert!(!text.contains("real-token") && text.contains("anonymized-token"));
        assert!(!text.contains("Path") && !text.contains("ImageBlurHashes"));
        assert_eq!(out["Items"][0]["RunTimeTicks"], 2224000000i64);
        assert_eq!(out["Items"][0]["Container"], "flac");
        assert_eq!(
            out["Items"][0]["ProviderIds"]
                .as_object()
                .unwrap()
                .keys()
                .next()
                .unwrap(),
            "MusicBrainzRecording"
        );
        // The artist's name is replaced the same way in both places.
        assert_eq!(
            out["Items"][0]["ArtistItems"][0]["Name"],
            out["Items"][0]["Artists"][0]
        );
        assert!(a.leak_check(&text).is_ok());
        assert!(a.leak_check("... Slow Tide ...").is_err());
    }
}
