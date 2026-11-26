//! A small JPEG reader for cover colours, with no outside library (Phase 12, ADR
//! core-0021). Subsonic servers can't be asked for a PNG, and their covers are almost
//! always JPEG: Navidrome scales them with Go's `image/jpeg`, which writes baseline
//! files.
//!
//! A palette needs only a small picture, so this reads each 8 x 8 block's average
//! colour (its DC coefficient) and skips the inverse DCT: a 512 px cover comes out
//! 64 px, the palette's sample size. Baseline and extended (8-bit) Huffman files are
//! read, with any chroma subsampling and restart markers. Progressive, lossless,
//! arithmetic-coded, 12-bit and CMYK files give None, so the cover counts as having
//! no colours and the profile accent shows, as it did before.

/// The largest picture read, in pixels a side (covers are asked for at 512).
const MAX_SIDE: usize = 8192;

/// A cover as RGBA at one eighth of its size (rounded up): width, height, pixels.
pub(super) fn decode_dc(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut dc_quant = [0u16; 4];
    let mut dc_tables: [Option<Huffman>; 4] = Default::default();
    let mut ac_tables: [Option<Huffman>; 4] = Default::default();
    let mut frame: Option<Frame> = None;
    let mut restart_interval = 0usize;
    let mut adobe_transform: Option<u8> = None;
    let mut scanned = false;
    let mut pos = 2usize;
    loop {
        // The next marker: 0xFF, any fill bytes, then its code. A file that ends
        // without its end marker still gives what it had.
        while pos < bytes.len() && bytes[pos] != 0xFF {
            pos += 1;
        }
        while pos < bytes.len() && bytes[pos] == 0xFF {
            pos += 1;
        }
        let Some(&marker) = bytes.get(pos) else {
            break;
        };
        pos += 1;
        match marker {
            0xD9 => break,
            0x01 | 0xD0..=0xD8 => continue,
            _ => {}
        }
        let len = usize::from(u16::from_be_bytes([*bytes.get(pos)?, *bytes.get(pos + 1)?]));
        if len < 2 {
            return None;
        }
        let seg = bytes.get(pos + 2..pos + len)?;
        pos += len;
        match marker {
            0xDB => read_quant(seg, &mut dc_quant)?,
            0xC4 => read_huffman(seg, &mut dc_tables, &mut ac_tables)?,
            0xC0 | 0xC1 => frame = Some(Frame::read(seg)?),
            // Progressive, lossless, hierarchical or arithmetic-coded.
            0xC2 | 0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => return None,
            0xDD => {
                restart_interval = usize::from(u16::from_be_bytes([*seg.first()?, *seg.get(1)?]))
            }
            0xEE if seg.starts_with(b"Adobe") => adobe_transform = seg.get(11).copied(),
            0xDA => {
                let f = frame.as_mut()?;
                let scan = Scan::read(seg, f)?;
                let tables = Tables {
                    dc: &dc_tables,
                    ac: &ac_tables,
                    quant: &dc_quant,
                };
                pos = decode_scan(bytes, pos, f, &scan, &tables, restart_interval)?;
                scanned = true;
            }
            _ => {}
        }
    }
    let f = frame?;
    if !scanned {
        return None;
    }
    Some(f.to_rgba(adobe_transform))
}

struct Tables<'a> {
    dc: &'a [Option<Huffman>; 4],
    ac: &'a [Option<Huffman>; 4],
    quant: &'a [u16; 4],
}

/// DQT: only each table's first value (the DC step) is needed.
fn read_quant(mut seg: &[u8], dc_quant: &mut [u16; 4]) -> Option<()> {
    while let Some(&pq_tq) = seg.first() {
        let (precision, table) = (pq_tq >> 4, usize::from(pq_tq & 3));
        let size = if precision == 0 { 64 } else { 128 };
        let values = seg.get(1..1 + size)?;
        dc_quant[table] = if precision == 0 {
            u16::from(values[0])
        } else {
            u16::from_be_bytes([values[0], values[1]])
        };
        seg = &seg[1 + size..];
    }
    Some(())
}

