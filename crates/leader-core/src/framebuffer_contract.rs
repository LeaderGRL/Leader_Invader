use crate::framebuffer::framebuffer_pixel;
use crate::game::{ALIEN_COLS, ALIEN_ROWS, PLAYER_Y, SCREEN_H, SCREEN_W};
use crate::trace::{FrameState, MatchTrace, VramCheckpoint};

const INVADER_A: [&str; 6] = [
    "00111100",
    "01111110",
    "11011011",
    "11111111",
    "00100100",
    "01000010",
];
const INVADER_B: [&str; 6] = [
    "01100110",
    "11111111",
    "10111101",
    "11111111",
    "00100100",
    "01011010",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramebufferValidation {
    pub checked_frames: usize,
    pub checked_alien_pixels: usize,
    pub checked_player_pixels: usize,
    pub checked_floor_pixels: usize,
}

/// Validate the native 128×96 framebuffer against the game state retained at
/// the same raster boundary. This deliberately checks geometry, not only a
/// checksum: every mandatory sprite pixel for every living invader, the player
/// silhouette and the complete floor scanline must exist in VRAM.
#[must_use]
pub fn validate_native_framebuffer_contract(trace: &MatchTrace) -> Result<FramebufferValidation, String> {
    if trace.vram_checkpoints.is_empty() {
        return Err("native trace contains no VRAM checkpoints".to_string());
    }

    let mut validation = FramebufferValidation {
        checked_frames: 0,
        checked_alien_pixels: 0,
        checked_player_pixels: 0,
        checked_floor_pixels: 0,
    };

    for checkpoint in representative_checkpoints(&trace.vram_checkpoints, 24) {
        let state = matching_state(&trace.frames, checkpoint.frame)
            .ok_or_else(|| format!("missing FrameState for VRAM frame {}", checkpoint.frame))?;
        validate_frame(checkpoint, state, &mut validation)?;
        validation.checked_frames += 1;
    }

    Ok(validation)
}

fn validate_frame(
    checkpoint: &VramCheckpoint,
    state: &FrameState,
    validation: &mut FramebufferValidation,
) -> Result<(), String> {
    for row in 0..ALIEN_ROWS {
        for col in 0..ALIEN_COLS {
            if state.alive_rows[row] & (1 << col) == 0 {
                continue;
            }
            let origin_x = state.fleet_x + col as i16 * 12;
            let origin_y = state.fleet_y + row as i16 * 13;
            let bitmap = if (row + col) % 2 == 0 { &INVADER_B } else { &INVADER_A };
            for (dy, bits) in bitmap.iter().enumerate() {
                for (dx, value) in bits.as_bytes().iter().enumerate() {
                    if *value != b'1' {
                        continue;
                    }
                    require_pixel(checkpoint, origin_x + dx as i16, origin_y + dy as i16, "invader")?;
                    validation.checked_alien_pixels += 1;
                }
            }
        }
    }

    for dx in -5..=5 {
        require_pixel(checkpoint, state.player_x + dx, PLAYER_Y, "player base")?;
        validation.checked_player_pixels += 1;
    }
    for dx in -3..=3 {
        require_pixel(checkpoint, state.player_x + dx, PLAYER_Y - 1, "player turret")?;
        validation.checked_player_pixels += 1;
    }
    for dy in 2..=4 {
        require_pixel(checkpoint, state.player_x, PLAYER_Y - dy, "player barrel")?;
        validation.checked_player_pixels += 1;
    }

    for x in 0..SCREEN_W {
        require_pixel(checkpoint, x, SCREEN_H - 3, "floor")?;
        validation.checked_floor_pixels += 1;
    }

    Ok(())
}

fn require_pixel(checkpoint: &VramCheckpoint, x: i16, y: i16, label: &str) -> Result<(), String> {
    if x < 0 || y < 0 || x >= SCREEN_W || y >= SCREEN_H {
        return Err(format!(
            "{label} pixel ({x},{y}) is outside the native 128x96 raster at frame {}",
            checkpoint.frame
        ));
    }
    if framebuffer_pixel(&checkpoint.bytes, x as usize, y as usize) != Some(true) {
        return Err(format!(
            "missing {label} pixel ({x},{y}) in native VRAM frame {} checksum {:08X}",
            checkpoint.frame, checkpoint.checksum
        ));
    }
    Ok(())
}

fn matching_state(frames: &[FrameState], frame: u32) -> Option<&FrameState> {
    frames.iter().find(|state| state.frame == frame)
}

fn representative_checkpoints(values: &[VramCheckpoint], limit: usize) -> Vec<&VramCheckpoint> {
    if values.len() <= limit || limit == 0 {
        return values.iter().collect();
    }
    let stride = values.len().div_ceil(limit);
    values.iter().step_by(stride).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{ALIEN_H, ALIEN_W};
    use crate::Machine;

    #[test]
    fn native_space_invaders_raster_preserves_sprite_geometry() {
        let trace = Machine::run_match("framebuffer-visual-contract", 5000);
        let validation = validate_native_framebuffer_contract(&trace).expect("native framebuffer geometry");
        assert!(validation.checked_frames >= 8);
        assert!(validation.checked_alien_pixels > 100);
        assert!(validation.checked_player_pixels > 100);
        assert!(validation.checked_floor_pixels > 1000);
    }

    #[test]
    fn declared_sprite_dimensions_match_the_native_bitmap_contract() {
        assert_eq!(ALIEN_W, 8);
        assert_eq!(ALIEN_H, 6);
        assert_eq!(INVADER_A.len(), ALIEN_H as usize);
        assert_eq!(INVADER_B.len(), ALIEN_H as usize);
        assert!(INVADER_A.iter().all(|row| row.len() == ALIEN_W as usize));
        assert!(INVADER_B.iter().all(|row| row.len() == ALIEN_W as usize));
    }
}
