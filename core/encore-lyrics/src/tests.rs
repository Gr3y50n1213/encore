//! The parsers on written fixtures (tools/fixtures/lyrics, original text) and on
//! hostile input.

use crate::*;

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("../../../tools/fixtures/lyrics/", $name))
    };
}

fn read(text: &str) -> Lyrics {
    parse(text, Origin::MediaServer, &Rules::default())
        .unwrap()
        .expect("lyrics")
}

fn texts(l: &Lyrics) -> Vec<&str> {
    l.lines.iter().map(|x| x.text.as_str()).collect()
}

fn word_list(line: &Line) -> Vec<(&str, i64, i64, bool)> {
    line.words
        .iter()
        .map(|w| (w.text.as_str(), w.start_ms, w.end_ms, w.space_after))
        .collect()
}

#[test]
fn ttml_word_timing_singers_background_and_translations() {
    let l = read(fixture!("duet.ttml"));
    assert_eq!(l.format, Format::Ttml);
    assert_eq!(l.sync, SyncLevel::Word);
    assert_eq!(l.language.as_deref(), Some("en"));
    assert_eq!(
        texts(&l),
        [
            "The lighthouse blinks twice",
            "I count the waves",
            "Don\u{2019}t wait",
            "Salt & rope"
        ]
    );
    // "light" + "house" are two syllables of one word: no space between them.
    assert_eq!(
        word_list(&l.lines[0]),
        [
            ("The", 8000, 8400, true),
            ("light", 8400, 9100, false),
            ("house", 9100, 9600, true),
            ("blinks", 9600, 10200, true),
            ("twice", 10200, 11500, false),
        ]
    );
    assert_eq!(
        (l.lines[0].start_ms, l.lines[0].end_ms),
        (Some(8000), Some(11500))
    );

    // Singers: v1 on the main side, v2 opposite, the group line on the main side.
    let names: Vec<Option<&str>> = l.agents.iter().map(|a| a.name.as_deref()).collect();
    assert_eq!(names, [Some("Ember Cove"), Some("Salt & Lantern"), None]);
    let sides: Vec<Side> = l.lines.iter().map(|x| x.side).collect();
    assert_eq!(sides, [Side::Main, Side::Opposite, Side::Main, Side::Main]);
    assert_eq!(l.agents[2].kind, AgentKind::Group);

    // Background vocals under line 2, with their own words.
    let bg = &l.lines[1].background;
    assert_eq!(bg.len(), 1);
    assert_eq!(bg[0].text, "(the waves)");
    assert_eq!((bg[0].start_ms, bg[0].end_ms), (Some(14000), Some(15000)));
    assert_eq!(bg[0].words.len(), 2);

    // Translation from the head, pronunciation from the head, inline translation.
    assert_eq!(
        l.lines[0].translation.as_deref(),
        Some("Le phare clignote deux fois")
    );
    assert_eq!(
        l.lines[1].pronunciation.as_deref(),
        Some("ai kaunt the weivz")
    );
    assert_eq!(l.lines[1].translation, None);
    assert_eq!(l.lines[3].translation.as_deref(), Some("Sel et corde"));

    // The dots: before the first line (8 s) and the 12 s break after line 3.
    assert_eq!(
        l.gaps,
        [
            Gap {
                after_line: None,
                start_ms: 0,
                end_ms: 8000
            },
            Gap {
                after_line: Some(2),
                start_ms: 18000,
                end_ms: 30000
            }
        ]
    );
    assert!(l.lines.iter().all(|x| x.confidence == 1.0));
}

#[test]
fn ttml_times_relative_to_the_line_and_dur() {
    let l = read(fixture!("relative.ttml"));
    assert_eq!(l.sync, SyncLevel::Word);
    assert_eq!(
        word_list(&l.lines[0]),
        [("Wide", 10000, 11000, true), ("open", 11000, 13000, false)]
    );
    assert_eq!(l.lines[1].text, "No words here");
    assert_eq!(
        (l.lines[1].start_ms, l.lines[1].end_ms),
        (Some(20000), Some(22000))
    );
    assert!(
        l.lines[1].words.is_empty(),
        "a line with no words lights up whole"
    );
}

