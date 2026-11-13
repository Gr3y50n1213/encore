//! YAMNet in Rust (docs/adr/core-0019): what a few seconds of audio sound like, as 1024 numbers
//! and 521 AudioSet class scores (genres, instruments, "singing", "sad music" and so on).
//!
//! YAMNet is Google's audio event model (Apache-2.0), a MobileNet-v1 over a log-mel
//! spectrogram. It is run here by about 400 lines of plain Rust over weights converted from
//! MediaPipe's TFLite file (`tools/sound-spike/export_yamnet.py`), so there is no runtime library,
//! no 16 KB page-size question on Android and no NPU path to maintain, and every platform gets
//! the same numbers. The weights stay int8 with one scale per output channel (3.9 MB) and are
//! turned into floats once, when the model is loaded.
//!
//! One patch is 0.975 s of 16 kHz audio: 96 frames of 25 ms every 10 ms, 64 mel bands from 125 Hz
//! to 7.5 kHz, the log of each. A clip's embedding is the mean over its patches.

use std::fmt;

use realfft::RealFftPlanner;

/// The model works on audio at this rate.
pub const RATE: u32 = 16_000;
/// Samples in one patch (0.975 s).
pub const PATCH_SAMPLES: usize = 15_600;
/// The embedding's length, and the number of classes.
pub const EMBEDDING_LEN: usize = 1024;
pub const CLASSES: usize = 521;

const FRAMES: usize = 96;
const MELS: usize = 64;
const FFT: usize = 512;
const BINS: usize = FFT / 2 + 1;
const WINDOW: usize = 400;
const HOP: usize = 160;

/// Stride and output channels of each of the 13 depthwise-separable blocks (the first
/// convolution, 3x3 stride 2 from one channel to 32, comes before them).
const BLOCKS: [(usize, usize); 13] = [
    (1, 64),
    (2, 128),
    (1, 128),
    (2, 256),
    (1, 256),
    (2, 512),
    (1, 512),
    (1, 512),
    (1, 512),
    (1, 512),
    (1, 512),
    (2, 1024),
    (1, 1024),
];
const FIRST_CHANNELS: usize = 32;

/// Why a model file could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// Not a model file from `export_yamnet.py`.
    NotAModel,
    /// A model file of a version this build does not know.
    Version(u32),
    /// The file ends early.
    Truncated,
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelError::NotAModel => write!(f, "this is not a sound model file"),
            ModelError::Version(v) => write!(
                f,
                "the sound model file is version {v}, which this app does not know"
            ),
            ModelError::Truncated => write!(f, "the sound model file is cut short"),
        }
    }
}

impl std::error::Error for ModelError {}

/// A feature map, height x width x channels, channels innermost.
struct Map {
    h: usize,
    w: usize,
    c: usize,
    data: Vec<f32>,
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], ModelError> {
        let end = self.at.checked_add(n).ok_or(ModelError::Truncated)?;
        let s = self.bytes.get(self.at..end).ok_or(ModelError::Truncated)?;
        self.at = end;
        Ok(s)
    }

    fn u32(&mut self) -> Result<u32, ModelError> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| ModelError::Truncated)?,
        ))
    }

    fn f32s(&mut self, n: usize) -> Result<Vec<f32>, ModelError> {
        Ok(self
            .take(n * 4)?
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect())
    }

    /// int8 weights times their per-output-channel scale: `rows` x `cols` values, then the
    /// scales and the biases. Returns (weights, bias) with the weights as rows of `cols`.
    fn layer(&mut self, rows: usize, cols: usize) -> Result<(Vec<f32>, Vec<f32>), ModelError> {
        let q = self.take(rows * cols)?.to_vec();
        let scale = self.f32s(rows)?;
        let bias = self.f32s(rows)?;
        let mut w = vec![0f32; rows * cols];
        for r in 0..rows {
            for c in 0..cols {
                w[r * cols + c] = f32::from(q[r * cols + c] as i8) * scale[r];
            }
        }
        Ok((w, bias))
    }
}

