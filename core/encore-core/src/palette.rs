//! Album gradients (board 04): a colour palette made once from the cover.
//!
//! 1. Sample the cover at 64 x 64 and cluster it into five colours (k-means in
//!    OKLab).
//! 2. Rank the clusters by population and chroma, drop near-duplicates, keep four,
//!    then add a base tinted with the leading hue.
//! 3. Keep text readable: lower lightness until white text reaches 4.5:1; light art
//!    flips to a light field with dark text at 7:1.
//!
//! **Deterministic on every platform**: the same cover gives the same bytes on
//! every device, the server included (the year recap relies on that later). So
//! there is no platform maths library here: sRGB to linear is a fixed table,
//! the cube root is a fixed number of Newton steps, and everything else is IEEE
//! `+ - * /` and `sqrt`, which every platform rounds the same way. Colours go back
//! to 8-bit through the same table. k-means starts from a fixed seed and breaks
//! every tie by order. docs/adr/core-0007-palettes.md.

use serde::{Deserialize, Serialize};

/// Starting values from `design/tokens.json` (`albumGradient`).
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub sample_size_px: u32,
    pub clusters: usize,
    pub stops: usize,
    pub rank_chroma_weight: f64,
    pub duplicate_distance: f64,
    pub light_art_min_lightness: f64,
    pub dark_base_lightness: f64,
    pub dark_base_max_chroma: f64,
    pub light_base_lightness: f64,
    pub light_base_max_chroma: f64,
    /// On a light field every stop is at least this light...
    pub light_field_min_lightness: f64,
    /// ...and at most this colourful: light art gets a soft, pale field (board 04).
    pub light_field_max_chroma: f64,
    pub max_iterations: usize,
    pub text_contrast_min: f64,
    pub light_art_text_contrast_min: f64,
    pub lightness_step: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Scheme {
    /// A dark field with white text.
    Dark,
    /// Light art: a light field with dark text.
    Light,
}

/// One sampled cluster: its colour and its share of the cover (0 to 1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Swatch {
    pub colour: String,
    pub share: f64,
}

/// Colours are `RRGGBB` hex, upper case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    pub scheme: Scheme,
    /// The field's base: dark (or light, for light art), tinted with the leading hue.
    pub base: String,
    /// Four stops for the moving field, leading colour first. The mini player and
    /// album pages take their tint from the first.
    pub stops: Vec<String>,
    /// The text colour every colour above was checked against.
    pub text: String,
    /// The lowest contrast between `text` and any colour above, two decimals.
    pub lowest_contrast: f64,
    /// The clusters as sampled, largest first (board 04 shows them).
    pub sampled: Vec<Swatch>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PaletteError {
    #[error("the cover couldn't be read: {0}")]
    Unreadable(String),
    #[error("the cover is too large")]
    TooLarge,
}

// ---------------------------------------------------------------- colour maths