#[test]
fn ttml_line_timing_ignores_words() {
    let l = read(fixture!("line.ttml"));
    assert_eq!(l.sync, SyncLevel::Line);
    assert_eq!(texts(&l), ["Only lines", "here"]);
    assert!(l.lines.iter().all(|x| x.words.is_empty()));
}

#[test]
fn enhanced_lrc_words_offset_duets_and_background() {
    let l = read(fixture!("enhanced.lrc"));
    assert_eq!(l.format, Format::EnhancedLrc);
    assert_eq!(l.sync, SyncLevel::Word);
    assert_eq!(
        texts(&l),
        ["Harbour light, hold on", "I'm almost home", "Together"]
    );
    // [offset:+200] shows everything 200 ms earlier.
    assert_eq!(
        word_list(&l.lines[0]),
        [
            ("Har", 5000, 5400, false),
            ("bour", 5400, 5800, true),
            ("light,", 5800, 6600, true),
            ("hold", 6600, 7000, true),
            ("on", 7000, 7800, false),
        ]
    );
    assert_eq!(l.lines[0].background[0].text, "(hold on)");
    assert_eq!(l.lines[0].background[0].start_ms, Some(7000));
    let sides: Vec<Side> = l.lines.iter().map(|x| x.side).collect();
    assert_eq!(sides, [Side::Main, Side::Opposite, Side::Main]);
    assert_eq!(l.agents[2].kind, AgentKind::Group);
}

#[test]
fn lrc_repeats_blank_markers_credits_and_line_endings() {
    let lf = fixture!("simple.lrc").replace("\r\n", "\n");
    for text in [lf.clone(), lf.replace('\n', "\r\n"), lf.replace('\n', "\r")] {
        let l = read(&text);
        assert_eq!(l.format, Format::Lrc);
        assert_eq!(l.sync, SyncLevel::Line);
        assert_eq!(
            texts(&l),
            [
                "Paper moon over the bay",
                "Folded light on the water",
                "Every tide comes back again",
                "Paper moon over the bay",
                "Paper moon, stay"
            ],
            "the untimed credit line is dropped; the repeated line appears twice"
        );
        let times: Vec<(Option<i64>, Option<i64>)> =
            l.lines.iter().map(|x| (x.start_ms, x.end_ms)).collect();
        assert_eq!(
            times,
            [
                (Some(10000), Some(14500)),
                (Some(14500), Some(18000)), // ended by the blank line at 0:18
                (Some(25000), Some(40000)),
                (Some(40000), Some(44000)),
                (Some(44000), Some(49000)), // the last line stays lit for 5 s
            ]
        );
        assert_eq!(l.gaps.len(), 2, "{:?}", l.gaps);
        assert_eq!(l.gaps[1].after_line, Some(1));
    }
}

#[test]
fn the_last_line_never_runs_past_the_song() {
    let rules = Rules {
        song_duration_ms: Some(46_000),
        ..Rules::default()
    };
    let l = parse(fixture!("simple.lrc"), Origin::Sidecar, &rules)
        .unwrap()
        .unwrap();
    assert_eq!(l.lines.last().unwrap().end_ms, Some(46_000));
    assert_eq!(l.origin, Origin::Sidecar);
}

#[test]
fn plain_text_keeps_verse_breaks() {
    let l = read(fixture!("plain.txt"));
    assert_eq!(l.sync, SyncLevel::Unsynced);
    assert_eq!(l.format, Format::Plain);
    assert_eq!(
        texts(&l),
        [
            "First verse line",
            "",
            "Second verse, after a break",
            "",
            "Third, after two blanks"
        ]
    );
    assert!(l.gaps.is_empty() && l.lines.iter().all(|x| x.start_ms.is_none()));
}

#[test]
fn jellyfin_cues_are_utf16_ranges() {
    let l = read(fixture!("jellyfin-cues.json"));
    assert_eq!(l.format, Format::Jellyfin);
    assert_eq!(l.sync, SyncLevel::Word);
    assert_eq!(l.lines[0].text, "Café 🎵 tonight");
    assert_eq!(
        word_list(&l.lines[0]),
        [
            ("Café", 1000, 1500, true),
            ("🎵", 1500, 2000, true),
            ("tonight", 2000, 5000, false), // open end: up to the next line
        ]
    );
    assert!(l.lines[1].words.is_empty());
}

