//! Reading a song's audio (feature `decode`): a window of about 30 seconds from the middle of
//! the song, decoded to mono samples and handed to [`crate::analyse`].
//!
//! The source is anything seekable (a file, or the ranged reader over HTTP in encore-sources),
//! so only the bytes the decoder needs for that window are read: a seek jumps to the window
//! instead of reading the song from the start.

use symphonia::core::audio::GenericAudioBufferRef;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::Time;

use crate::{AnalysisError, SoundProfile, analyse};

/// How much audio is analysed.
pub const WINDOW_SECONDS: f64 = 30.0;
/// Where the window starts, as a share of the song (the middle is more typical than an intro).
const START_SHARE: f64 = 0.35;
/// A song this short is read from its start.
const SHORT_SECONDS: f64 = 45.0;

/// Mono samples and their sample rate, from a window of the song.
#[derive(Debug, Clone)]
pub struct Window {
    pub samples: Vec<f32>,
    pub rate: u32,
}

/// Analyse the song in `source`. `extension` (for example "mp3") helps find the format when the
/// bytes alone are ambiguous.
pub fn analyse_source(
    source: Box<dyn MediaSource>,
    extension: Option<&str>,
) -> Result<SoundProfile, AnalysisError> {
    let window = decode_window(source, extension)?;
    analyse(&window.samples, window.rate)
}

/// Decode about [`WINDOW_SECONDS`] of the song to mono.
pub fn decode_window(
    source: Box<dyn MediaSource>,
    extension: Option<&str>,
) -> Result<Window, AnalysisError> {
    let mss = MediaSourceStream::new(source, Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = extension {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| AnalysisError::Unsupported(format!("unknown format ({e})")))?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| AnalysisError::Unsupported("no audio track".into()))?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| AnalysisError::Unsupported("no audio parameters".into()))?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(|e| AnalysisError::Unsupported(format!("no decoder for this audio ({e})")))?;

    // The length, when the container says: from the frame count or the duration and the time base.
    let total_seconds = track
        .time_base
        .zip(track.duration)
        .and_then(|(tb, d)| {
            let end = track.start_ts.checked_add(d)?;
            tb.calc_time(end)
        })
        .map(|t| t.as_secs_f64());
    if let Some(total) = total_seconds.filter(|t| *t > SHORT_SECONDS) {
        let start = (total * START_SHARE)
            .min(total - WINDOW_SECONDS - 1.0)
            .max(0.0);
        if let Some(time) = Time::try_from_secs_f64(start) {
            // A failed seek reads from the start: slower, still right.
            if format
                .seek(
                    SeekMode::Coarse,
                    SeekTo::Time {
                        time,
                        track_id: Some(track_id),
                    },
                )
                .is_ok()
            {
                decoder.reset();
            }
        }
    }

    let mut samples: Vec<f32> = Vec::new();
    let mut rate = 0u32;
    let mut scratch: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(SymphoniaError::ResetRequired) => break,
            Err(SymphoniaError::IoError(_)) => break,
            Err(e) => return Err(AnalysisError::Decode(e.to_string())),
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buf) => {
                rate = buf.spec().rate();
                append_mono(&buf, &mut scratch, &mut samples);
                if rate > 0 && samples.len() as f64 >= WINDOW_SECONDS * f64::from(rate) {
                    break;
                }
            }
            // One bad packet (a damaged frame) is skipped, as players do.
            Err(SymphoniaError::DecodeError(_)) => {}
            Err(e) => return Err(AnalysisError::Decode(e.to_string())),
        }
    }
    if rate == 0 || samples.is_empty() {
        return Err(AnalysisError::Decode("no audio could be decoded".into()));
    }
    Ok(Window { samples, rate })
}

/// The buffer's channels averaged into `out`.
fn append_mono(buf: &GenericAudioBufferRef<'_>, scratch: &mut Vec<f32>, out: &mut Vec<f32>) {
    let channels = buf.spec().channels().count().max(1);
    scratch.resize(buf.samples_interleaved(), 0.0);
    buf.copy_to_slice_interleaved(&mut scratch[..]);
    if channels == 1 {
        out.extend_from_slice(scratch);
    } else {
        out.extend(
            scratch
                .chunks_exact(channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A 16-bit PCM WAV file in memory.
    fn wav(samples: &[f32], rate: u32, channels: u16) -> Vec<u8> {
        let data: Vec<u8> = samples
            .iter()
            .flat_map(|s| {
                let v = (s.clamp(-1.0, 1.0) * 32_767.0) as i16;
                (0..channels).flat_map(move |_| v.to_le_bytes())
            })
            .collect();
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
        out.extend_from_slice(&(channels * 2).to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        out
    }

    fn clicks(bpm: f32, seconds: f32, rate: u32) -> Vec<f32> {
        let n = (seconds * rate as f32) as usize;
        let mut out = vec![0.001f32; n];
        let period = 60.0 / bpm * rate as f32;
        let mut beat = 0f32;
        while (beat as usize) < n {
            let start = beat as usize;
            for i in 0..((0.03 * rate as f32) as usize).min(n - start) {
                let t = i as f32 / rate as f32;
                out[start + i] +=
                    0.8 * (-t * 120.0).exp() * (2.0 * std::f32::consts::PI * 1_500.0 * t).sin();
            }
            beat += period;
        }
        out
    }

    #[test]
    fn a_wav_is_read_and_analysed() {
        let bytes = wav(&clicks(120.0, 20.0, 44_100), 44_100, 1);
        let p = analyse_source(Box::new(Cursor::new(bytes)), Some("wav")).expect("a wav analyses");
        assert!((p.bpm - 120.0).abs() <= 2.0, "read as {} BPM", p.bpm);
    }

    #[test]
    fn stereo_is_mixed_to_mono() {
        let bytes = wav(&clicks(100.0, 20.0, 48_000), 48_000, 2);
        let window = decode_window(Box::new(Cursor::new(bytes)), None).unwrap();
        assert_eq!(window.rate, 48_000);
        assert!(
            (window.samples.len() as i64 - 20 * 48_000).abs() < 2_000,
            "{} samples",
            window.samples.len()
        );
    }

    #[test]
    fn a_long_song_is_read_in_a_window_from_its_middle() {
        // 120 seconds of beats: the window is about 30 s, not the whole song.
        let bytes = wav(&clicks(90.0, 120.0, 22_050), 22_050, 1);
        let window = decode_window(Box::new(Cursor::new(bytes)), Some("wav")).unwrap();
        let seconds = window.samples.len() as f64 / f64::from(window.rate);
        assert!(
            (29.0..=45.0).contains(&seconds),
            "read {seconds} s of a 120 s song"
        );
        let p = analyse(&window.samples, window.rate).unwrap();
        assert!((p.bpm - 90.0).abs() <= 2.0, "read as {} BPM", p.bpm);
    }

    #[test]
    fn what_is_not_audio_is_refused() {
        let err = analyse_source(Box::new(Cursor::new(vec![7u8; 5_000])), None).unwrap_err();
        assert!(matches!(err, AnalysisError::Unsupported(_)), "{err}");
    }
}