/// A convolution with its weights laid out for the loops below.
struct Conv {
    /// Depthwise: [tap][channel]. Pointwise: [input channel][output channel]. First: [tap][output].
    w: Vec<f32>,
    bias: Vec<f32>,
    stride: usize,
}

/// The loaded network.
pub struct Yamnet {
    window: Vec<f32>,
    /// The mel filters as (first bin, weights) per band: they are triangles, so most of each row is zero.
    mel: Vec<(usize, Vec<f32>)>,
    mel_offset: f32,
    first: Conv,
    blocks: Vec<(Conv, Conv)>,
    /// [class][input]
    classifier: Vec<f32>,
    classifier_bias: Vec<f32>,
}

impl fmt::Debug for Yamnet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Yamnet").finish_non_exhaustive()
    }
}

/// What one patch, or the mean of many, came to.
#[derive(Debug, Clone, PartialEq)]
pub struct Embedding {
    /// The 1024 numbers before the classifier, each 0 to 6.
    pub pooled: Vec<f32>,
    /// The 521 class scores, each 0 to 1.
    pub scores: Vec<f32>,
}

/// The name stored beside an embedding, so a better model can replace it later: every row made by
/// another name is analysed again.
pub const MODEL_NAME: &str = "yamnet-1";

/// The stored form's length: one version byte, the 1024 numbers and the 521 scores, one byte each.
pub const EMBEDDING_ENCODED_LEN: usize = 1 + EMBEDDING_LEN + CLASSES;

impl Embedding {
    /// One byte per number: the embedding's 0 to 6 and the scores' 0 to 1 scaled to 0 to 255, which
    /// keeps nearest-neighbour order (each value is within 0.012 of the truth).
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(EMBEDDING_ENCODED_LEN);
        out.push(1);
        out.extend(
            self.pooled
                .iter()
                .map(|v| (v / 6.0 * 255.0).round().clamp(0.0, 255.0) as u8),
        );
        out.extend(
            self.scores
                .iter()
                .map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8),
        );
        out
    }

    /// The numbers back from [`Embedding::encode`]; None for another version or length.
    pub fn decode(bytes: &[u8]) -> Option<Embedding> {
        if bytes.len() != EMBEDDING_ENCODED_LEN || bytes[0] != 1 {
            return None;
        }
        let (pooled, scores) = bytes[1..].split_at(EMBEDDING_LEN);
        Some(Embedding {
            pooled: pooled.iter().map(|b| f32::from(*b) / 255.0 * 6.0).collect(),
            scores: scores.iter().map(|b| f32::from(*b) / 255.0).collect(),
        })
    }
}

fn relu6(x: f32) -> f32 {
    x.clamp(0.0, 6.0)
}

/// TensorFlow's SAME padding: the output size, and how much is added before the first element.
fn same(input: usize, stride: usize, kernel: usize) -> (usize, usize) {
    let out = input.div_ceil(stride);
    let total = ((out - 1) * stride + kernel).saturating_sub(input);
    (out, total / 2)
}