#[test]
fn jellyfin_unsynced_offset_and_bad_cues() {
    let unsynced =
        r#"{"Metadata":{"IsSynced":false},"Lyrics":[{"Text":"One","Start":null},{"Text":"Two"}]}"#;
    let l = read(unsynced);
    assert_eq!(l.sync, SyncLevel::Unsynced);
    assert_eq!(texts(&l), ["One", "Two"]);

    // A reported offset is applied once; cues pointing past the text are cut.
    let offset = r#"{"Metadata":{"Offset":5000000},"Lyrics":[
        {"Text":"Late","Start":30000000,"Cues":[{"Position":0,"EndPosition":99,"Start":30000000,"End":40000000},
                                                 {"Position":-5,"EndPosition":-1,"Start":1,"End":2}]}]}"#;
    let l = read(offset);
    assert_eq!(l.lines[0].start_ms, Some(2500));
    assert_eq!(word_list(&l.lines[0]), [("Late", 2500, 3500, false)]);

    // No lyrics at all: None, not an empty page.
    assert_eq!(
        parse(
            r#"{"Metadata":{},"Lyrics":[]}"#,
            Origin::MediaServer,
            &Rules::default()
        ),
        Ok(None)
    );
}

/// The real recording from the owner's server: line-synced lyrics with empty cue
/// lists (the text is anonymized; the timings are real).
#[test]
fn the_recorded_jellyfin_lyrics_read_as_line_synced() {
    let recording: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/fixtures/jellyfin/recorded.json"
    ))
    .unwrap();
    let body = recording
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["path"].as_str().is_some_and(|p| p.ends_with("/Lyrics")) && x["status"] == 200)
        .map(|x| x["body"].to_string())
        .unwrap();
    let l = read(&body);
    assert_eq!(l.sync, SyncLevel::Line);
    assert!(l.lines.len() > 10, "{}", l.lines.len());
    assert!(l.lines.windows(2).all(|w| w[0].start_ms <= w[1].start_ms));
    assert!(
        l.lines
            .iter()
            .all(|x| x.end_ms >= x.start_ms && !x.text.is_empty())
    );
}

#[test]
fn instrumental_and_empty() {
    let l = read("[00:00.00]Instrumental");
    assert!(l.instrumental && l.lines.is_empty());
    let l = read("[00:00.00](instrumental)\n[00:30.00]");
    assert!(l.instrumental);
    for nothing in ["", "   \n\n", "[ar:Someone]\n[ti:Nothing]", "\u{feff}"] {
        assert_eq!(
            parse(nothing, Origin::MediaServer, &Rules::default()),
            Ok(None),
            "{nothing:?}"
        );
    }
}

#[test]
fn unknown_markup_and_literal_angle_brackets() {
    let l = read("[00:01.00]a <3 b <00:02.00>c");
    assert_eq!(l.lines[0].text, "a <3 b c");
    let l = read(
        r#"<tt xmlns="http://www.w3.org/ns/ttml"><body><div><p begin="1s" end="2s">x <b>bold</b> y<br/>z</p></div></body></tt>"#,
    );
    assert_eq!(l.lines[0].text, "x bold y z");
}

/// Deterministic noise.
fn noise(seed: u64, len: usize) -> String {
    let mut x = seed;
    let alphabet: Vec<char> = "[]<>:.0123456789 /\n\r&;#xvbgabc\"='tpspan\u{e9}\u{1F3B5}{}"
        .chars()
        .collect();
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            alphabet[(x % alphabet.len() as u64) as usize]
        })
        .collect()
}