fn read_huffman(
    mut seg: &[u8],
    dc: &mut [Option<Huffman>; 4],
    ac: &mut [Option<Huffman>; 4],
) -> Option<()> {
    while let Some(&tc_th) = seg.first() {
        let counts = seg.get(1..17)?;
        let total: usize = counts.iter().map(|&c| usize::from(c)).sum();
        let values = seg.get(17..17 + total)?;
        let table = Huffman::new(counts, values)?;
        let slot = usize::from(tc_th & 3);
        match tc_th >> 4 {
            0 => dc[slot] = Some(table),
            1 => ac[slot] = Some(table),
            _ => return None,
        }
        seg = &seg[17 + total..];
    }
    Some(())
}

/// A Huffman table in the standard's decoding form (ITU T.81, F.2.2.3).
struct Huffman {
    max_code: [i32; 17],
    min_code: [i32; 17],
    first_value: [usize; 17],
    values: Vec<u8>,
}

impl Huffman {
    fn new(counts: &[u8], values: &[u8]) -> Option<Huffman> {
        let mut h = Huffman {
            max_code: [-1; 17],
            min_code: [0; 17],
            first_value: [0; 17],
            values: values.to_vec(),
        };
        let (mut code, mut k) = (0i32, 0usize);
        for (i, &count) in counts.iter().enumerate().take(16) {
            let (len, n) = (i + 1, usize::from(count));
            h.first_value[len] = k;
            h.min_code[len] = code;
            code += n as i32;
            k += n;
            if n > 0 {
                h.max_code[len] = code - 1;
            }
            // More codes than this length can hold: not a valid table.
            if code > (1 << len) {
                return None;
            }
            code <<= 1;
        }
        Some(h)
    }

    fn decode(&self, bits: &mut Bits<'_>) -> Option<u8> {
        let mut code = bits.bit() as i32;
        let lengths = self
            .max_code
            .iter()
            .zip(&self.min_code)
            .zip(&self.first_value);
        for ((&max, &min), &first) in lengths.skip(1) {
            if code <= max {
                return self.values.get(first + (code - min) as usize).copied();
            }
            code = (code << 1) | bits.bit() as i32;
        }
        None
    }
}

struct Component {
    id: u8,
    h: usize,
    v: usize,
    quant: usize,
    /// Blocks a row and rows of blocks, padded to whole MCUs.
    blocks_wide: usize,
    blocks_high: usize,
    /// Each block's dequantised DC coefficient.
    dc: Vec<i32>,
}

struct Frame {
    width: usize,
    height: usize,
    h_max: usize,
    v_max: usize,
    mcus_wide: usize,
    mcus_high: usize,
    components: Vec<Component>,
}

