//! The sound profile of one song and its compact stored form.

use std::fmt;

/// The length of the texture vector.
pub const TEXTURE_LEN: usize = 48;

/// The stored form's version byte. A profile with another version is analysed again.
pub const VERSION: u8 = 1;

/// The size of [`SoundProfile::encode`]'s output in bytes.
pub const ENCODED_LEN: usize = 1 + 2 + 1 + 1 + 2 + 3 + TEXTURE_LEN;

/// A musical key: a pitch class (C = 0, C sharp = 1, ... B = 11) and a mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub pitch_class: u8,
    pub major: bool,
}

impl Key {
    /// "C major", "A minor", "F# major" (sharps, no flats).
    pub fn name(self) -> String {
        const NAMES: [&str; 12] = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        format!(
            "{} {}",
            NAMES[usize::from(self.pitch_class % 12)],
            if self.major { "major" } else { "minor" }
        )
    }
}

/// What a song sounds like, measured from about 30 seconds of its audio.
///
/// Only what signal processing can measure honestly is here: how fast, in what key,
/// how loud, how energetic, how bright, how much it changes in loudness, and a small
/// vector for the sound's texture (its spectral shape over time and its pitch
/// content). Happy or sad, acoustic, danceable and instrumental need a trained
/// classifier and are not guessed (docs/adr/core-0019).
#[derive(Debug, Clone, PartialEq)]
pub struct SoundProfile {
    /// Beats per minute, 0 when the clip has no steady beat.
    pub bpm: f32,
    /// How steady the beat is, 0 to 1.
    pub beat_strength: f32,
    /// The key, when one stands out. From pitch content alone a major key and its relative
    /// minor (three semitones lower) are sometimes confused, so treat them as close.
    pub key: Option<Key>,
    /// Integrated loudness, LUFS (ITU-R BS.1770 / EBU R128).
    pub lufs: f32,
    /// A mix of loudness, how much changes between moments, and brightness: 0 calm to 1 intense.
    pub energy: f32,
    /// How much high-frequency content: 0 warm and dark to 1 bright.
    pub brightness: f32,
    /// How much the loudness moves: 0 flat to 1 wide.
    pub dynamics: f32,
    /// The texture, each value -127..127 for -1..1. Compare with [`SoundProfile::texture_similarity`].
    pub texture: [i8; TEXTURE_LEN],
}

/// Why a clip could not be analysed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisError {
    /// Shorter than [`crate::MIN_SECONDS`].
    TooShort,
    /// Silence.
    Silent,
    /// The decoder could not read the audio; the text says why.
    Decode(String),
    /// A format the decoder does not handle (for example Opus).
    Unsupported(String),
    /// The source failed while being read (a network error, a server that would not serve the
    /// file): nothing is known about the audio, so the song is tried again later, not marked unreadable.
    Io(String),
}

impl AnalysisError {
    /// Whether the song itself is the problem (never worth trying again) and not the way it was fetched.
    pub fn is_permanent(&self) -> bool {
        !matches!(self, AnalysisError::Io(_))
    }
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalysisError::TooShort => write!(f, "the clip is too short to analyse"),
            AnalysisError::Silent => write!(f, "the clip is silent"),
            AnalysisError::Decode(why) => write!(f, "could not read the audio: {why}"),
            AnalysisError::Unsupported(what) => write!(f, "not a format Encore can read: {what}"),
            AnalysisError::Io(why) => write!(f, "the song could not be fetched: {why}"),
        }
    }
}

impl std::error::Error for AnalysisError {}

impl SoundProfile {
    /// The stored form, [`ENCODED_LEN`] bytes: version, beats per minute (tenths), key (0 to
    /// 23, 255 for none), beat strength, loudness (tenths of a LUFS), energy, brightness,
    /// dynamics (one byte each, 0 to 255) and the texture.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(ENCODED_LEN);
        out.push(VERSION);
        out.extend_from_slice(
            &((self.bpm * 10.0).round().clamp(0.0, 65_535.0) as u16).to_le_bytes(),
        );
        out.push(match self.key {
            Some(k) => k.pitch_class % 12 + if k.major { 0 } else { 12 },
            None => 255,
        });
        out.push(unit_byte(self.beat_strength));
        out.extend_from_slice(
            &((self.lufs * 10.0).round().clamp(-1000.0, 300.0) as i16).to_le_bytes(),
        );
        out.push(unit_byte(self.energy));
        out.push(unit_byte(self.brightness));
        out.push(unit_byte(self.dynamics));
        out.extend(self.texture.iter().map(|v| *v as u8));
        debug_assert_eq!(out.len(), ENCODED_LEN);
        out
    }

    /// The profile from its stored form; None for another version or a wrong length.
    pub fn decode(bytes: &[u8]) -> Option<SoundProfile> {
        if bytes.len() != ENCODED_LEN || bytes[0] != VERSION {
            return None;
        }
        let bpm = f32::from(u16::from_le_bytes([bytes[1], bytes[2]])) / 10.0;
        let key = match bytes[3] {
            k @ 0..=23 => Some(Key {
                pitch_class: k % 12,
                major: k < 12,
            }),
            _ => None,
        };
        let beat_strength = f32::from(bytes[4]) / 255.0;
        let lufs = f32::from(i16::from_le_bytes([bytes[5], bytes[6]])) / 10.0;
        let energy = f32::from(bytes[7]) / 255.0;
        let brightness = f32::from(bytes[8]) / 255.0;
        let dynamics = f32::from(bytes[9]) / 255.0;
        let mut texture = [0i8; TEXTURE_LEN];
        for (slot, byte) in texture.iter_mut().zip(&bytes[10..]) {
            *slot = *byte as i8;
        }
        Some(SoundProfile {
            bpm,
            beat_strength,
            key,
            lufs,
            energy,
            brightness,
            dynamics,
            texture,
        })
    }

    /// How alike two textures are, 0 to 1 (cosine, integer arithmetic so every platform agrees).
    pub fn texture_similarity(&self, other: &SoundProfile) -> f64 {
        texture_cosine(&self.texture, &other.texture)
    }
}

fn unit_byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Cosine similarity of two texture vectors mapped to 0..1. Dot products and norms are
/// exact integers; the one division and the square roots are correctly rounded, so the
/// result is the same on every platform.
pub fn texture_cosine(a: &[i8; TEXTURE_LEN], b: &[i8; TEXTURE_LEN]) -> f64 {
    let (mut dot, mut na, mut nb) = (0i64, 0i64, 0i64);
    for (x, y) in a.iter().zip(b) {
        let (x, y) = (i64::from(*x), i64::from(*y));
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0 || nb == 0 {
        return 0.0;
    }
    let cos = dot as f64 / ((na as f64).sqrt() * (nb as f64).sqrt());
    ((cos + 1.0) / 2.0).clamp(0.0, 1.0)
}
