//! Frames of a clip: spectra, mel bands, chroma and a few per-frame measures.

use realfft::RealFftPlanner;

/// The analysis window, in samples, and the step between windows.
pub(crate) const FFT: usize = 2048;
pub(crate) const HOP: usize = 512;
/// Mel bands per frame; the texture groups them in pairs.
pub(crate) const MELS: usize = 32;

const MEL_LOW_HZ: f32 = 30.0;
const MEL_HIGH_HZ: f32 = 16_000.0;
/// The pitch range read, and how far below the frame's strongest bin a peak may sit (-25 dB;
/// weaker peaks in a Hann window are mostly side lobes).
const CHROMA_LOW_HZ: f32 = 55.0;
const CHROMA_HIGH_HZ: f32 = 5000.0;
const PEAK_FLOOR: f32 = 0.003_162;

/// What each frame of the clip measured.
pub(crate) struct Frames {
    /// Frames per second.
    pub frame_rate: f32,
    /// Mel band energies in dB.
    pub mel_db: Vec<[f32; MELS]>,
    /// Power per pitch class (C = 0).
    pub chroma: Vec<[f32; 12]>,
    pub centroid_hz: Vec<f32>,
    /// 0 (tonal) to 1 (noise-like).
    pub flatness: Vec<f32>,
    /// Zero crossings per sample.
    pub zcr: Vec<f32>,
    /// Total power in dB.
    pub total_db: Vec<f32>,
}

fn hz_to_mel(hz: f32) -> f32 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

fn mel_to_hz(mel: f32) -> f32 {
    700.0 * (10f32.powf(mel / 2595.0) - 1.0)
}

/// Triangular filters over the spectrum's bins: for each mel band, the bins and their weights.
fn mel_filters(rate: u32) -> Vec<Vec<(usize, f32)>> {
    let nyquist = rate as f32 / 2.0;
    let high = MEL_HIGH_HZ.min(nyquist * 0.99);
    let (lo, hi) = (hz_to_mel(MEL_LOW_HZ), hz_to_mel(high));
    let edges: Vec<f32> = (0..MELS + 2)
        .map(|i| mel_to_hz(lo + (hi - lo) * i as f32 / (MELS + 1) as f32))
        .collect();
    let bin_hz = rate as f32 / FFT as f32;
    (0..MELS)
        .map(|b| {
            let (left, centre, right) = (edges[b], edges[b + 1], edges[b + 2]);
            let first = ((left / bin_hz).floor() as usize).max(1);
            let last = ((right / bin_hz).ceil() as usize).min(FFT / 2);
            let mut bins = Vec::new();
            for k in first..=last {
                let f = k as f32 * bin_hz;
                let w = if f < left || f > right {
                    0.0
                } else if f <= centre {
                    (f - left) / (centre - left).max(1e-6)
                } else {
                    (right - f) / (right - centre).max(1e-6)
                };
                if w > 0.0 {
                    bins.push((k, w));
                }
            }
            // A band narrower than a bin (at the low end) still takes the nearest bin.
            if bins.is_empty() {
                bins.push((((centre / bin_hz).round() as usize).clamp(1, FFT / 2), 1.0));
            }
            bins
        })
        .collect()
}

/// Frames of mono samples at `rate`. The caller has checked the clip is long enough.
pub(crate) fn frames(samples: &[f32], rate: u32) -> Frames {
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT);
    let mut input = fft.make_input_vec();
    let mut output = fft.make_output_vec();
    let window: Vec<f32> = (0..FFT)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / FFT as f32).cos())
        .collect();
    let filters = mel_filters(rate);
    let bin_hz = rate as f32 / FFT as f32;
    let scale = 1.0 / (FFT as f32 * FFT as f32);

    let count = (samples.len() - FFT) / HOP + 1;
    let mut out = Frames {
        frame_rate: rate as f32 / HOP as f32,
        mel_db: Vec::with_capacity(count),
        chroma: Vec::with_capacity(count),
        centroid_hz: Vec::with_capacity(count),
        flatness: Vec::with_capacity(count),
        zcr: Vec::with_capacity(count),
        total_db: Vec::with_capacity(count),
    };
    let mut power = vec![0f32; FFT / 2 + 1];
    for t in 0..count {
        let frame = &samples[t * HOP..t * HOP + FFT];
        for (i, (x, w)) in frame.iter().zip(&window).enumerate() {
            input[i] = x * w;
        }
        // The input is the right length and the output buffer came from the plan.
        fft.process(&mut input, &mut output)
            .expect("fft buffers have the plan's lengths");
        for (p, c) in power.iter_mut().zip(&output) {
            *p = (c.re * c.re + c.im * c.im) * scale;
        }

        let mut mel = [0f32; MELS];
        for (m, bins) in mel.iter_mut().zip(&filters) {
            let e: f32 = bins.iter().map(|(k, w)| power[*k] * w).sum();
            *m = 10.0 * (e + 1e-10).log10();
        }
        out.mel_db.push(mel);

        let mut chroma = [0f32; 12];
        let (mut mag_sum, mut weighted, mut log_sum, mut total) = (0f32, 0f32, 0f32, 0f32);
        for (k, p) in power.iter().copied().enumerate().skip(1) {
            total += p;
            let mag = p.sqrt();
            mag_sum += mag;
            weighted += mag * k as f32 * bin_hz;
            log_sum += (mag + 1e-9).ln();
        }
        // Pitch comes from the spectrum's peaks, placed between bins by a parabola through the
        // peak and its neighbours: bins 21 Hz apart are wider than a semitone below about 300 Hz,
        // so assigning whole bins to pitch classes smears every bass note over its neighbours.
        let top = power.iter().copied().fold(0f32, f32::max);
        for (i, w) in power.windows(3).enumerate().skip(1) {
            let k = i + 1;
            let (a, b, c) = (w[0], w[1], w[2]);
            if b <= a || b < c || b < top * PEAK_FLOOR {
                continue;
            }
            let (la, lb, lc) = ((a + 1e-20).ln(), (b + 1e-20).ln(), (c + 1e-20).ln());
            let denom = la - 2.0 * lb + lc;
            let shift = if denom.abs() > 1e-9 {
                (0.5 * (la - lc) / denom).clamp(-0.5, 0.5)
            } else {
                0.0
            };
            let f = (k as f32 + shift) * bin_hz;
            if (CHROMA_LOW_HZ..=CHROMA_HIGH_HZ).contains(&f) {
                let midi = (12.0 * (f / 440.0).log2() + 69.0).round() as i64;
                chroma[midi.rem_euclid(12) as usize] += b;
            }
        }
        out.chroma.push(chroma);
        out.centroid_hz.push(if mag_sum > 1e-9 {
            weighted / mag_sum
        } else {
            0.0
        });
        let n = (FFT / 2) as f32;
        out.flatness
            .push(((log_sum / n).exp() / (mag_sum / n + 1e-9)).clamp(0.0, 1.0));
        out.total_db.push(10.0 * (total + 1e-10).log10());
        let crossings = frame
            .windows(2)
            .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
            .count();
        out.zcr.push(crossings as f32 / FFT as f32);
    }
    out
}