impl Frame {
    fn read(seg: &[u8]) -> Option<Frame> {
        if *seg.first()? != 8 {
            return None; // 12-bit
        }
        let height = usize::from(u16::from_be_bytes([*seg.get(1)?, *seg.get(2)?]));
        let width = usize::from(u16::from_be_bytes([*seg.get(3)?, *seg.get(4)?]));
        let count = usize::from(*seg.get(5)?);
        if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
            return None;
        }
        if count != 1 && count != 3 {
            return None; // CMYK and the like
        }
        let mut components = Vec::with_capacity(count);
        for i in 0..count {
            let c = seg.get(6 + 3 * i..9 + 3 * i)?;
            let (h, v) = (usize::from(c[1] >> 4), usize::from(c[1] & 15));
            if !(1..=4).contains(&h) || !(1..=4).contains(&v) {
                return None;
            }
            components.push(Component {
                id: c[0],
                h,
                v,
                quant: usize::from(c[2] & 3),
                blocks_wide: 0,
                blocks_high: 0,
                dc: Vec::new(),
            });
        }
        let h_max = components.iter().map(|c| c.h).max()?;
        let v_max = components.iter().map(|c| c.v).max()?;
        let mcus_wide = width.div_ceil(8 * h_max);
        let mcus_high = height.div_ceil(8 * v_max);
        for c in &mut components {
            c.blocks_wide = mcus_wide * c.h;
            c.blocks_high = mcus_high * c.v;
            c.dc = vec![0; c.blocks_wide * c.blocks_high];
        }
        Some(Frame {
            width,
            height,
            h_max,
            v_max,
            mcus_wide,
            mcus_high,
            components,
        })
    }

    /// One pixel per 8 x 8 pixels of the picture, in RGBA.
    fn to_rgba(&self, adobe_transform: Option<u8>) -> (u32, u32, Vec<u8>) {
        let (w, h) = (self.width.div_ceil(8), self.height.div_ceil(8));
        let level = |c: &Component, x: usize, y: usize| -> f32 {
            let bx = (x * c.h / self.h_max).min(c.blocks_wide - 1);
            let by = (y * c.v / self.v_max).min(c.blocks_high - 1);
            // The DC coefficient is eight times the block's mean, less the 128 offset.
            (c.dc[by * c.blocks_wide + bx] as f32 / 8.0 + 128.0).clamp(0.0, 255.0)
        };
        // RGB when an Adobe marker says so, or the components are named R, G, B.
        let ids: Vec<u8> = self.components.iter().map(|c| c.id).collect();
        let rgb = adobe_transform == Some(0) || ids == [b'R', b'G', b'B'];
        let mut out = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let px = if self.components.len() == 1 {
                    let l = level(&self.components[0], x, y);
                    [l, l, l]
                } else {
                    let [a, b, c] = [0, 1, 2].map(|i| level(&self.components[i], x, y));
                    if rgb {
                        [a, b, c]
                    } else {
                        let (cb, cr) = (b - 128.0, c - 128.0);
                        [
                            a + 1.402 * cr,
                            a - 0.344_136 * cb - 0.714_136 * cr,
                            a + 1.772 * cb,
                        ]
                    }
                };
                out.extend(px.map(|v| v.round().clamp(0.0, 255.0) as u8));
                out.push(255);
            }
        }
        (w as u32, h as u32, out)
    }
}

/// The components a scan holds (indexes into the frame's) with their tables.
struct Scan {
    parts: Vec<(usize, usize, usize)>,
}

impl Scan {
    fn read(seg: &[u8], frame: &Frame) -> Option<Scan> {
        let count = usize::from(*seg.first()?);
        if count == 0 || count > frame.components.len() {
            return None;
        }
        let mut parts = Vec::with_capacity(count);
        for i in 0..count {
            let s = seg.get(1 + 2 * i..3 + 2 * i)?;
            let index = frame.components.iter().position(|c| c.id == s[0])?;
            parts.push((index, usize::from(s[1] >> 4), usize::from(s[1] & 15)));
        }
        Some(Scan { parts })
    }
}

/// Reads the entropy-coded data after an SOS header; returns where it ended.
fn decode_scan(
    bytes: &[u8],
    start: usize,
    frame: &mut Frame,
    scan: &Scan,
    tables: &Tables<'_>,
    restart_interval: usize,
) -> Option<usize> {
    let mut bits = Bits::new(bytes, start);
    let mut preds = vec![0i32; scan.parts.len()];
    fn table(set: &[Option<Huffman>; 4], i: usize) -> Option<&Huffman> {
        set.get(i).and_then(Option::as_ref)
    }
    let parts: Vec<(usize, &Huffman, &Huffman, i32)> = scan
        .parts
        .iter()
        .map(|&(c, dc, ac)| {
            let q = i32::from(tables.quant[frame.components[c].quant]);
            Some((c, table(tables.dc, dc)?, table(tables.ac, ac)?, q))
        })
        .collect::<Option<_>>()?;

    // One component alone: its blocks in order, only those inside the picture.
    // Several: whole MCUs, each component's blocks in order inside each.
    let (units_wide, units_high) = if parts.len() == 1 {
        let c = &frame.components[parts[0].0];
        (
            (frame.width * c.h).div_ceil(frame.h_max).div_ceil(8),
            (frame.height * c.v).div_ceil(frame.v_max).div_ceil(8),
        )
    } else {
        (frame.mcus_wide, frame.mcus_high)
    };
    let units = units_wide * units_high;
    for unit in 0..units {
        if restart_interval > 0 && unit > 0 && unit % restart_interval == 0 {
            bits.restart()?;
            preds.iter_mut().for_each(|p| *p = 0);
        }
        let (ux, uy) = (unit % units_wide, unit / units_wide);
        for (i, &(c, dc, ac, q)) in parts.iter().enumerate() {
            let comp = &frame.components[c];
            let (bh, bv) = if parts.len() == 1 {
                (1, 1)
            } else {
                (comp.h, comp.v)
            };
            for by in 0..bv {
                for bx in 0..bh {
                    preds[i] = read_block(&mut bits, dc, ac, preds[i])?;
                    let comp = &mut frame.components[c];
                    let (x, y) = (ux * bh + bx, uy * bv + by);
                    if x < comp.blocks_wide && y < comp.blocks_high {
                        comp.dc[y * comp.blocks_wide + x] = preds[i] * q;
                    }
                }
            }
        }
    }
    Some(bits.end())
}

