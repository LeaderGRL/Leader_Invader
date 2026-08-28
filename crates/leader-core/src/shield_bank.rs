pub const SHIELD_COUNT: usize = 4;
pub const SHIELD_W: usize = 16;
pub const SHIELD_H: usize = 8;
pub const SHIELD_BYTES_PER_ROW: usize = SHIELD_W / 8;
pub const SHIELD_BYTES_PER: usize = SHIELD_BYTES_PER_ROW * SHIELD_H;
pub const SHIELD_TOTAL_BYTES: usize = SHIELD_COUNT * SHIELD_BYTES_PER;
pub const SHIELD_Y: i16 = 69;
pub const SHIELD_X: [i16; SHIELD_COUNT] = [7, 38, 70, 101];

const INITIAL_ROWS: [u16; SHIELD_H] = [
    0b0011_1111_1111_1100,
    0b0111_1111_1111_1110,
    0b1111_1111_1111_1111,
    0b1111_1111_1111_1111,
    0b1111_1111_1111_1111,
    0b1111_1111_1111_1111,
    0b1111_1100_0011_1111,
    0b1111_1000_0001_1111,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShieldDamage {
    pub shield: usize,
    pub local_x: u8,
    pub local_y: u8,
    pub byte_index: usize,
    pub mask: u8,
    pub before: u8,
    pub after: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShieldBank {
    bytes: [u8; SHIELD_TOTAL_BYTES],
}

impl Default for ShieldBank {
    fn default() -> Self {
        let mut bank = Self {
            bytes: [0; SHIELD_TOTAL_BYTES],
        };
        for shield in 0..SHIELD_COUNT {
            for (row, bits) in INITIAL_ROWS.into_iter().enumerate() {
                let base = shield * SHIELD_BYTES_PER + row * SHIELD_BYTES_PER_ROW;
                bank.bytes[base] = (bits >> 8) as u8;
                bank.bytes[base + 1] = bits as u8;
            }
        }
        bank
    }
}

impl ShieldBank {
    #[must_use]
    pub const fn bytes(&self) -> &[u8; SHIELD_TOTAL_BYTES] {
        &self.bytes
    }

    #[must_use]
    pub fn remaining_pixels(&self) -> u32 {
        self.bytes.iter().map(|byte| byte.count_ones()).sum()
    }

    #[must_use]
    pub fn pixel(&self, shield: usize, local_x: usize, local_y: usize) -> bool {
        let Some((byte_index, mask)) = bit_address(shield, local_x, local_y) else {
            return false;
        };
        self.bytes[byte_index] & mask != 0
    }

    #[must_use]
    pub fn world_pixel(&self, x: i16, y: i16) -> bool {
        locate_world(x, y).is_some_and(|(shield, local_x, local_y)| {
            self.pixel(shield, usize::from(local_x), usize::from(local_y))
        })
    }

    pub fn damage_world(&mut self, x: i16, y: i16) -> Option<ShieldDamage> {
        let (shield, local_x, local_y) = locate_world(x, y)?;
        self.damage(shield, usize::from(local_x), usize::from(local_y))
    }

    pub fn damage(
        &mut self,
        shield: usize,
        local_x: usize,
        local_y: usize,
    ) -> Option<ShieldDamage> {
        let (byte_index, mask) = bit_address(shield, local_x, local_y)?;
        let before = self.bytes[byte_index];
        if before & mask == 0 {
            return None;
        }
        let after = before & !mask;
        self.bytes[byte_index] = after;
        Some(ShieldDamage {
            shield,
            local_x: local_x as u8,
            local_y: local_y as u8,
            byte_index,
            mask,
            before,
            after,
        })
    }

    #[must_use]
    pub fn rows(&self, shield: usize) -> Option<[u16; SHIELD_H]> {
        if shield >= SHIELD_COUNT {
            return None;
        }
        let mut rows = [0u16; SHIELD_H];
        for (row, value) in rows.iter_mut().enumerate() {
            let base = shield * SHIELD_BYTES_PER + row * SHIELD_BYTES_PER_ROW;
            *value = (u16::from(self.bytes[base]) << 8) | u16::from(self.bytes[base + 1]);
        }
        Some(rows)
    }
}

#[must_use]
pub fn locate_world(x: i16, y: i16) -> Option<(usize, u8, u8)> {
    let local_y = y - SHIELD_Y;
    if !(0..SHIELD_H as i16).contains(&local_y) {
        return None;
    }
    for (shield, origin_x) in SHIELD_X.into_iter().enumerate() {
        let local_x = x - origin_x;
        if (0..SHIELD_W as i16).contains(&local_x) {
            return Some((shield, local_x as u8, local_y as u8));
        }
    }
    None
}

#[must_use]
pub const fn byte_offset(shield: usize, local_x: usize, local_y: usize) -> Option<usize> {
    if shield >= SHIELD_COUNT || local_x >= SHIELD_W || local_y >= SHIELD_H {
        return None;
    }
    Some(shield * SHIELD_BYTES_PER + local_y * SHIELD_BYTES_PER_ROW + local_x / 8)
}

#[must_use]
pub const fn bit_mask(local_x: usize) -> Option<u8> {
    if local_x >= SHIELD_W {
        return None;
    }
    Some(1 << (7 - local_x % 8))
}

#[must_use]
pub const fn bit_address(
    shield: usize,
    local_x: usize,
    local_y: usize,
) -> Option<(usize, u8)> {
    let Some(offset) = byte_offset(shield, local_x, local_y) else {
        return None;
    };
    let Some(mask) = bit_mask(local_x) else {
        return None;
    };
    Some((offset, mask))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bank_contains_four_identical_bitmaps() {
        let bank = ShieldBank::default();
        for shield in 0..SHIELD_COUNT {
            assert_eq!(bank.rows(shield), Some(INITIAL_ROWS));
        }
        let expected: u32 = INITIAL_ROWS.into_iter().map(u16::count_ones).sum::<u32>()
            * SHIELD_COUNT as u32;
        assert_eq!(bank.remaining_pixels(), expected);
    }

    #[test]
    fn every_local_pixel_maps_to_exact_byte_and_bit() {
        let bank = ShieldBank::default();
        for shield in 0..SHIELD_COUNT {
            for y in 0..SHIELD_H {
                for x in 0..SHIELD_W {
                    let (byte, mask) = bit_address(shield, x, y).expect("valid bit address");
                    assert_eq!(byte, shield * SHIELD_BYTES_PER + y * 2 + x / 8);
                    assert_eq!(mask, 1 << (7 - x % 8));
                    let expected = INITIAL_ROWS[y] & (1 << (15 - x)) != 0;
                    assert_eq!(bank.pixel(shield, x, y), expected);
                }
            }
        }
    }

    #[test]
    fn world_coordinates_resolve_all_four_shields() {
        for (shield, x) in SHIELD_X.into_iter().enumerate() {
            assert_eq!(locate_world(x + 8, SHIELD_Y + 3), Some((shield, 8, 3)));
        }
        assert_eq!(locate_world(0, SHIELD_Y), None);
        assert_eq!(locate_world(SHIELD_X[0], SHIELD_Y - 1), None);
    }

    #[test]
    fn damage_clears_exactly_one_existing_bit_and_is_idempotent() {
        let mut bank = ShieldBank::default();
        let before_pixels = bank.remaining_pixels();
        let x = 8usize;
        let y = 3usize;
        assert!(bank.pixel(0, x, y));
        let damage = bank.damage(0, x, y).expect("solid shield pixel");
        assert_eq!(damage.mask, 0x80);
        assert_eq!(damage.after, damage.before & !damage.mask);
        assert!(!bank.pixel(0, x, y));
        assert_eq!(bank.remaining_pixels(), before_pixels - 1);
        assert_eq!(bank.damage(0, x, y), None);
    }

    #[test]
    fn holes_in_initial_silhouette_do_not_generate_damage() {
        let mut bank = ShieldBank::default();
        assert!(!bank.pixel(0, 0, 0));
        assert_eq!(bank.damage(0, 0, 0), None);
        assert!(!bank.pixel(0, 7, 7));
        assert_eq!(bank.damage(0, 7, 7), None);
    }
}