/// sRGB 8-bit to linear light, `((c + 0.055) / 1.055) ^ 2.4` (or `c / 12.92`),
/// computed once and written out so no platform's `pow` is involved.
#[rustfmt::skip]
const SRGB_TO_LINEAR: [f64; 256] = [
    0.0, 0.0003035269835488375, 0.000607053967097675, 0.0009105809506465125,
    0.00121410793419535, 0.0015176349177441874, 0.001821161901293025, 0.0021246888848418626,
    0.0024282158683907, 0.0027317428519395373, 0.003035269835488375, 0.003346535763899161,
    0.003676507324047436, 0.004024717018496307, 0.004391442037410293, 0.004776953480693729,
    0.005181516702338386, 0.005605391624202723, 0.006048833022857054, 0.006512090792594475,
    0.006995410187265387, 0.007499032043226175, 0.008023192985384994, 0.008568125618069307,
    0.009134058702220787, 0.00972121732023785, 0.010329823029626936, 0.010960094006488246,
    0.011612245179743885, 0.012286488356915872, 0.012983032342173012, 0.013702083047289686,
    0.014443843596092545, 0.01520851442291271, 0.01599629336550963, 0.016807375752887384,
    0.017641954488384078, 0.018500220128379697, 0.019382360956935723, 0.0202885630566524,
    0.021219010376003555, 0.02217388479338738, 0.02315336617811041, 0.024157632448504756,
    0.02518685962736163, 0.026241221894849898, 0.027320891639074894, 0.028426039504420793,
    0.0295568344378088, 0.030713443732993635, 0.03189603307301153, 0.033104766570885055,
    0.03433980680868217, 0.03560131487502034, 0.03688945040110004, 0.0382043715953465,
    0.03954623527673284, 0.04091519690685319, 0.042311410620809675, 0.043735029256973465,
    0.04518620438567554, 0.046665086336880095, 0.04817182422688942, 0.04970656598412723,
    0.05126945837404324, 0.052860647023180246, 0.05448027644244237, 0.05612849004960009,
    0.05780543019106723, 0.0595112381629812, 0.06124605423161761, 0.06301001765316767,
    0.06480326669290577, 0.06662593864377289, 0.06847816984440017, 0.07036009569659588,
    0.07227185068231748, 0.07421356838014963, 0.07618538148130785, 0.07818742180518633,
    0.08021982031446832, 0.0822827071298148, 0.08437621154414882, 0.08650046203654976,
    0.08865558628577294, 0.09084171118340768, 0.09305896284668745, 0.0953074666309647,
    0.09758734714186246, 0.09989872824711389, 0.10224173308810132, 0.10461648409110419,
    0.10702310297826761, 0.10946171077829933, 0.1119324278369056, 0.11443537382697373,
    0.11697066775851084, 0.11953842798834562, 0.12213877222960187, 0.12477181756095049,
    0.12743768043564743, 0.1301364766903643, 0.13286832155381798, 0.13563332965520566,
    0.13843161503245183, 0.14126329114027164, 0.14412847085805777, 0.14702726649759498,
    0.14995978981060856, 0.15292615199615017, 0.1559264637078274, 0.1589608350608804,
    0.162029375639111, 0.1651321945016676, 0.16826940018969075, 0.1714411007328226,
    0.17464740365558504, 0.17788841598362912, 0.18116424424986022, 0.184474994500441,
    0.18782077230067787, 0.19120168274079138, 0.1946178304415758, 0.19806931955994886,
    0.20155625379439707, 0.20507873639031693, 0.20863687014525575, 0.21223075741405523,
    0.21586050011389926, 0.2195261997292692, 0.2232279573168085, 0.22696587351009836,
    0.23074004852434915, 0.23455058216100522, 0.238397573812271, 0.24228112246555486,
    0.24620132670783548, 0.25015828472995344, 0.25415209433082675, 0.2581828529215958,
    0.26225065752969623, 0.26635560480286247, 0.2704977910130658, 0.27467731206038465,
    0.2788942634768104, 0.2831487404299921, 0.2874408377269175, 0.29177064981753587,
    0.2961382707983211, 0.3005437944157765, 0.3049873140698863, 0.30946892281750854,
    0.31398871337571754, 0.31854677812509186, 0.32314320911295075, 0.3277780980565422,
    0.33245153634617935, 0.33716361504833037, 0.3419144249086609, 0.3467040563550296,
    0.35153259950043936, 0.3564001441459435, 0.3613067797835095, 0.3662525955988395,
    0.3712376804741491, 0.3762621229909065, 0.38132601143253014, 0.386429433787049,
    0.39157247774972326, 0.39675523072562685, 0.4019777798321958, 0.4072402119017367,
    0.41254261348390375, 0.4178850708481375, 0.4232676699860717, 0.4286904966139066,
    0.43415363617474895, 0.4396571738409188, 0.44520119451622786, 0.45078578283822346,
    0.45641102318040466, 0.4620769996544071, 0.467783796112159, 0.47353149614800955,
    0.4793201831008268, 0.4851499400560704, 0.4910208498478356, 0.4969329950608704,
    0.5028864580325687, 0.5088813208549338, 0.5149176653765214, 0.5209955732043543,
    0.5271151257058131, 0.5332764040105052, 0.5394794890121072, 0.5457244613701866,
    0.5520114015120001, 0.5583403896342679, 0.5647115057049292, 0.5711248294648731,
    0.5775804404296506, 0.5840784178911641, 0.5906188409193369, 0.5972017883637634,
    0.6038273388553378, 0.6104955708078648, 0.6172065624196511, 0.6239603916750761,
    0.6307571363461468, 0.6375968739940326, 0.6444796819705821, 0.6514056374198242,
    0.6583748172794485, 0.665387298282272, 0.6724431569576875, 0.6795424696330938,
    0.6866853124353135, 0.6938717612919899, 0.7011018919329731, 0.7083757798916868,
    0.7156935005064807, 0.7230551289219693, 0.7304607400903537, 0.7379104087727308,
    0.7454042095403874, 0.7529422167760779, 0.7605245046752924, 0.768151147247507,
    0.7758222183174236, 0.7835377915261935, 0.7912979403326302, 0.799102738014409,
    0.8069522576692516, 0.8148465722161012, 0.8227857543962835, 0.8307698767746546,
    0.83879901174074, 0.846873231509858, 0.8549926081242338, 0.8631572134541023,
    0.8713671191987972, 0.8796223968878317, 0.8879231178819663, 0.8962693533742664,
    0.9046611743911496, 0.9130986517934192, 0.9215818562772946, 0.9301108583754237,
    0.938685728457888, 0.9473065367331999, 0.9559733532492861, 0.9646862478944651,
    0.9734452903984125, 0.9822505503331171, 0.9911020971138298, 1.0,
];

