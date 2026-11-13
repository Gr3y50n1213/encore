//! Tests on audio made in the test itself (click tracks, sines and chords): no real music is
//! in the repository, so there is nothing to licence and nothing to download.

use super::*;

const RATE: u32 = 44_100;

/// A little deterministic noise, so quiet parts are not digital silence.
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }
}

/// A short decaying burst at every beat.
fn click_track(bpm: f32, seconds: f32, rate: u32) -> Vec<f32> {
    let n = (seconds * rate as f32) as usize;
    let mut noise = Noise(7);
    let mut out: Vec<f32> = (0..n).map(|_| noise.next() * 0.002).collect();
    let period = 60.0 / bpm * rate as f32;
    let burst = (0.03 * rate as f32) as usize;
    let mut beat = 0f32;
    while (beat as usize) < n {
        let start = beat as usize;
        for i in 0..burst.min(n - start) {
            let t = i as f32 / rate as f32;
            out[start + i] +=
                0.8 * (-t * 120.0).exp() * (2.0 * std::f32::consts::PI * 1_500.0 * t).sin();
        }
        beat += period;
    }
    out
}

/// Notes sounding together for the whole clip.
fn chord(freqs: &[f32], seconds: f32, rate: u32, amp: f32) -> Vec<f32> {
    let n = (seconds * rate as f32) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / rate as f32;
            freqs
                .iter()
                .map(|f| amp * (2.0 * std::f32::consts::PI * f * t).sin())
                .sum::<f32>()
        })
        .collect()
}

#[test]
fn tempo_is_found_across_the_usual_range() {
    for bpm in [70.0, 85.0, 100.0, 120.0, 128.0, 140.0, 160.0] {
        let p = analyse(&click_track(bpm, 20.0, RATE), RATE).expect("a click track analyses");
        assert!(
            (p.bpm - bpm).abs() <= 2.0,
            "{bpm} BPM was read as {}",
            p.bpm
        );
        assert!(
            p.beat_strength > 0.3,
            "{bpm} BPM: the beat strength was {}",
            p.beat_strength
        );
    }
}

#[test]
fn tempo_does_not_depend_on_the_sample_rate() {
    for rate in [22_050, 44_100, 48_000] {
        let p = analyse(&click_track(110.0, 20.0, rate), rate).unwrap();
        assert!((p.bpm - 110.0).abs() <= 2.0, "{rate} Hz: read as {}", p.bpm);
    }
}

#[test]
fn a_steady_tone_has_no_steady_beat() {
    let p = analyse(&chord(&[440.0], 15.0, RATE, 0.3), RATE).unwrap();
    assert_eq!(p.bpm, 0.0, "a held note has no tempo, read as {}", p.bpm);
}

/// I, IV, V, I of a key (i, iv, v, i in minor) with the tonic chord sounding longest: a bare
/// triad fits more than one key (D major's D F# A is also most of F# minor), music doesn't.
fn progression(tonic_midi: i32, major: bool, rate: u32) -> Vec<f32> {
    let hz = |m: i32| 440.0 * 2f32.powf((m - 69) as f32 / 12.0);
    // With the bass note an octave down, as a bass line would play it.
    let triad = |root: i32, third: i32| [hz(root - 12), hz(root), hz(root + third), hz(root + 7)];
    let (i, iv, v) = if major {
        (
            triad(tonic_midi, 4),
            triad(tonic_midi + 5, 4),
            triad(tonic_midi + 7, 4),
        )
    } else {
        (
            triad(tonic_midi, 3),
            triad(tonic_midi + 5, 3),
            triad(tonic_midi + 7, 3),
        )
    };
    let mut out = chord(&i, 6.0, rate, 0.12);
    out.extend(chord(&iv, 3.0, rate, 0.12));
    out.extend(chord(&v, 3.0, rate, 0.12));
    out.extend(chord(&i, 4.0, rate, 0.12));
    out
}

#[test]
fn the_key_of_a_chord_progression_is_found() {
    // (tonic as a MIDI note in the fourth octave, major). A major key and its relative minor
    // (three semitones lower) share every note, and listening to pitch content alone can't always
    // tell them apart: the key may be read as its relative. The engine treats relatives as close
    // (docs/adr/core-0019), so this test accepts either, and counts how many are exact.
    let mut exact = 0;
    let cases = [
        (60, true),
        (69, false),
        (62, true),
        (64, false),
        (67, true),
        (66, false),
        (65, true),
    ];
    for (midi, major) in cases {
        let key = analyse(&progression(midi, major, RATE), RATE)
            .unwrap()
            .key
            .expect("music has a key");
        let tonic = (midi % 12) as u8;
        let relative_pc = if major {
            (tonic + 9) % 12
        } else {
            (tonic + 3) % 12
        };
        let is_exact = key.pitch_class == tonic && key.major == major;
        let is_relative = key.pitch_class == relative_pc && key.major != major;
        assert!(
            is_exact || is_relative,
            "tonic {midi} ({}), read as {}",
            if major { "major" } else { "minor" },
            key.name()
        );
        exact += usize::from(is_exact);
    }
    assert!(
        exact >= 5,
        "only {exact} of {} keys were exact",
        cases.len()
    );
}