#[test]
fn hostile_input_never_panics() {
    let rules = Rules::default();
    // Every prefix of every fixture: truncated files.
    for f in [
        fixture!("duet.ttml"),
        fixture!("enhanced.lrc"),
        fixture!("simple.lrc"),
        fixture!("jellyfin-cues.json"),
    ] {
        for (i, _) in f.char_indices() {
            let _ = parse(&f[..i], Origin::MediaServer, &rules);
        }
    }
    // Noise, alone and behind each format's opening.
    for seed in 1..400u64 {
        let n = noise(seed, (seed as usize * 7) % 600);
        for prefix in [
            "",
            "<tt>",
            "[00:01.00]",
            "{\"Lyrics\":[",
            "<tt xmlns=\"x\"><body><p begin=\"1s\">",
        ] {
            let _ = parse(&format!("{prefix}{n}"), Origin::MediaServer, &rules);
        }
    }
    // Out-of-range numbers.
    for t in [
        "[999999999:59.99]x",
        "[00:01.00]<99999:00.00>a<00:00.00>b",
        r#"{"Lyrics":[{"Text":"x","Start":9223372036854775807,"Cues":[{"Position":2147483647,"EndPosition":-2147483648,"Start":-9223372036854775808,"End":9223372036854775807}]}]}"#,
        r#"<tt><body><p begin="99999999999h" end="-1s"><span begin="1e9s">x</span></p></body></tt>"#,
    ] {
        let _ = parse(t, Origin::MediaServer, &rules);
    }
}

#[test]
fn malformed_and_oversized_input_is_an_error() {
    assert!(matches!(
        parse(
            "<tt><body><p>unclosed</span></body></tt>",
            Origin::MediaServer,
            &Rules::default()
        ),
        Err(LyricsError::Malformed(_))
    ));
    assert!(matches!(
        parse("{\"Lyrics\": [", Origin::MediaServer, &Rules::default()),
        Err(LyricsError::Malformed(_))
    ));
    let huge = "a".repeat(MAX_BYTES + 1);
    assert_eq!(
        parse(&huge, Origin::MediaServer, &Rules::default()),
        Err(LyricsError::TooLarge)
    );
    let many = "[00:01.00]x\n".repeat(MAX_LINES + 1);
    assert_eq!(
        parse(&many, Origin::MediaServer, &Rules::default()),
        Err(LyricsError::TooLarge)
    );
}

/// A long word-timed song parses well inside one frame at 120 Hz (8 ms) in a
/// release build; the debug bound here only catches a blow-up. CI prints the
/// release time.
#[test]
fn a_long_word_timed_song_parses_quickly() {
    let mut ttml = String::from(
        r#"<tt xmlns="http://www.w3.org/ns/ttml" xmlns:itunes="http://music.apple.com/lyric-ttml-internal" itunes:timing="Word"><body><div>"#,
    );
    for line in 0..400 {
        let base = line * 3000;
        ttml.push_str(&format!(r#"<p begin="{base}ms" end="{}ms">"#, base + 2800));
        for w in 0..8 {
            let t = base + w * 350;
            ttml.push_str(&format!(
                r#"<span begin="{t}ms" end="{}ms">word{w}</span> "#,
                t + 350
            ));
        }
        ttml.push_str("</p>");
    }
    ttml.push_str("</div></body></tt>");
    let median = |f: &mut dyn FnMut()| {
        let mut times: Vec<std::time::Duration> = (0..7)
            .map(|_| {
                let t = std::time::Instant::now();
                f();
                t.elapsed()
            })
            .collect();
        times.sort();
        times[3]
    };
    let mut l = None;
    let parse = median(&mut || l = Some(read(&ttml)));
    let l = l.unwrap();
    // Every later view reads the kept copy back from JSON instead.
    let json = serde_json::to_string(&l).unwrap();
    let cached = median(&mut || {
        let back: Lyrics = serde_json::from_str(&json).unwrap();
        std::hint::black_box(back);
    });
    println!(
        "400 lines, 3,200 timed words: parsed in {:.2} ms, read back from the cache in {:.2} ms (medians of 7)",
        parse.as_secs_f64() * 1e3,
        cached.as_secs_f64() * 1e3
    );
    assert_eq!(l.lines.len(), 400);
    assert_eq!(l.lines.iter().map(|x| x.words.len()).sum::<usize>(), 3200);
    assert!(parse < std::time::Duration::from_millis(500), "{parse:?}");
}

#[test]
fn the_model_round_trips_through_json() {
    let l = read(fixture!("duet.ttml"));
    let json = serde_json::to_string(&l).unwrap();
    let back: Lyrics = serde_json::from_str(&json).unwrap();
    assert_eq!(back, l);
    assert!(json.contains("\"spaceAfter\""), "camelCase field names");
}