/// The nearest 8-bit sRGB value to linear light `x`, through the same table.
fn from_linear(x: f64) -> u8 {
    if x.is_nan() || x <= 0.0 {
        return 0;
    }
    if x >= 1.0 {
        return 255;
    }
    let (mut lo, mut hi) = (0usize, 255usize);
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if SRGB_TO_LINEAR[mid] <= x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    if x - SRGB_TO_LINEAR[lo] <= SRGB_TO_LINEAR[hi] - x {
        lo as u8
    } else {
        hi as u8
    }
}

/// Cube root from basic arithmetic only: a first guess from the exponent bits,
/// then a fixed number of Newton steps.
fn cbrt(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return if x.is_finite() { 0.0 } else { x };
    }
    let a = x.abs();
    let mut y = f64::from_bits(a.to_bits() / 3 + 0x2AA0_0000_0000_0000);
    for _ in 0..8 {
        y -= (y * y * y - a) / (3.0 * y * y);
    }
    if x < 0.0 { -y } else { y }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Lab {
    l: f64,
    a: f64,
    b: f64,
}

impl Lab {
    fn chroma(self) -> f64 {
        (self.a * self.a + self.b * self.b).sqrt()
    }

    fn dist2(self, o: Lab) -> f64 {
        let (dl, da, db) = (self.l - o.l, self.a - o.a, self.b - o.b);
        dl * dl + da * da + db * db
    }

    /// The same hue with its chroma limited to `max`.
    fn tint(self, l: f64, max_chroma: f64) -> Lab {
        let c = self.chroma();
        let s = if c > max_chroma && c > 0.0 {
            max_chroma / c
        } else {
            1.0
        };
        Lab {
            l,
            a: self.a * s,
            b: self.b * s,
        }
    }
}

/// OKLab (Björn Ottosson, 2020), from linear sRGB.
fn lab_of(rgb: [u8; 3]) -> Lab {
    let [r, g, b] = rgb.map(|c| SRGB_TO_LINEAR[c as usize]);
    let l = cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
    let m = cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
    let s = cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
    Lab {
        l: 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        a: 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        b: 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    }
}

/// Back to 8-bit sRGB; out-of-gamut channels are clipped.
fn rgb_of(c: Lab) -> [u8; 3] {
    let l = c.l + 0.3963377774 * c.a + 0.2158037573 * c.b;
    let m = c.l - 0.1055613458 * c.a - 0.0638541728 * c.b;
    let s = c.l - 0.0894841775 * c.a - 1.2914855480 * c.b;
    let (l, m, s) = (l * l * l, m * m * m, s * s * s);
    [
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    ]
    .map(from_linear)
}