#[test]
fn loudness_follows_the_standard() {
    // A 1 kHz sine of amplitude 0.1 is -20 dBFS peak, -23 dB in mean square: about -23 LUFS.
    let quiet = analyse(&chord(&[1_000.0], 12.0, RATE, 0.1), RATE).unwrap();
    assert!(
        (quiet.lufs + 23.0).abs() < 0.8,
        "read as {} LUFS",
        quiet.lufs
    );
    let loud = analyse(&chord(&[1_000.0], 12.0, RATE, 0.2), RATE).unwrap();
    assert!(
        (loud.lufs - quiet.lufs - 6.02).abs() < 0.3,
        "twice the level should be 6 dB more: {} then {}",
        quiet.lufs,
        loud.lufs
    );
}

#[test]
fn brightness_and_energy_rise_with_the_sound() {
    let dark = analyse(&chord(&[300.0], 12.0, RATE, 0.05), RATE).unwrap();
    let bright = analyse(&chord(&[6_000.0], 12.0, RATE, 0.3), RATE).unwrap();
    assert!(
        bright.brightness > dark.brightness + 0.2,
        "{} vs {}",
        dark.brightness,
        bright.brightness
    );
    assert!(
        bright.energy > dark.energy,
        "{} vs {}",
        dark.energy,
        bright.energy
    );
}

#[test]
fn dynamics_see_a_loudness_change() {
    let mut swell = chord(&[440.0], 16.0, RATE, 0.02);
    let louder = chord(&[440.0], 8.0, RATE, 0.4);
    let half = swell.len() / 2;
    for (a, b) in swell[half..].iter_mut().zip(&louder) {
        *a = *b;
    }
    let flat = analyse(&chord(&[440.0], 16.0, RATE, 0.2), RATE).unwrap();
    let wide = analyse(&swell, RATE).unwrap();
    assert!(
        wide.dynamics > flat.dynamics + 0.3,
        "{} vs {}",
        flat.dynamics,
        wide.dynamics
    );
}

#[test]
fn refuses_what_it_cannot_read_honestly() {
    assert_eq!(
        analyse(&chord(&[440.0], 3.0, RATE, 0.3), RATE),
        Err(AnalysisError::TooShort)
    );
    assert_eq!(
        analyse(&vec![0.0; 12 * RATE as usize], RATE),
        Err(AnalysisError::Silent)
    );
    assert!(matches!(
        analyse(&chord(&[440.0], 12.0, 4_000, 0.3), 4_000),
        Err(AnalysisError::Unsupported(_))
    ));
}

#[test]
fn the_stored_form_round_trips() {
    let p = analyse(&click_track(124.0, 15.0, RATE), RATE).unwrap();
    let bytes = p.encode();
    assert_eq!(bytes.len(), ENCODED_LEN);
    let back = SoundProfile::decode(&bytes).expect("decodes");
    assert!((back.bpm - p.bpm).abs() < 0.06);
    assert_eq!(back.key, p.key);
    assert!((back.lufs - p.lufs).abs() < 0.06);
    assert!((back.energy - p.energy).abs() < 0.003);
    assert_eq!(back.texture, p.texture);
    // Another version or length is refused, so an old row is analysed again.
    let mut old = bytes.clone();
    old[0] = VERSION + 1;
    assert!(SoundProfile::decode(&old).is_none());
    assert!(SoundProfile::decode(&bytes[1..]).is_none());
}

#[test]
fn analysing_twice_gives_the_same_bytes() {
    let audio = click_track(96.0, 15.0, RATE);
    assert_eq!(
        analyse(&audio, RATE).unwrap().encode(),
        analyse(&audio, RATE).unwrap().encode()
    );
}

#[test]
fn texture_tells_alike_from_different() {
    let a = analyse(&click_track(100.0, 15.0, RATE), RATE).unwrap();
    // The same sound, quieter: the texture is about shape, not level.
    let quieter: Vec<f32> = click_track(100.0, 15.0, RATE)
        .iter()
        .map(|s| s * 0.5)
        .collect();
    let b = analyse(&quieter, RATE).unwrap();
    let c = analyse(&chord(&[261.63, 329.63, 392.0], 15.0, RATE, 0.15), RATE).unwrap();
    let same = a.texture_similarity(&b);
    let different = a.texture_similarity(&c);
    assert!(same > 0.97, "the same sound at half level: {same}");
    assert!(
        different < same - 0.1,
        "a click track against a chord: {different} vs {same}"
    );
    assert_eq!(a.texture_similarity(&a), 1.0);
}
