//! A fixed random projection of the embedding to 128 numbers, for the engine (docs/adr/core-0019).
//!
//! Similarity between songs is looked up in the engine's memory for every candidate of every pick,
//! so a song's sound is kept there as 128 integers (256 bytes) instead of 1024 (1 KB, 100 MB for
//! 100,000 songs). A random projection keeps the distances between vectors, and on the 200 test
//! clips 128 numbers found the same genre as often as all 1024 (43.6% against 45.4%, both after
//! centring). The signs come from SplitMix64 on the position, in integers, so every platform,
//! and the server, projects the same bytes to the same numbers.

use std::sync::OnceLock;

use crate::EMBEDDING_LEN;

/// The length of the projection.
pub const PROJECTION_DIMS: usize = 128;

const SEED: u64 = 0x656E_636F_7265_0001;

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The +1 or -1 for input `i` and output `j`, as a table [input][output].
fn signs() -> &'static [i8] {
    static SIGNS: OnceLock<Vec<i8>> = OnceLock::new();
    SIGNS.get_or_init(|| {
        let mut table = Vec::with_capacity(EMBEDDING_LEN * PROJECTION_DIMS);
        for i in 0..EMBEDDING_LEN {
            for j in 0..PROJECTION_DIMS {
                let h = splitmix(SEED ^ ((i as u64) << 16) ^ j as u64);
                table.push(if h & 1 == 1 { 1 } else { -1 });
            }
        }
        table
    })
}

/// Project the stored embedding (`Embedding::encode`: a version byte, the 1024 numbers, the 521
/// scores) to [`PROJECTION_DIMS`] numbers. None when the bytes are not an embedding.
pub fn project_encoded(encoded: &[u8]) -> Option<Vec<i16>> {
    if encoded.len() != crate::EMBEDDING_ENCODED_LEN || encoded[0] != 1 {
        return None;
    }
    let numbers = &encoded[1..=EMBEDDING_LEN];
    let table = signs();
    let mut sums = [0i32; PROJECTION_DIMS];
    for (i, byte) in numbers.iter().enumerate() {
        let row = &table[i * PROJECTION_DIMS..(i + 1) * PROJECTION_DIMS];
        let v = i32::from(*byte);
        for (s, sign) in sums.iter_mut().zip(row) {
            *s += v * i32::from(*sign);
        }
    }
    // Scaled down by 256 so the values fit 16 bits whatever the input (at most 1024 x 255 / 256).
    Some(sums.iter().map(|s| ((s + 128) >> 8) as i16).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EMBEDDING_ENCODED_LEN, Embedding};

    fn embedding(seed: u32) -> Embedding {
        Embedding {
            pooled: (0..EMBEDDING_LEN as u32)
                .map(|i| ((i * seed) % 61) as f32 / 10.0)
                .collect(),
            scores: vec![0.0; crate::CLASSES],
        }
    }

    #[test]
    fn it_projects_to_the_same_numbers_every_time() {
        let bytes = embedding(7).encode();
        let a = project_encoded(&bytes).unwrap();
        assert_eq!(a.len(), PROJECTION_DIMS);
        assert_eq!(a, project_encoded(&bytes).unwrap());
        // Pinned: a change here would make every stored projection and every server catalog disagree.
        assert_eq!(&a[..4], [PINNED[0], PINNED[1], PINNED[2], PINNED[3]]);
    }

    /// The first four numbers of embedding(7)'s projection, from the first run of this function.
    const PINNED: [i16; 4] = [-10, 22, -5, -10];

    #[test]
    fn it_keeps_what_is_alike_alike() {
        let cos = |a: &[i16], b: &[i16]| {
            let dot: f64 = a
                .iter()
                .zip(b)
                .map(|(x, y)| f64::from(*x) * f64::from(*y))
                .sum();
            let na: f64 = a.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
            let nb: f64 = b.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
            dot / (na * nb)
        };
        let a = embedding(7);
        let mut near = embedding(7);
        near.pooled
            .iter_mut()
            .step_by(5)
            .for_each(|v| *v = (*v + 0.2).min(6.0));
        let far = embedding(13);
        let (pa, pn, pf) = (
            project_encoded(&a.encode()).unwrap(),
            project_encoded(&near.encode()).unwrap(),
            project_encoded(&far.encode()).unwrap(),
        );
        assert!(
            cos(&pa, &pn) > cos(&pa, &pf),
            "{} vs {}",
            cos(&pa, &pn),
            cos(&pa, &pf)
        );
    }

    #[test]
    fn it_refuses_what_is_not_an_embedding() {
        assert!(project_encoded(&[1, 2, 3]).is_none());
        let mut bytes = embedding(3).encode();
        bytes[0] = 9;
        assert!(project_encoded(&bytes).is_none());
        assert_eq!(embedding(3).encode().len(), EMBEDDING_ENCODED_LEN);
    }
}