impl Yamnet {
    /// Load the model from the bytes of `yamnet-v1.bin`.
    pub fn from_bytes(bytes: &[u8]) -> Result<Yamnet, ModelError> {
        let mut r = Reader { bytes, at: 0 };
        if r.take(4)? != b"YAMN" {
            return Err(ModelError::NotAModel);
        }
        match r.u32()? {
            1 => {}
            v => return Err(ModelError::Version(v)),
        }
        let window = r.f32s(WINDOW)?;
        let matrix = r.f32s(MELS * BINS)?;
        let mel_offset = r.f32s(1)?[0];
        let mel = (0..MELS)
            .map(|m| {
                let row = &matrix[m * BINS..(m + 1) * BINS];
                let first = row.iter().position(|v| *v != 0.0).unwrap_or(0);
                let last = row.iter().rposition(|v| *v != 0.0).unwrap_or(0);
                (first, row[first..=last.max(first)].to_vec())
            })
            .collect();

        // The first convolution: weights come as [output][tap], used as [tap][output].
        let (w, bias) = r.layer(FIRST_CHANNELS, 9)?;
        let mut first = vec![0f32; 9 * FIRST_CHANNELS];
        for co in 0..FIRST_CHANNELS {
            for tap in 0..9 {
                first[tap * FIRST_CHANNELS + co] = w[co * 9 + tap];
            }
        }
        let first = Conv {
            w: first,
            bias,
            stride: 2,
        };

        let mut blocks = Vec::with_capacity(BLOCKS.len());
        let mut channels = FIRST_CHANNELS;
        for (stride, out) in BLOCKS {
            // Depthwise: [channel][tap] -> [tap][channel].
            let (w, bias) = r.layer(channels, 9)?;
            let mut dw = vec![0f32; 9 * channels];
            for c in 0..channels {
                for tap in 0..9 {
                    dw[tap * channels + c] = w[c * 9 + tap];
                }
            }
            let depthwise = Conv {
                w: dw,
                bias,
                stride,
            };
            // Pointwise: [output][input] -> [input][output].
            let (w, bias) = r.layer(out, channels)?;
            let mut pw = vec![0f32; channels * out];
            for co in 0..out {
                for ci in 0..channels {
                    pw[ci * out + co] = w[co * channels + ci];
                }
            }
            blocks.push((
                depthwise,
                Conv {
                    w: pw,
                    bias,
                    stride: 1,
                },
            ));
            channels = out;
        }
        let (classifier, classifier_bias) = r.layer(CLASSES, EMBEDDING_LEN)?;
        Ok(Yamnet {
            window,
            mel,
            mel_offset,
            first,
            blocks,
            classifier,
            classifier_bias,
        })
    }

