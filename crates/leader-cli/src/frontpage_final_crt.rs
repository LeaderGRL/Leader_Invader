use std::fmt::Write as _;

use leader_core::{
    framebuffer_pixel, FrameState, MatchTrace, VramCheckpoint, FRAMEBUFFER_HEIGHT,
    FRAMEBUFFER_WIDTH,
};
use leader_svg::RenderConfig;

const FINAL_OUTER_X: f32 = 180.0;
const FINAL_OUTER_Y: f32 = 35.0;
const FINAL_OUTER_W: f32 = 840.0;
const FINAL_OUTER_H: f32 = 600.0;
const FINAL_RASTER_X: f32 = 220.0;
const FINAL_RASTER_Y: f32 = 75.0;
const FINAL_RASTER_SCALE: f32 = 5.9375;
const FOCUS_ZOOM_FROM: f32 = 0.42;
const ROOT_CENTER_X: f32 = 600.0;
const ROOT_CENTER_Y: f32 = 337.5;
const MAX_SHOWCASE_FRAMES: usize = 28;
const SHOWCASE_SECONDS: f32 = 4.8;
const SHOWCASE_PRESENTATION_FRACTION: f32 = 0.56;
const TARGET_ALIENS: u32 = 18;
const TARGET_SCORE: u16 = 140;

#[derive(Debug, Clone, Copy)]
struct Showcase<'a> {
    state: &'a FrameState,
    start_time: f32,
    end_time: f32,
}