/// One block: the DC difference added to the prediction, the AC values skipped.
fn read_block(bits: &mut Bits<'_>, dc: &Huffman, ac: &Huffman, pred: i32) -> Option<i32> {
    let size = dc.decode(bits)?;
    if size > 11 {
        return None;
    }
    let dc_value = pred.checked_add(extend(bits.take(size), size))?;
    let mut k = 1;
    while k < 64 {
        let rs = ac.decode(bits)?;
        let (run, size) = (rs >> 4, rs & 15);
        if size == 0 {
            if run == 15 {
                k += 16;
                continue;
            }
            break; // end of block
        }
        if size > 10 {
            return None;
        }
        k += usize::from(run);
        bits.take(size);
        k += 1;
    }
    Some(dc_value)
}

/// A `size`-bit value read as the standard's signed form (T.81, F.2.2.1).
fn extend(v: i32, size: u8) -> i32 {
    if size == 0 {
        0
    } else if v < (1 << (size - 1)) {
        v - (1 << size) + 1
    } else {
        v
    }
}

/// The entropy-coded bits, with stuffed zero bytes taken out. At a marker (or the
/// end) it reads zeros, so a short file ends in a wrong picture, never a hang.
struct Bits<'a> {
    bytes: &'a [u8],
    pos: usize,
    acc: u32,
    left: u32,
    at_marker: bool,
}

impl<'a> Bits<'a> {
    fn new(bytes: &'a [u8], pos: usize) -> Bits<'a> {
        Bits {
            bytes,
            pos,
            acc: 0,
            left: 0,
            at_marker: false,
        }
    }

    fn bit(&mut self) -> u32 {
        if self.left == 0 {
            self.acc = u32::from(self.next_byte());
            self.left = 8;
        }
        self.left -= 1;
        (self.acc >> self.left) & 1
    }

    fn take(&mut self, n: u8) -> i32 {
        let mut v = 0i32;
        for _ in 0..n {
            v = (v << 1) | self.bit() as i32;
        }
        v
    }

    fn next_byte(&mut self) -> u8 {
        if self.at_marker {
            return 0;
        }
        match self.bytes.get(self.pos) {
            None => {
                self.at_marker = true;
                0
            }
            Some(0xFF) => match self.bytes.get(self.pos + 1) {
                Some(0x00) => {
                    self.pos += 2;
                    0xFF
                }
                _ => {
                    self.at_marker = true;
                    0
                }
            },
            Some(&b) => {
                self.pos += 1;
                b
            }
        }
    }

    /// At a restart marker: drop the bits left in the byte and step over the marker.
    fn restart(&mut self) -> Option<()> {
        self.left = 0;
        // Find the next marker (a damaged file may have a few bytes before it).
        while !self.at_marker {
            self.next_byte();
            if self.pos >= self.bytes.len() {
                return None;
            }
        }
        let code = *self.bytes.get(self.pos + 1)?;
        if !(0xD0..=0xD7).contains(&code) {
            return None;
        }
        self.pos += 2;
        self.at_marker = false;
        Some(())
    }

    /// Where the scan's data ended: the marker after it.
    fn end(&self) -> usize {
        let mut pos = self.pos;
        // Skip past any data not read (a scan with blocks this reader didn't need).
        while pos + 1 < self.bytes.len() {
            if self.bytes[pos] == 0xFF
                && self.bytes[pos + 1] != 0x00
                && !(0xD0..=0xD7).contains(&self.bytes[pos + 1])
            {
                return pos;
            }
            pos += 1;
        }
        self.bytes.len()
    }
}