    /// The log-mel spectrogram of one patch: [96 frames][64 bands].
    fn log_mel(&self, patch: &[f32]) -> Vec<f32> {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT);
        let mut input = fft.make_input_vec();
        let mut output = fft.make_output_vec();
        let mut mag = vec![0f32; BINS];
        let mut out = vec![0f32; FRAMES * MELS];
        for t in 0..FRAMES {
            let frame = &patch[t * HOP..t * HOP + WINDOW];
            for (i, slot) in input.iter_mut().enumerate() {
                *slot = if i < WINDOW {
                    frame[i] * self.window[i]
                } else {
                    0.0
                };
            }
            // The buffers come from the plan, so their lengths are right.
            fft.process(&mut input, &mut output)
                .expect("fft buffers have the plan's lengths");
            for (m, c) in mag.iter_mut().zip(&output) {
                *m = (c.re * c.re + c.im * c.im).sqrt();
            }
            for (band, (first, weights)) in self.mel.iter().enumerate() {
                let e: f32 = weights.iter().zip(&mag[*first..]).map(|(w, m)| w * m).sum();
                out[t * MELS + band] = (e + self.mel_offset).ln();
            }
        }
        out
    }

    fn first_conv(&self, logmel: &[f32]) -> Map {
        let (oh, pt) = same(FRAMES, 2, 3);
        let (ow, pl) = same(MELS, 2, 3);
        let c = FIRST_CHANNELS;
        let mut data = vec![0f32; oh * ow * c];
        for oy in 0..oh {
            for ox in 0..ow {
                let acc = &mut data[(oy * ow + ox) * c..(oy * ow + ox + 1) * c];
                acc.copy_from_slice(&self.first.bias);
                for ky in 0..3 {
                    let Some(iy) = (oy * 2 + ky).checked_sub(pt).filter(|y| *y < FRAMES) else {
                        continue;
                    };
                    for kx in 0..3 {
                        let Some(ix) = (ox * 2 + kx).checked_sub(pl).filter(|x| *x < MELS) else {
                            continue;
                        };
                        let v = logmel[iy * MELS + ix];
                        let w = &self.first.w[(ky * 3 + kx) * c..(ky * 3 + kx + 1) * c];
                        for (a, w) in acc.iter_mut().zip(w) {
                            *a += v * w;
                        }
                    }
                }
                for a in acc.iter_mut() {
                    *a = relu6(*a);
                }
            }
        }
        Map {
            h: oh,
            w: ow,
            c,
            data,
        }
    }

    fn depthwise(conv: &Conv, x: &Map) -> Map {
        let (oh, pt) = same(x.h, conv.stride, 3);
        let (ow, pl) = same(x.w, conv.stride, 3);
        let c = x.c;
        let mut data = vec![0f32; oh * ow * c];
        for oy in 0..oh {
            for ox in 0..ow {
                let acc = &mut data[(oy * ow + ox) * c..(oy * ow + ox + 1) * c];
                acc.copy_from_slice(&conv.bias);
                for ky in 0..3 {
                    let Some(iy) = (oy * conv.stride + ky).checked_sub(pt).filter(|y| *y < x.h)
                    else {
                        continue;
                    };
                    for kx in 0..3 {
                        let Some(ix) = (ox * conv.stride + kx).checked_sub(pl).filter(|v| *v < x.w)
                        else {
                            continue;
                        };
                        let input = &x.data[(iy * x.w + ix) * c..(iy * x.w + ix + 1) * c];
                        let w = &conv.w[(ky * 3 + kx) * c..(ky * 3 + kx + 1) * c];
                        for ((a, v), w) in acc.iter_mut().zip(input).zip(w) {
                            *a += v * w;
                        }
                    }
                }
                for a in acc.iter_mut() {
                    *a = relu6(*a);
                }
            }
        }
        Map {
            h: oh,
            w: ow,
            c,
            data,
        }
    }

    fn pointwise(conv: &Conv, x: &Map, out_channels: usize) -> Map {
        let mut data = vec![0f32; x.h * x.w * out_channels];
        for p in 0..x.h * x.w {
            let input = &x.data[p * x.c..(p + 1) * x.c];
            let acc = &mut data[p * out_channels..(p + 1) * out_channels];
            acc.copy_from_slice(&conv.bias);
            for (ci, v) in input.iter().enumerate() {
                // After a ReLU about half the inputs are exactly zero and add nothing.
                if *v == 0.0 {
                    continue;
                }
                let w = &conv.w[ci * out_channels..(ci + 1) * out_channels];
                for (a, w) in acc.iter_mut().zip(w) {
                    *a += v * w;
                }
            }
            for a in acc.iter_mut() {
                *a = relu6(*a);
            }
        }
        Map {
            h: x.h,
            w: x.w,
            c: out_channels,
            data,
        }
    }

    /// One patch of exactly [`PATCH_SAMPLES`] samples at 16 kHz.
    pub fn patch(&self, samples: &[f32]) -> Embedding {
        assert_eq!(
            samples.len(),
            PATCH_SAMPLES,
            "a patch is {PATCH_SAMPLES} samples"
        );
        let mut x = self.first_conv(&self.log_mel(samples));
        for (i, (dw, pw)) in self.blocks.iter().enumerate() {
            x = Self::depthwise(dw, &x);
            x = Self::pointwise(pw, &x, BLOCKS[i].1);
        }
        let n = (x.h * x.w) as f32;
        let mut pooled = vec![0f32; x.c];
        for p in 0..x.h * x.w {
            for (s, v) in pooled.iter_mut().zip(&x.data[p * x.c..(p + 1) * x.c]) {
                *s += v / n;
            }
        }
        let scores = (0..CLASSES)
            .map(|k| {
                let z = self.classifier_bias[k]
                    + self.classifier[k * EMBEDDING_LEN..(k + 1) * EMBEDDING_LEN]
                        .iter()
                        .zip(&pooled)
                        .map(|(w, v)| w * v)
                        .sum::<f32>();
                1.0 / (1.0 + (-z).exp())
            })
            .collect();
        Embedding { pooled, scores }
    }

    /// The mean embedding over patches of `samples` (16 kHz mono), one every `hop` samples.
    /// None when there is less than one patch.
    pub fn clip(&self, samples: &[f32], hop: usize) -> Option<Embedding> {
        let hop = hop.max(1);
        let mut pooled = vec![0f32; EMBEDDING_LEN];
        let mut scores = vec![0f32; CLASSES];
        let mut n = 0u32;
        let mut start = 0;
        while start + PATCH_SAMPLES <= samples.len() {
            let e = self.patch(&samples[start..start + PATCH_SAMPLES]);
            pooled.iter_mut().zip(&e.pooled).for_each(|(s, v)| *s += v);
            scores.iter_mut().zip(&e.scores).for_each(|(s, v)| *s += v);
            n += 1;
            start += hop;
        }
        if n == 0 {
            return None;
        }
        let k = n as f32;
        pooled.iter_mut().for_each(|v| *v /= k);
        scores.iter_mut().for_each(|v| *v /= k);
        Some(Embedding { pooled, scores })
    }
}