/// Adds a large native CRT replay sourced from an active portion of the match.
/// The source frames are exact 1536-byte VRAM checkpoints selected while a
/// substantial alien formation and live projectile activity still exist. The
/// replay itself is scheduled into a reserved presentation window, so it never
/// masks a named technical camera scene and never waits for game completion.
#[must_use]
pub fn apply(mut svg: String, trace: &MatchTrace, config: RenderConfig) -> String {
    if !svg.contains("data-frontpage-version=\"physical-die-v2\"") || trace.total_frames == 0 {
        return svg;
    }
    let Some(showcase) = select_showcase(trace, config) else {
        return svg;
    };
    let frames = showcase_checkpoints(trace, showcase.state.frame);
    if frames.is_empty() {
        return svg;
    }

    let focus_start = showcase.start_time;
    let focus_end = showcase.end_time;
    let zoom_in_end = (focus_start + 0.65).min(focus_end - 0.5);
    let zoom_out_start = (focus_end - 0.65).max(zoom_in_end + 0.1);
    let k1 = normalized(focus_start, config.total());
    let k2 = normalized(zoom_in_end, config.total()).max(k1 + 0.000_01);
    let k3 = normalized(zoom_out_start, config.total()).max(k2 + 0.000_01);
    let k4 = normalized(focus_end, config.total()).max(k3 + 0.000_01);
    let zoom_tx = ROOT_CENTER_X * (1.0 - FOCUS_ZOOM_FROM);
    let zoom_ty = ROOT_CENTER_Y * (1.0 - FOCUS_ZOOM_FROM);
    let enemy_shots = active_enemy_shots(showcase.state);
    let player_shot = showcase.state.player_shot.is_some();

    let mut overlay = String::with_capacity(frames.len() * 16_000 + 5_000);
    let _ = writeln!(
        overlay,
        r##"<g id="v2-final-crt-focus" opacity="0" data-final-focus="native-vram" data-showcase-live="true" data-vram-frame="{}" data-vram-checksum="{:08X}" data-showcase-alive="{}" data-showcase-score="{}" data-showcase-enemy-shots="{enemy_shots}" data-showcase-player-shot="{player_shot}" data-focus-start="{focus_start:.3}" data-focus-end="{focus_end:.3}">
<animate attributeName="opacity" values="0;0;1;1;0;0" keyTimes="0;{k1:.7};{k2:.7};{k3:.7};{k4:.7};1" dur="{:.3}s" repeatCount="indefinite"/>
<g id="v2-final-crt-translate" transform="translate({zoom_tx:.5} {zoom_ty:.5})"><animateTransform attributeName="transform" attributeType="XML" type="translate" values="{zoom_tx:.5} {zoom_ty:.5};{zoom_tx:.5} {zoom_ty:.5};0 0;0 0;{zoom_tx:.5} {zoom_ty:.5};{zoom_tx:.5} {zoom_ty:.5}" keyTimes="0;{k1:.7};{k2:.7};{k3:.7};{k4:.7};1" dur="{:.3}s" repeatCount="indefinite"/>
<g id="v2-final-crt-scale" transform="scale({FOCUS_ZOOM_FROM:.5})"><animateTransform attributeName="transform" attributeType="XML" type="scale" values="{FOCUS_ZOOM_FROM:.5};{FOCUS_ZOOM_FROM:.5};1;1;{FOCUS_ZOOM_FROM:.5};{FOCUS_ZOOM_FROM:.5}" keyTimes="0;{k1:.7};{k2:.7};{k3:.7};{k4:.7};1" dur="{:.3}s" repeatCount="indefinite"/>
<rect x="0" y="0" width="1200" height="675" fill="#020406" opacity=".985"/>
<rect x="{FINAL_OUTER_X}" y="{FINAL_OUTER_Y}" width="{FINAL_OUTER_W}" height="{FINAL_OUTER_H}" rx="34" fill="#071019" stroke="#657d89" stroke-width="5"/>
<rect x="194" y="49" width="812" height="572" rx="27" fill="#030807" stroke="#243c35" stroke-width="2"/>
<rect x="{FINAL_RASTER_X}" y="{FINAL_RASTER_Y}" width="760" height="570" rx="13" fill="#010302" stroke="#41614f" stroke-width="2" data-final-native-raster="128x96"/>
"##,
        showcase.state.frame,
        showcase.state.vram_checksum,
        alive_count(showcase.state),
        showcase.state.score,
        config.total(),
        config.total(),
        config.total(),
    );

    let span = (focus_end - focus_start).max(0.1);
    for (index, checkpoint) in frames.iter().enumerate() {
        let frame_start = focus_start + index as f32 / frames.len() as f32 * span;
        let frame_end = focus_start + (index + 1) as f32 / frames.len() as f32 * span;
        let fk1 = normalized(frame_start, config.total());
        let fk2 = normalized(frame_end, config.total()).max(fk1 + 0.000_01);
        let path = framebuffer_path(&checkpoint.bytes);
        let _ = writeln!(
            overlay,
            r##"<path d="{path}" transform="translate({FINAL_RASTER_X:.3} {FINAL_RASTER_Y:.3}) scale({FINAL_RASTER_SCALE:.7})" fill="#b9ff78" shape-rendering="crispEdges" opacity="0" data-final-native-pixels="true" data-showcase-vram-frame="{}" data-showcase-vram-checksum="{:08X}"><animate attributeName="opacity" values="0;1;0;0" keyTimes="0;{fk1:.7};{fk2:.7};1" calcMode="discrete" dur="{:.3}s" repeatCount="indefinite"/></path>"##,
            checkpoint.frame,
            checkpoint.checksum,
            config.total(),
        );
    }

    let _ = writeln!(
        overlay,
        r##"<path d="M232 86 C390 63 808 63 968 86" fill="none" stroke="#e8ffe0" stroke-width="2" opacity=".055"/>
<text x="{FINAL_RASTER_X}" y="65" fill="#8fb09b" font-size="10" font-weight="900">ACTIVE NATIVE VRAM REPLAY · {} ALIENS · SCORE {} · {} ENEMY SHOTS</text>
<text x="980" y="65" text-anchor="end" fill="#657f70" font-size="9" font-weight="900">VRAM → DMA → SCANOUT · 128×96 · 1 BPP</text>
</g></g></g>"##,
        alive_count(showcase.state),
        showcase.state.score,
        enemy_shots,
    );

    if let Some(index) = svg.rfind("</svg>") {
        svg.insert_str(index, &overlay);
    }
    svg
}

fn select_showcase(trace: &MatchTrace, config: RenderConfig) -> Option<Showcase<'_>> {
    let target_frame = trace.total_frames.saturating_mul(22) / 100;
    let state = trace
        .frames
        .iter()
        .filter(|state| {
            let alive = alive_count(state);
            let enemy_activity = active_enemy_shots(state) > 0;
            (14..=24).contains(&alive)
                && (80..=200).contains(&state.score)
                && state.lives >= 1
                && (state.player_shot.is_some() || enemy_activity)
        })
        .min_by_key(|state| showcase_cost(state, target_frame))
        .or_else(|| {
            trace
                .frames
                .iter()
                .filter(|state| {
                    alive_count(state) >= 10
                        && state.lives >= 1
                        && (state.player_shot.is_some() || active_enemy_shots(state) > 0)
                })
                .min_by_key(|state| showcase_cost(state, target_frame))
        })?;

    // Presentation timing is intentionally independent from the source frame.
    // The CRT is an explicit native replay window, placed between technical
    // scenes so it cannot hide ROM/ALU/RAM inspection and never lands in outro.
    let start_time = config.game_start() + config.game_seconds * SHOWCASE_PRESENTATION_FRACTION;
    let end_time = (start_time + SHOWCASE_SECONDS).min(config.game_end() - 1.2);
    Some(Showcase {
        state,
        start_time,
        end_time,
    })
}

