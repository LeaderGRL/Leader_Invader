use std::fmt::Write;

use leader_core::{
    framebuffer_pixel, VramCheckpoint, FRAMEBUFFER_HEIGHT, FRAMEBUFFER_WIDTH,
};

use crate::RenderConfig;

const MAX_RASTER_SAMPLES: usize = 96;

pub(crate) fn render_vram_replay(
    out: &mut String,
    checkpoints: &[VramCheckpoint],
    total_frames: u32,
    config: RenderConfig,
) {
    if checkpoints.is_empty() {
        return;
    }
    let samples = sample_checkpoints(checkpoints, MAX_RASTER_SAMPLES);
    let total = config.total();

    for (index, checkpoint) in samples.iter().enumerate() {
        let start = trace_time(checkpoint.frame, total_frames, config);
        let end = samples
            .get(index + 1)
            .map_or(config.game_end(), |next| {
                trace_time(next.frame, total_frames, config)
            })
            .max(start);
        let path = framebuffer_runs(&checkpoint.bytes);
        if path.is_empty() {
            continue;
        }
        let _ = write!(
            out,
            r##"<path d="{path}" fill="#b7ff72" opacity="0"><animate attributeName="opacity" values="0;1;1;0" keyTimes="0;{:.6};{:.6};1" dur="{total:.3}s" repeatCount="indefinite" calcMode="discrete"/></path>"##,
            norm(start, total),
            norm(end, total)
        );
    }
}

fn framebuffer_runs(bytes: &[u8]) -> String {
    let mut path = String::new();
    for y in 0..FRAMEBUFFER_HEIGHT {
        let mut x = 0;
        while x < FRAMEBUFFER_WIDTH {
            if framebuffer_pixel(bytes, x, y) != Some(true) {
                x += 1;
                continue;
            }
            let start = x;
            x += 1;
            while x < FRAMEBUFFER_WIDTH && framebuffer_pixel(bytes, x, y) == Some(true) {
                x += 1;
            }
            let width = x - start;
            let _ = write!(path, "M{start} {y}h{width}v1h-{width}z");
        }
    }
    path
}

fn sample_checkpoints(
    checkpoints: &[VramCheckpoint],
    max_samples: usize,
) -> Vec<&VramCheckpoint> {
    if checkpoints.len() <= max_samples {
        return checkpoints.iter().collect();
    }
    let stride = checkpoints.len().div_ceil(max_samples);
    let mut sampled = checkpoints.iter().step_by(stride).collect::<Vec<_>>();
    if sampled.last().map(|checkpoint| checkpoint.frame)
        != checkpoints.last().map(|checkpoint| checkpoint.frame)
    {
        if let Some(last) = checkpoints.last() {
            sampled.push(last);
        }
    }
    sampled
}

fn trace_time(frame: u32, total_frames: u32, config: RenderConfig) -> f32 {
    config.game_start()
        + frame as f32 / total_frames.max(1) as f32 * config.game_seconds
}

fn norm(value: f32, total: f32) -> f32 {
    (value / total).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::FRAMEBUFFER_BYTES;

    fn checkpoint(frame: u32, pixels: &[(usize, usize)]) -> VramCheckpoint {
        let mut bytes = vec![0_u8; FRAMEBUFFER_BYTES];
        for &(x, y) in pixels {
            let index = y * FRAMEBUFFER_WIDTH + x;
            bytes[index >> 3] |= 1 << (7 - (index & 7));
        }
        VramCheckpoint {
            frame,
            checksum: frame,
            bytes: bytes.into_boxed_slice(),
        }
    }

    #[test]
    fn raster_path_compacts_adjacent_lit_pixels_into_horizontal_runs() {
        let checkpoint = checkpoint(0, &[(0, 0), (1, 0), (2, 0), (7, 1)]);
        let path = framebuffer_runs(&checkpoint.bytes);
        assert!(path.contains("M0 0h3v1h-3z"));
        assert!(path.contains("M7 1h1v1h-1z"));
    }

    #[test]
    fn sampling_preserves_final_native_checkpoint() {
        let checkpoints = (0..200)
            .map(|frame| checkpoint(frame, &[(frame as usize % FRAMEBUFFER_WIDTH, 0)]))
            .collect::<Vec<_>>();
        let sampled = sample_checkpoints(&checkpoints, 16);
        assert!(sampled.len() <= 17);
        assert_eq!(sampled.last().map(|checkpoint| checkpoint.frame), Some(199));
    }

    #[test]
    fn replay_serializes_native_rasters_without_game_objects() {
        let checkpoints = vec![checkpoint(0, &[(2, 3)]), checkpoint(1, &[(4, 5)])];
        let mut svg = String::new();
        render_vram_replay(&mut svg, &checkpoints, 2, RenderConfig::default());
        assert!(svg.contains("M2 3h1v1h-1z"));
        assert!(svg.contains("M4 5h1v1h-1z"));
        assert!(!svg.contains("alien"));
        assert!(!svg.contains("projectile"));
    }
}