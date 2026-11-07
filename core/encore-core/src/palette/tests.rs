//! The four board 04 covers (original art, rendered from their SVGs to 64 x 64 PNG
//! in tools/fixtures/artwork), determinism, and awkward covers.

use super::*;
use crate::tokens::tokens;

fn settings() -> Settings {
    tokens().palette.clone()
}

fn lab_hex(h: &str) -> Lab {
    let v = u32::from_str_radix(h, 16).unwrap();
    lab_of([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

fn dist(a: &str, b: &str) -> f64 {
    lab_hex(a).dist2(lab_hex(b)).sqrt()
}

/// For each expected colour, the distance to the nearest measured one.
fn nearest(expected: &[&str], measured: &[String]) -> Vec<f64> {
    expected
        .iter()
        .map(|e| {
            measured
                .iter()
                .map(|m| dist(e, m))
                .fold(f64::INFINITY, f64::min)
        })
        .collect()
}

struct Board {
    file: &'static [u8],
    name: &'static str,
    sampled: [&'static str; 5],
}

const BOARD: [Board; 4] = [
    Board {
        file: include_bytes!("../../../../tools/fixtures/artwork/golden-hour-static.png"),
        name: "Golden Hour Static",
        sampled: ["E65D45", "C52957", "1E1231", "FED07A", "673942"],
    },
    Board {
        file: include_bytes!("../../../../tools/fixtures/artwork/tidewater.png"),
        name: "Tidewater",
        sampled: ["0D3857", "61C6BF", "267B8D", "C2EFE6", "1C506A"],
    },
    Board {
        file: include_bytes!("../../../../tools/fixtures/artwork/brass-arches.png"),
        name: "Brass Arches",
        sampled: ["1F3D2B", "E8C053", "86AD66", "D8ED92", "D8ED92"],
    },
    Board {
        file: include_bytes!("../../../../tools/fixtures/artwork/paper-moon.png"),
        name: "Paper Moon",
        sampled: ["F3EBDD", "D94C3A", "D4C1B5", "E59383", "87837C"],
    },
];

/// The board's numbers come from tokens.json `albumGradient.exampleAlbums`, so the
/// test and the design system can't drift apart.
fn board_example(name: &str) -> (String, Vec<String>, f64) {
    let v: serde_json::Value =
        serde_json::from_str(include_str!("../../../../design/tokens.json")).unwrap();
    let ex = v["albumGradient"]["exampleAlbums"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == name)
        .unwrap()
        .clone();
    let colours: Vec<String> = ex["baseThenStops"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_owned())
        .collect();
    (
        ex["scheme"].as_str().unwrap().to_owned(),
        colours,
        ex["lowestContrast"].as_f64().unwrap(),
    )
}

#[test]
fn the_board_covers_give_the_boards_palettes() {
    let s = settings();
    for b in &BOARD {
        let p = palette_of_png(b.file, &s).unwrap().unwrap();
        let (scheme, board_colours, board_contrast) = board_example(b.name);
        let sampled: Vec<String> = p.sampled.iter().map(|x| x.colour.clone()).collect();
        let mut ours = vec![p.base.clone()];
        ours.extend(p.stops.iter().cloned());
        let sampled_d = nearest(&b.sampled, &sampled);
        let used_d = nearest(
            &board_colours.iter().map(String::as_str).collect::<Vec<_>>(),
            &ours,
        );
        println!(
            "{}: {:?}, base {} stops {:?}, lowest contrast {} (board {})\n  sampled {:?} vs board {:?}: OKLab distance to nearest {:.3?}\n  used {:?} vs board {:?}: OKLab distance to nearest {:.3?}",
            b.name,
            p.scheme,
            p.base,
            p.stops,
            p.lowest_contrast,
            board_contrast,
            sampled,
            b.sampled,
            sampled_d,
            ours,
            board_colours,
            used_d
        );
        let expected = if scheme == "light" {
            Scheme::Light
        } else {
            Scheme::Dark
        };
        assert_eq!(p.scheme, expected, "{}", b.name);
        assert_eq!(p.stops.len(), 4);
        let min = if expected == Scheme::Light { 7.0 } else { 4.5 };
        assert!(
            p.lowest_contrast >= min,
            "{}: {}",
            b.name,
            p.lowest_contrast
        );
        assert_eq!(
            p.text,
            if expected == Scheme::Light {
                "000000"
            } else {
                "FFFFFF"
            }
        );
        // Every colour the board sampled has a cluster of ours close to it, and
        // every colour the board used has one of ours close to it.
        assert!(
            sampled_d.iter().all(|d| *d < SAMPLED_TOLERANCE),
            "{}: {sampled_d:?}",
            b.name
        );
        assert!(
            used_d.iter().all(|d| *d < USED_TOLERANCE),
            "{}: {used_d:?}",
            b.name
        );
    }
}

/// Measured on 6 November 2026: at most 0.038 over the four covers. The bound
/// sits just above it, to catch a change that moves the palettes away from the
/// board.
const USED_TOLERANCE: f64 = 0.05;

/// How close our clusters must be to the board's (OKLab distance; 0.02 is about a
/// just-noticeable difference). The board's own sampling used a renderer and a
/// k-means of its own, so the clusters can't match exactly.
const SAMPLED_TOLERANCE: f64 = 0.1;

#[test]
fn the_same_cover_gives_the_same_bytes() {
    let s = settings();
    let first = serde_json::to_string(&palette_of_png(BOARD[0].file, &s).unwrap()).unwrap();
    for _ in 0..100 {
        let again = serde_json::to_string(&palette_of_png(BOARD[0].file, &s).unwrap()).unwrap();
        assert_eq!(again, first);
    }
    // A golden value: if this changes, every palette made so far changes too (and
    // the server's and the recap's). Change the algorithm or its tokens only on
    // purpose, and update this with it. CI checks it on Linux, macOS, Windows
    // and Android builds alike.
    println!("golden: {first}");
    assert_eq!(first, GOLDEN_HOUR_STATIC);
}

const GOLDEN_HOUR_STATIC: &str = r#"{"scheme":"dark","base":"3D0000","stops":["CF4630","1B1030","C02D56","986E00"],"text":"FFFFFF","lowestContrast":4.59,"sampled":[{"colour":"E65C44","share":0.2991},{"colour":"1B1030","share":0.2656},{"colour":"C02D56","share":0.2134},{"colour":"FFD17A","share":0.1628},{"colour":"58303E","share":0.0591}]}"#;

#[test]
fn colour_maths_round_trips() {
    for v in 0..=255u8 {
        assert_eq!(from_linear(SRGB_TO_LINEAR[v as usize]), v);
    }
    for c in [
        [0, 0, 0],
        [255, 255, 255],
        [230, 93, 69],
        [13, 56, 87],
        [243, 235, 221],
        [255, 0, 0],
    ] {
        assert_eq!(rgb_of(lab_of(c)), c, "{c:?}");
    }
    for x in [1e-9, 0.001, 0.125, 0.5, 1.0, 8.0, 27.0] {
        let y = cbrt(x);
        assert!((y * y * y - x).abs() <= x * 1e-14, "{x}: {y}");
    }
    assert_eq!(cbrt(-8.0), -2.0);
    assert!((contrast([255, 255, 255], [0, 0, 0]) - 21.0).abs() < 1e-9);
    assert_eq!(two_decimals(4.516), 4.52);
}

fn solid(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
    (0..w * h).flat_map(|_| rgba).collect()
}

#[test]
fn awkward_covers() {
    let s = settings();
    // Solid black and solid white: one colour, still four stops and readable text.
    let black = palette_of_rgba(8, 8, &solid(8, 8, [0, 0, 0, 255]), &s).unwrap();
    assert_eq!((black.scheme, black.stops.len()), (Scheme::Dark, 4));
    assert!(black.lowest_contrast >= 4.5);
    let white = palette_of_rgba(8, 8, &solid(8, 8, [255, 255, 255, 255]), &s).unwrap();
    assert_eq!((white.scheme, white.stops.len()), (Scheme::Light, 4));
    assert!(white.lowest_contrast >= 7.0);
    // One pixel.
    assert!(palette_of_rgba(1, 1, &[200, 40, 40, 255], &s).is_some());
    // Fully transparent: no artwork.
    assert_eq!(
        palette_of_rgba(4, 4, &solid(4, 4, [255, 0, 0, 0]), &s),
        None
    );
    // Half transparent: only the opaque half counts.
    let mut half = solid(4, 4, [0, 0, 255, 0]);
    for px in half.chunks_mut(4).take(8) {
        px.copy_from_slice(&[255, 0, 0, 255]);
    }
    let p = palette_of_rgba(4, 4, &half, &s).unwrap();
    assert_eq!(p.sampled.len(), 1, "{:?}", p.sampled);
    // A wide cover larger than the sample size is averaged down, not cropped.
    let mut wide = solid(300, 20, [10, 120, 60, 255]);
    for px in wide.chunks_mut(4).skip(150 * 20).take(50) {
        px.copy_from_slice(&[250, 250, 0, 255]);
    }
    assert!(palette_of_rgba(300, 20, &wide, &s).is_some());
    assert_eq!(sample(300, 20, &wide, 64).len(), 60 * 4);
}

/// Real covers from the owner's library, kept only on that computer
/// (tools/target/private-covers, written by the fixture recorder; copyrighted, so
/// never committed). Run by hand:
///   ENCORE_PRIVATE_COVERS=../tools/target/private-covers cargo test -p encore-core real_covers -- --ignored --nocapture
#[test]
#[ignore = "needs the owner's covers: ENCORE_PRIVATE_COVERS"]
fn real_covers_keep_text_readable() {
    let dir = std::env::var("ENCORE_PRIVATE_COVERS").expect("set ENCORE_PRIVATE_COVERS");
    let s = settings();
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "png"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    let (mut dark, mut light, mut lowest) = (0, 0, f64::INFINITY);
    for f in &files {
        let p = palette_of_png(&std::fs::read(f).unwrap(), &s)
            .unwrap()
            .expect("a cover has a palette");
        let min = if p.scheme == Scheme::Light { 7.0 } else { 4.5 };
        assert!(p.lowest_contrast >= min, "{}: {}", f.display(), p.lowest_contrast);
        assert_eq!(p.stops.len(), 4);
        match p.scheme {
            Scheme::Dark => dark += 1,
            Scheme::Light => light += 1,
        }
        lowest = lowest.min(p.lowest_contrast - min);
    }
    println!(
        "{} real covers: {dark} dark, {light} light; every one readable, smallest margin over its minimum {lowest:.2}",
        files.len()
    );
}

#[test]
fn broken_files_are_errors_not_panics() {
    let s = settings();
    assert!(matches!(
        palette_of_png(b"", &s),
        Err(PaletteError::Unreadable(_))
    ));
    assert!(matches!(
        palette_of_png(b"\x89PNG\r\n\x1a\nnot really", &s),
        Err(PaletteError::Unreadable(_))
    ));
    let good = BOARD[1].file;
    for cut in [8, 20, 33, 60, good.len() / 2, good.len() - 1] {
        let _ = palette_of_png(&good[..cut], &s);
    }
    let mut flipped = good.to_vec();
    for i in (40..flipped.len()).step_by(7) {
        flipped[i] ^= 0x5A;
        let _ = palette_of_png(&flipped, &s);
    }
}