fn showcase_cost(state: &FrameState, target_frame: u32) -> u64 {
    let alive_penalty = u64::from(alive_count(state).abs_diff(TARGET_ALIENS)) * 10_000;
    let score_penalty = u64::from(state.score.abs_diff(TARGET_SCORE)) * 100;
    let frame_penalty = u64::from(state.frame.abs_diff(target_frame));
    let activity_bonus = if state.player_shot.is_some() { 1_500 } else { 0 }
        + u64::from(active_enemy_shots(state)) * 2_000;
    alive_penalty
        .saturating_add(score_penalty)
        .saturating_add(frame_penalty)
        .saturating_sub(activity_bonus)
}

fn showcase_checkpoints(trace: &MatchTrace, center_frame: u32) -> Vec<&VramCheckpoint> {
    if trace.vram_checkpoints.is_empty() {
        return Vec::new();
    }
    let radius = (trace.total_frames / 18).max(24);
    let start = center_frame.saturating_sub(radius);
    let end = center_frame.saturating_add(radius).min(trace.total_frames);
    let candidates = trace
        .vram_checkpoints
        .iter()
        .filter(|checkpoint| checkpoint.frame >= start && checkpoint.frame <= end)
        .collect::<Vec<_>>();
    sample_refs(&candidates, MAX_SHOWCASE_FRAMES)
}

fn alive_count(state: &FrameState) -> u32 {
    state.alive_rows.iter().map(|row| row.count_ones()).sum()
}

fn active_enemy_shots(state: &FrameState) -> u32 {
    state.enemy_shots.iter().filter(|shot| shot.is_some()).count() as u32
}

fn framebuffer_path(bytes: &[u8]) -> String {
    let mut path = String::with_capacity(30_000);
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

fn sample_refs<'a, T>(values: &[&'a T], maximum: usize) -> Vec<&'a T> {
    if values.len() <= maximum || maximum == 0 {
        return values.to_vec();
    }
    let stride = values.len().div_ceil(maximum);
    let mut sampled = values.iter().step_by(stride).copied().collect::<Vec<_>>();
    if let (Some(sampled_last), Some(last)) = (sampled.last(), values.last()) {
        if !std::ptr::eq(*sampled_last, *last) {
            sampled.push(*last);
        }
    }
    sampled
}

fn normalized(time: f32, total: f32) -> f32 {
    (time / total.max(0.001)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::Machine;

    #[test]
    fn showcase_is_selected_before_the_match_is_cleared() {
        let trace = Machine::run_match("active-crt-showcase", 5000);
        let showcase = select_showcase(&trace, crate::frontpage::render_config()).expect("active showcase");
        assert!(alive_count(showcase.state) >= 10);
        assert!(showcase.state.score < trace.final_score);
        assert!(showcase.state.player_shot.is_some() || active_enemy_shots(showcase.state) > 0);
        assert!(showcase.end_time < crate::frontpage::render_config().game_end());
    }

    #[test]
    fn preferred_showcase_is_a_dense_mid_match_state() {
        let trace = Machine::run_match("active-crt-density", 5000);
        let showcase = select_showcase(&trace, crate::frontpage::render_config()).expect("active showcase");
        assert!(alive_count(showcase.state) >= 14);
        assert!(showcase.state.score <= 200);
    }

    #[test]
    fn showcase_uses_multiple_native_vram_checkpoints() {
        let trace = Machine::run_match("active-crt-frames", 5000);
        let showcase = select_showcase(&trace, crate::frontpage::render_config()).expect("active showcase");
        let frames = showcase_checkpoints(&trace, showcase.state.frame);
        assert!(frames.len() >= 8);
        assert!(frames.len() <= MAX_SHOWCASE_FRAMES + 1);
        assert!(frames.windows(2).all(|pair| pair[0].frame < pair[1].frame));
    }

    #[test]
    fn large_crt_replays_native_frames_during_gameplay_not_outro() {
        let trace = Machine::run_match("active-crt-render", 5000);
        let source = String::from("<svg data-frontpage-version=\"physical-die-v2\"></svg>");
        let output = apply(source, &trace, crate::frontpage::render_config());
        assert!(output.contains("id=\"v2-final-crt-focus\""));
        assert!(output.contains("data-showcase-live=\"true\""));
        assert!(output.contains("data-showcase-alive=\""));
        assert!(output.contains("data-showcase-enemy-shots=\""));
        assert!(output.contains("data-showcase-player-shot=\""));
        assert!(output.matches("data-showcase-vram-frame=\"").count() >= 8);
        assert!(output.contains("ACTIVE NATIVE VRAM REPLAY"));
        assert!(!output.contains("FINAL NATIVE VRAM"));
        assert!(!output.contains("<script"));
    }
}