/// WCAG relative luminance.
fn luminance(c: [u8; 3]) -> f64 {
    let [r, g, b] = c.map(|v| SRGB_TO_LINEAR[v as usize]);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// WCAG contrast ratio, 1 to 21.
fn contrast(x: [u8; 3], y: [u8; 3]) -> f64 {
    let (a, b) = (luminance(x), luminance(y));
    let (hi, lo) = if a > b { (a, b) } else { (b, a) };
    (hi + 0.05) / (lo + 0.05)
}

fn hex(c: [u8; 3]) -> String {
    format!("{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// Two decimals, rounded half away from zero, without `round()`'s platform call.
fn two_decimals(x: f64) -> f64 {
    let scaled = x * 100.0 + 0.5;
    (scaled as i64) as f64 / 100.0
}

// ---------------------------------------------------------------- pixels

/// Read a PNG into RGBA, 8 bits per channel.
pub fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), PaletteError> {
    let bad = |e: png::DecodingError| match e {
        png::DecodingError::LimitsExceeded => PaletteError::TooLarge,
        other => PaletteError::Unreadable(other.to_string()),
    };
    let mut decoder =
        png::Decoder::new_with_limits(std::io::Cursor::new(bytes), png::Limits { bytes: 32 << 20 });
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(bad)?;
    let size = reader.output_buffer_size().ok_or(PaletteError::TooLarge)?;
    let mut buf = vec![0; size];
    let info = reader.next_frame(&mut buf).map_err(bad)?;
    let (w, h) = (info.width, info.height);
    if w == 0 || h == 0 {
        return Err(PaletteError::Unreadable("the image is empty".into()));
    }
    let channels = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => {
            return Err(PaletteError::Unreadable(
                "indexed colour was not expanded".into(),
            ));
        }
    };
    let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
    for row in buf.chunks(info.line_size).take(h as usize) {
        for px in row.chunks(channels).take(w as usize) {
            let (r, g, b, a) = match channels {
                1 => (px[0], px[0], px[0], 255),
                2 => (px[0], px[0], px[0], px[1]),
                3 => (px[0], px[1], px[2], 255),
                _ => (px[0], px[1], px[2], px[3]),
            };
            rgba.extend_from_slice(&[r, g, b, a]);
        }
    }
    Ok((w, h, rgba))
}

/// The opaque pixels, averaged down in whole blocks so the longer side is at most
/// `max` (integer sums: exact everywhere). A block counts when most of it is opaque.
fn sample(w: u32, h: u32, rgba: &[u8], max: u32) -> Vec<[u8; 3]> {
    let (w, h) = (w as usize, h as usize);
    let step = w.max(h).div_ceil(max.max(1) as usize).max(1);
    let mut out = Vec::new();
    for by in (0..h).step_by(step) {
        for bx in (0..w).step_by(step) {
            let (mut sum, mut opaque, mut total) = ([0u64; 3], 0u64, 0u64);
            for y in by..(by + step).min(h) {
                for x in bx..(bx + step).min(w) {
                    let i = (y * w + x) * 4;
                    let Some(px) = rgba.get(i..i + 4) else {
                        continue;
                    };
                    total += 1;
                    if px[3] >= 128 {
                        opaque += 1;
                        for c in 0..3 {
                            sum[c] += u64::from(px[c]);
                        }
                    }
                }
            }
            if opaque > 0 && opaque * 2 >= total {
                out.push(sum.map(|s| ((s + opaque / 2) / opaque) as u8));
            }
        }
    }
    out
}

// ---------------------------------------------------------------- clustering

struct Point {
    lab: Lab,
    weight: f64,
    key: u32,
}

fn kmeans(points: &[Point], k: usize, max_iterations: usize) -> Vec<(Lab, f64)> {
    let k = k.min(points.len()).max(1);
    // First centre: the most populous 16-level colour bucket (ties: the lowest).
    let mut buckets: std::collections::BTreeMap<u32, (f64, f64, f64, f64)> = Default::default();
    for p in points {
        let bucket = (p.key >> 4) & 0x0F0F0F;
        let e = buckets.entry(bucket).or_insert((0.0, 0.0, 0.0, 0.0));
        e.0 += p.weight;
        e.1 += p.lab.l * p.weight;
        e.2 += p.lab.a * p.weight;
        e.3 += p.lab.b * p.weight;
    }
    let mut best: Option<(f64, Lab)> = None;
    for (w, l, a, b) in buckets.values() {
        if best.is_none_or(|(bw, _)| *w > bw) {
            best = Some((
                *w,
                Lab {
                    l: l / w,
                    a: a / w,
                    b: b / w,
                },
            ));
        }
    }
    let mut centres = vec![best.map(|(_, c)| c).unwrap_or(points[0].lab)];
    // The rest: the point that adds the most weighted distance (ties: the first).
    while centres.len() < k {
        let mut pick: Option<(f64, Lab)> = None;
        for p in points {
            let d = centres
                .iter()
                .map(|c| c.dist2(p.lab))
                .fold(f64::INFINITY, f64::min);
            let score = d * p.weight;
            if score > 0.0 && pick.is_none_or(|(s, _)| score > s) {
                pick = Some((score, p.lab));
            }
        }
        match pick {
            Some((_, lab)) => centres.push(lab),
            None => break,
        }
    }
    let mut assign = vec![usize::MAX; points.len()];
    for _ in 0..max_iterations.max(1) {
        let mut changed = false;
        for (i, p) in points.iter().enumerate() {
            let mut nearest = 0;
            let mut nd = f64::INFINITY;
            for (j, c) in centres.iter().enumerate() {
                let d = c.dist2(p.lab);
                if d < nd {
                    nd = d;
                    nearest = j;
                }
            }
            if assign[i] != nearest {
                assign[i] = nearest;
                changed = true;
            }
        }
        let mut sums = vec![(0.0, 0.0, 0.0, 0.0); centres.len()];
        for (i, p) in points.iter().enumerate() {
            let s = &mut sums[assign[i]];
            s.0 += p.weight;
            s.1 += p.lab.l * p.weight;
            s.2 += p.lab.a * p.weight;
            s.3 += p.lab.b * p.weight;
        }
        for (c, (w, l, a, b)) in centres.iter_mut().zip(&sums) {
            if *w > 0.0 {
                *c = Lab {
                    l: l / w,
                    a: a / w,
                    b: b / w,
                };
            }
        }
        if !changed {
            break;
        }
    }
    let mut weights = vec![0.0; centres.len()];
    for (i, p) in points.iter().enumerate() {
        weights[assign[i]] += p.weight;
    }
    let mut out: Vec<(Lab, f64)> = centres
        .into_iter()
        .zip(weights)
        .filter(|(_, w)| *w > 0.0)
        .collect();
    // Largest first; ties by lightness, then order.
    out.sort_by(|x, y| y.1.total_cmp(&x.1).then(x.0.l.total_cmp(&y.0.l)));
    out
}

// ---------------------------------------------------------------- the palette

/// Lower (dark field) or raise (light field) lightness until `text` on the colour
/// reaches `min` contrast.
fn readable(mut c: Lab, text: [u8; 3], min: f64, darker: bool, step: f64) -> [u8; 3] {
    let step = step.max(0.001);
    for _ in 0..2000 {
        let rgb = rgb_of(c);
        if contrast(rgb, text) >= min {
            return rgb;
        }
        if darker {
            c.l -= step;
            if c.l <= 0.0 {
                return [0, 0, 0];
            }
        } else {
            c.l += step;
            if c.l >= 1.0 {
                return [255, 255, 255];
            }
        }
    }
    if darker { [0, 0, 0] } else { [255, 255, 255] }
}

/// The palette of an RGBA image; None when it has no opaque pixels.
pub fn palette_of_rgba(w: u32, h: u32, rgba: &[u8], s: &Settings) -> Option<Palette> {
    let pixels = sample(w, h, rgba, s.sample_size_px);
    if pixels.is_empty() {
        return None;
    }
    // Identical colours once, with their count, in a fixed order.
    let mut counts: std::collections::BTreeMap<u32, u32> = Default::default();
    for p in &pixels {
        let key = (u32::from(p[0]) << 16) | (u32::from(p[1]) << 8) | u32::from(p[2]);
        *counts.entry(key).or_insert(0) += 1;
    }
    let total = pixels.len() as f64;
    let points: Vec<Point> = counts
        .into_iter()
        .map(|(key, n)| Point {
            lab: lab_of([(key >> 16) as u8, (key >> 8) as u8, key as u8]),
            weight: f64::from(n),
            key,
        })
        .collect();
    let clusters = kmeans(&points, s.clusters, s.max_iterations);

    // Rank by population and chroma; drop near-duplicates; keep `stops`.
    let mut ranked: Vec<(f64, usize)> = clusters
        .iter()
        .enumerate()
        .map(|(i, (lab, w))| ((w / total) * (1.0 + s.rank_chroma_weight * lab.chroma()), i))
        .collect();
    ranked.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
    let dup2 = s.duplicate_distance * s.duplicate_distance;
    let mut chosen: Vec<Lab> = Vec::new();
    for (_, i) in &ranked {
        let lab = clusters[*i].0;
        if chosen.len() < s.stops && chosen.iter().all(|c| c.dist2(lab) >= dup2) {
            chosen.push(lab);
        }
    }
    // Too few distinct colours (a flat cover): steps of the leading colour.
    let lead = chosen[0];
    let mut n = 1.0;
    while chosen.len() < s.stops.max(1) {
        let shift = if chosen.len() % 2 == 1 {
            -0.06 * n
        } else {
            0.06 * n
        };
        chosen.push(Lab {
            l: (lead.l + shift).clamp(0.0, 1.0),
            ..lead
        });
        n += 1.0;
    }

    let mean_l = clusters.iter().map(|(lab, w)| lab.l * w).sum::<f64>() / total;
    let (scheme, text, min, darker, base) = if mean_l >= s.light_art_min_lightness {
        (
            Scheme::Light,
            [0, 0, 0],
            s.light_art_text_contrast_min,
            false,
            lead.tint(s.light_base_lightness, s.light_base_max_chroma),
        )
    } else {
        (
            Scheme::Dark,
            [255, 255, 255],
            s.text_contrast_min,
            true,
            lead.tint(s.dark_base_lightness, s.dark_base_max_chroma),
        )
    };
    let base = readable(base, text, min, darker, s.lightness_step);
    let stops: Vec<[u8; 3]> = chosen
        .iter()
        .map(|c| {
            let c = if scheme == Scheme::Light {
                c.tint(
                    c.l.max(s.light_field_min_lightness),
                    s.light_field_max_chroma,
                )
            } else {
                *c
            };
            readable(c, text, min, darker, s.lightness_step)
        })
        .collect();
    let lowest = stops
        .iter()
        .chain(std::iter::once(&base))
        .map(|c| contrast(*c, text))
        .fold(f64::INFINITY, f64::min);
    Some(Palette {
        scheme,
        base: hex(base),
        stops: stops.iter().map(|c| hex(*c)).collect(),
        text: hex(text),
        lowest_contrast: two_decimals(lowest),
        sampled: clusters
            .iter()
            .map(|(lab, w)| Swatch {
                colour: hex(rgb_of(*lab)),
                share: two_decimals(w / total * 100.0) / 100.0,
            })
            .collect(),
    })
}

/// The palette of a PNG cover; None when it has no opaque pixels.
pub fn palette_of_png(bytes: &[u8], s: &Settings) -> Result<Option<Palette>, PaletteError> {
    let (w, h, rgba) = decode_png(bytes)?;
    if u64::from(w) * u64::from(h) > 4096 * 4096 {
        return Err(PaletteError::TooLarge);
    }
    Ok(palette_of_rgba(w, h, &rgba, s))
}

#[cfg(test)]
mod tests;
