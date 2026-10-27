//! A small, dependency-free Bloom filter used as a fast negative-membership
//! gate in front of the global/user lexicons.

#[derive(Debug, Clone)]
pub struct BloomFilter {
    bits: Vec<u64>,
    num_bits: u64,
    num_hashes: u32,
}

impl BloomFilter {
    pub fn with_capacity(expected_items: usize, false_positive_rate: f64) -> Self {
        let n = expected_items.max(1) as f64;
        let p = false_positive_rate.clamp(1e-9, 0.5);
        let m = (-(n * p.ln()) / (std::f64::consts::LN_2 * std::f64::consts::LN_2)).ceil() as u64;
        let m = m.max(64);
        let k = ((m as f64 / n) * std::f64::consts::LN_2)
            .round()
            .clamp(1.0, 16.0) as u32;
        Self {
            bits: vec![0u64; m.div_ceil(64) as usize],
            num_bits: m,
            num_hashes: k,
        }
    }

    pub fn insert(&mut self, value: &str) {
        let indices: Vec<u64> = self.indices(value).collect();
        for bit in indices {
            self.bits[(bit / 64) as usize] |= 1u64 << (bit % 64);
        }
    }

    pub fn contains(&self, value: &str) -> bool {
        self.indices(value)
            .all(|bit| self.bits[(bit / 64) as usize] & (1u64 << (bit % 64)) != 0)
    }

    fn indices<'a>(&'a self, value: &'a str) -> impl Iterator<Item = u64> + 'a {
        let h1 = fnv1a_64(value.as_bytes());
        let h2 = splitmix64(h1 ^ 0x9e37_79b9_7f4a_7c15) | 1;
        (0..self.num_hashes)
            .map(move |i| h1.wrapping_add((i as u64).wrapping_mul(h2)) % self.num_bits)
    }
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_false_negatives() {
        let mut bloom = BloomFilter::with_capacity(10_000, 0.01);
        let words: Vec<String> = (0..5_000).map(|i| format!("word{i}")).collect();
        for word in &words {
            bloom.insert(word);
        }
        for word in &words {
            assert!(bloom.contains(word), "false negative for {word}");
        }
        let misses = (0..5_000)
            .filter(|i| !bloom.contains(&format!("missing{i}")))
            .count();
        assert!(
            misses > 4_000,
            "unexpectedly high false positive rate: {misses}"
        );
    }
}
