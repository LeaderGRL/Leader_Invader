use crate::memory_map::FRAMEBUFFER_BYTES;
use crate::video_timing::{H_VISIBLE, V_VISIBLE};

pub const FRAMEBUFFER_WIDTH: usize = H_VISIBLE as usize;
pub const FRAMEBUFFER_HEIGHT: usize = V_VISIBLE as usize;
pub const FRAMEBUFFER_FORMAT: &str = "1bpp-msb-first-row-major";

/// Decodes one native framebuffer pixel according to the core-owned VRAM
/// representation. Renderers may use this for presentation without duplicating
/// packing, row-stride, or bit-order semantics.
#[must_use]
pub fn framebuffer_pixel(bytes: &[u8], x: usize, y: usize) -> Option<bool> {
    if bytes.len() != FRAMEBUFFER_BYTES || x >= FRAMEBUFFER_WIDTH || y >= FRAMEBUFFER_HEIGHT {
        return None;
    }
    let index = y * FRAMEBUFFER_WIDTH + x;
    let byte = bytes[index >> 3];
    Some(byte & (1 << (7 - (index & 7))) != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn representation_dimensions_match_canonical_vram_capacity() {
        assert_eq!(FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT / 8, FRAMEBUFFER_BYTES);
    }

    #[test]
    fn decoder_owns_msb_first_row_major_packing() {
        let mut bytes = vec![0_u8; FRAMEBUFFER_BYTES];
        bytes[0] = 0b1000_0001;
        bytes[FRAMEBUFFER_WIDTH / 8] = 0b0100_0000;

        assert_eq!(framebuffer_pixel(&bytes, 0, 0), Some(true));
        assert_eq!(framebuffer_pixel(&bytes, 1, 0), Some(false));
        assert_eq!(framebuffer_pixel(&bytes, 7, 0), Some(true));
        assert_eq!(framebuffer_pixel(&bytes, 1, 1), Some(true));
    }

    #[test]
    fn decoder_rejects_noncanonical_buffers_and_coordinates() {
        assert_eq!(framebuffer_pixel(&[], 0, 0), None);
        let bytes = vec![0_u8; FRAMEBUFFER_BYTES];
        assert_eq!(framebuffer_pixel(&bytes, FRAMEBUFFER_WIDTH, 0), None);
        assert_eq!(framebuffer_pixel(&bytes, 0, FRAMEBUFFER_HEIGHT), None);
    }
}