/// Resample mono `samples` from `rate` to [`RATE`] with a windowed-sinc filter (a low-pass at the
/// new Nyquist when going down), which is all the model needs. The filter is tabulated once and
/// read by linear interpolation, so each output sample costs a few dozen multiplications.
pub fn resample_to_model_rate(samples: &[f32], rate: u32) -> Vec<f32> {
    if rate == RATE {
        return samples.to_vec();
    }
    const HALF_TAPS: f64 = 16.0;
    const STEPS: usize = 256; // table entries per input sample
    let ratio = f64::from(rate) / f64::from(RATE); // input samples per output sample
    let cutoff = (1.0 / ratio).min(1.0) * 0.95; // the share of the input's Nyquist kept
    let span = (HALF_TAPS / cutoff).ceil() as isize; // taps each side of the centre, in input samples
    let pi = std::f64::consts::PI;
    let kernel = |d: f64| -> f64 {
        let t = d / span as f64;
        if t.abs() >= 1.0 {
            return 0.0;
        }
        let x = d * cutoff;
        let sinc = if x == 0.0 {
            1.0
        } else {
            (pi * x).sin() / (pi * x)
        };
        sinc * (0.5 + 0.5 * (pi * t).cos()) * cutoff
    };
    let table: Vec<f32> = (0..=2 * span as usize * STEPS)
        .map(|i| kernel(i as f64 / STEPS as f64 - span as f64) as f32)
        .collect();
    let at = |d: f64| -> f32 {
        let p = (d + span as f64) * STEPS as f64;
        let i = p.floor() as usize;
        let f = (p - i as f64) as f32;
        let (a, b) = (
            table[i.min(table.len() - 1)],
            table[(i + 1).min(table.len() - 1)],
        );
        a + (b - a) * f
    };
    let out_len = (samples.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let centre = i as f64 * ratio;
        let base = centre.floor() as isize;
        let (mut sum, mut norm) = (0f32, 0f32);
        for j in base - span + 1..=base + span {
            let k = at(j as f64 - centre);
            norm += k;
            if let Some(s) = usize::try_from(j).ok().and_then(|j| samples.get(j)) {
                sum += s * k;
            }
        }
        out.push(sum / norm);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: &[u8] = include_bytes!("../model/yamnet-v1.bin");

    fn tone(freq: f32, seconds: f32, rate: u32, amp: f32) -> Vec<f32> {
        (0..(seconds * rate as f32) as usize)
            .map(|i| amp * (2.0 * std::f32::consts::PI * freq * i as f32 / rate as f32).sin())
            .collect()
    }

    #[test]
    fn the_model_file_loads_and_bad_files_are_refused() {
        assert!(Yamnet::from_bytes(MODEL).is_ok());
        assert_eq!(
            Yamnet::from_bytes(b"YA").unwrap_err(),
            ModelError::Truncated
        );
        assert_eq!(
            Yamnet::from_bytes(b"nope").unwrap_err(),
            ModelError::NotAModel
        );
        assert_eq!(
            Yamnet::from_bytes(&[b'X'; 64]).unwrap_err(),
            ModelError::NotAModel
        );
        let mut other = MODEL.to_vec();
        other[4] = 9;
        assert_eq!(
            Yamnet::from_bytes(&other).unwrap_err(),
            ModelError::Version(9)
        );
        assert_eq!(
            Yamnet::from_bytes(&MODEL[..MODEL.len() - 100]).unwrap_err(),
            ModelError::Truncated
        );
    }

    #[test]
    fn a_patch_gives_finite_numbers_in_range_and_the_same_every_time() {
        let model = Yamnet::from_bytes(MODEL).unwrap();
        let audio = tone(440.0, 1.0, RATE, 0.3);
        let a = model.patch(&audio[..PATCH_SAMPLES]);
        assert_eq!((a.pooled.len(), a.scores.len()), (EMBEDDING_LEN, CLASSES));
        assert!(
            a.pooled
                .iter()
                .all(|v| v.is_finite() && (0.0..=6.0).contains(v))
        );
        assert!(
            a.scores
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        );
        assert_eq!(a, model.patch(&audio[..PATCH_SAMPLES]));
    }

    #[test]
    fn different_sounds_have_different_embeddings() {
        let model = Yamnet::from_bytes(MODEL).unwrap();
        let cosine = |a: &[f32], b: &[f32]| {
            let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
            dot / (a.iter().map(|x| x * x).sum::<f32>().sqrt()
                * b.iter().map(|x| x * x).sum::<f32>().sqrt())
        };
        let low = model
            .clip(&tone(220.0, 4.0, RATE, 0.3), PATCH_SAMPLES)
            .unwrap();
        let louder = model
            .clip(&tone(220.0, 4.0, RATE, 0.6), PATCH_SAMPLES)
            .unwrap();
        let high = model
            .clip(&tone(3_000.0, 4.0, RATE, 0.3), PATCH_SAMPLES)
            .unwrap();
        assert!(
            cosine(&low.pooled, &louder.pooled) > cosine(&low.pooled, &high.pooled),
            "the same note at two levels is closer than two notes"
        );
        assert!(
            model
                .clip(&tone(220.0, 0.5, RATE, 0.3), PATCH_SAMPLES)
                .is_none(),
            "under one patch"
        );
    }

    #[test]
    fn resampling_keeps_the_pitch_and_the_level() {
        for rate in [22_050, 44_100, 48_000] {
            let out = resample_to_model_rate(&tone(1_000.0, 2.0, rate, 0.5), rate);
            assert!(
                (out.len() as i64 - 32_000).abs() <= 1,
                "{rate} Hz gave {} samples",
                out.len()
            );
            let crossings = out.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
            assert!(
                (1_998..=2_002).contains(&crossings),
                "{rate} Hz: {crossings} cycles of 1 kHz in 2 s"
            );
            let peak = out[200..out.len() - 200]
                .iter()
                .fold(0f32, |m, v| m.max(v.abs()));
            assert!((peak - 0.5).abs() < 0.02, "{rate} Hz: peak {peak}");
        }
        // A tone above the new Nyquist is filtered out, not folded down into the band.
        let out = resample_to_model_rate(&tone(10_000.0, 1.0, 48_000, 0.5), 48_000);
        let peak = out[200..out.len() - 200]
            .iter()
            .fold(0f32, |m, v| m.max(v.abs()));
        assert!(
            peak < 0.05,
            "a 10 kHz tone at 16 kHz sampling leaked at {peak}"
        );
    }

    #[test]
    fn the_stored_form_keeps_the_numbers_to_within_a_byte() {
        let model = Yamnet::from_bytes(MODEL).unwrap();
        let e = model
            .clip(&tone(330.0, 3.0, RATE, 0.3), PATCH_SAMPLES)
            .unwrap();
        let bytes = e.encode();
        assert_eq!(bytes.len(), EMBEDDING_ENCODED_LEN);
        let back = Embedding::decode(&bytes).unwrap();
        assert!(
            e.pooled
                .iter()
                .zip(&back.pooled)
                .all(|(a, b)| (a - b).abs() <= 0.013)
        );
        assert!(
            e.scores
                .iter()
                .zip(&back.scores)
                .all(|(a, b)| (a - b).abs() <= 0.003)
        );
        assert!(Embedding::decode(&bytes[1..]).is_none());
        let mut other = bytes.clone();
        other[0] = 2;
        assert!(Embedding::decode(&other).is_none());
    }

    /// The waveform `tools/sound-spike/reference_yamnet.py` feeds the original network: three tones
    /// and a little noise from a 64-bit linear congruential generator.
    fn reference_wave(n: usize) -> Vec<f32> {
        let mut state = 12_345u64;
        (0..n)
            .map(|i| {
                let t = i as f64 / 16_000.0;
                let pi2 = 2.0 * std::f64::consts::PI;
                let tones = 0.25 * (pi2 * 220.0 * t).sin()
                    + 0.15 * (pi2 * 1_318.5 * t).sin()
                    + 0.10 * (pi2 * (300.0 * t + 100.0 * t * t)).sin();
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                let noise = (state >> 40) as f64 / f64::from(1u32 << 24) * 2.0 - 1.0;
                (tones + 0.05 * noise) as f32
            })
            .collect()
    }

    fn cosine(a: &[f32], b: &[f32]) -> f64 {
        let dot: f64 = a
            .iter()
            .zip(b)
            .map(|(x, y)| f64::from(*x) * f64::from(*y))
            .sum();
        let na: f64 = a.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
        let nb: f64 = b.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
        dot / (na * nb)
    }

    /// The Rust network against the original TFLite network (int8, from CI) on the same audio.
    /// The original is integer quantised, so the two differ by that rounding; they agree on what
    /// the sound is.
    #[test]
    fn the_rust_network_agrees_with_the_original() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../model/yamnet-reference.json")).unwrap();
        let wave = reference_wave(15_600 * 2);
        let model = Yamnet::from_bytes(MODEL).unwrap();
        for patch in reference["patches"].as_array().unwrap() {
            let start = patch["start"].as_u64().unwrap() as usize;
            let numbers = |key: &str| -> Vec<f32> {
                patch[key]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_f64().unwrap() as f32)
                    .collect()
            };
            let (want_pooled, want_scores) = (numbers("pooled"), numbers("scores"));
            let got = model.patch(&wave[start..start + PATCH_SAMPLES]);
            let c_pooled = cosine(&got.pooled, &want_pooled);
            let c_scores = cosine(&got.scores, &want_scores);
            let mean_abs: f32 = got
                .pooled
                .iter()
                .zip(&want_pooled)
                .map(|(a, b)| (a - b).abs())
                .sum::<f32>()
                / 1024.0;
            eprintln!(
                "patch at {start}: embedding cosine {c_pooled:.4}, mean error {mean_abs:.4}, scores cosine {c_scores:.4}"
            );
            assert!(
                c_pooled > 0.995,
                "the embedding at {start}: cosine {c_pooled}"
            );
            assert!(c_scores > 0.97, "the scores at {start}: cosine {c_scores}");
        }
    }

    #[test]
    fn how_long_a_clip_takes() {
        let model = Yamnet::from_bytes(MODEL).unwrap();
        let audio = tone(330.0, 30.0, RATE, 0.3);
        let started = std::time::Instant::now();
        let e = model.clip(&audio, PATCH_SAMPLES / 2).unwrap();
        eprintln!(
            "30 s clip, 61 patches: {} ms",
            started.elapsed().as_millis()
        );
        assert_eq!(e.pooled.len(), EMBEDDING_LEN);
    }
}
