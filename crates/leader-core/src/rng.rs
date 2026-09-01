#[derive(Debug, Clone)]
pub struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    #[must_use]
    pub fn from_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    #[must_use]
    pub fn from_text(seed: &str) -> Self {
        Self::from_seed(hash_seed(seed))
    }

    #[must_use]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    #[must_use]
    pub fn range_u32(&mut self, upper_exclusive: u32) -> u32 {
        if upper_exclusive <= 1 {
            return 0;
        }
        (self.next_u64() % u64::from(upper_exclusive)) as u32
    }

    #[must_use]
    pub fn range_i16(&mut self, min_inclusive: i16, max_inclusive: i16) -> i16 {
        if min_inclusive >= max_inclusive {
            return min_inclusive;
        }
        let span = i32::from(max_inclusive) - i32::from(min_inclusive) + 1;
        min_inclusive + self.range_u32(span as u32) as i16
    }

    #[must_use]
    pub fn chance(&mut self, numerator: u32, denominator: u32) -> bool {
        if denominator == 0 || numerator == 0 {
            return false;
        }
        if numerator >= denominator {
            return true;
        }
        self.range_u32(denominator) < numerator
    }
}

#[must_use]
pub fn hash_seed(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_seed_is_stable() {
        assert_eq!(hash_seed("leader"), 0x9d24_b27a_c6dd_eae4);
    }

    #[test]
    fn splitmix_sequence_is_repeatable() {
        let mut a = DeterministicRng::from_seed(42);
        let mut b = DeterministicRng::from_seed(42);
        for _ in 0..32 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }
}
