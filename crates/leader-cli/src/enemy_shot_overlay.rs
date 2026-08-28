use leader_core::{FrameState, MatchTrace, Topology, ENEMY_SHOT_SLOTS};
use leader_svg::RenderConfig;

const MAX_PRESENTED_FRAMES: usize = 84;

#[must_use]
pub fn apply(
    mut svg: String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
) -> String {
    if trace.frames.is_empty() || trace.total_frames == 0 {
        return svg;
    }
    let overlay = render(topology, trace, config);
    let Some(svg_close) = svg.rfind("</svg>") else {
        return svg;
    };
    let Some(world_close) = svg[..svg_close].rfind("</g>") else {
        return svg;
    };
    svg.insert_str(world_close, &overlay);
    svg
}

fn render(topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    let total = config.total();
    let frames = sampled_frames(&trace.frames);
    let mut out = String::with_capacity(frames.len() * 1800);
    out.push_str("<g id=\"m3-enemy-shot-bank\">\n");

    for frame in frames {
        let active_count = frame.enemy_shots.iter().flatten().count();
        let moment = trace_moment(frame.frame, trace, config) + 0.010;
        let k1 = norm(moment, total);
        let k2 = norm(moment + 0.020, total);
        let k3 = norm(moment + 0.145, total);
        out.push_str(&format!(
            "<g opacity=\"0\" data-enemy-shot-frame=\"{}\" data-enemy-shot-active-count=\"{}\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>",
            frame.frame, active_count
        ));

        glow(topology, &mut out, "enemyShotAlloc", "#ef7caf");
        glow(topology, &mut out, "enemyShotCooldown", "#67d9b3");
        for slot in 0..ENEMY_SHOT_SLOTS {
            match frame.enemy_shots[slot] {
                Some(shot) => {
                    out.push_str(&format!(
                        "<g data-enemy-shot-slot=\"{slot}\" data-enemy-shot-active=\"1\" data-enemy-shot-x=\"{}\" data-enemy-shot-y=\"{}\">",
                        shot.x, shot.y
                    ));
                    glow(topology, &mut out, &format!("enemyShot{slot}X"), "#6dcff6");
                    glow(topology, &mut out, &format!("enemyShot{slot}Y"), "#e8e677");
                    glow(
                        topology,
                        &mut out,
                        &format!("enemyShot{slot}Active"),
                        "#ff8065",
                    );
                    out.push_str("</g>");
                }
                None => {
                    out.push_str(&format!(
                        "<g data-enemy-shot-slot=\"{slot}\" data-enemy-shot-active=\"0\" data-enemy-shot-x=\"0\" data-enemy-shot-y=\"0\"></g>"
                    ));
                }
            }
        }
        out.push_str("</g>\n");
    }

    out.push_str("</g>\n");
    out
}

fn sampled_frames(frames: &[FrameState]) -> Vec<&FrameState> {
    if frames.len() <= MAX_PRESENTED_FRAMES {
        return frames.iter().collect();
    }

    let stride = frames.len().div_ceil(MAX_PRESENTED_FRAMES.saturating_sub(12).max(1));
    let mut out = Vec::with_capacity(MAX_PRESENTED_FRAMES + 12);
    let mut slot_seen = [false; ENEMY_SHOT_SLOTS];
    let mut concurrent_seen = false;
    let mut previous_active = 0usize;

    for (index, frame) in frames.iter().enumerate() {
        let active = frame.enemy_shots.iter().flatten().count();
        let lifecycle_transition = index > 0 && active != previous_active;
        let first_concurrent = active >= 2 && !concurrent_seen;
        if first_concurrent {
            concurrent_seen = true;
        }

        let mut first_slot = false;
        for (slot, projectile) in frame.enemy_shots.iter().enumerate() {
            if projectile.is_some() && !slot_seen[slot] {
                slot_seen[slot] = true;
                first_slot = true;
            }
        }

        if index == 0
            || index + 1 == frames.len()
            || index % stride == 0
            || lifecycle_transition
            || first_concurrent
            || first_slot
        {
            if out
                .last()
                .is_none_or(|last: &&FrameState| !std::ptr::eq(*last, frame))
            {
                out.push(frame);
            }
        }
        previous_active = active;
    }
    out
}

fn glow(topology: &Topology, out: &mut String, id: &str, color: &str) {
    let Some(node) = topology.node(id) else {
        return;
    };
    let b = node.bounds;
    out.push_str(&format!(
        "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"7\" fill=\"{color}\" fill-opacity=\".24\" stroke=\"{color}\" stroke-width=\"7\" filter=\"url(#glow)\"/>",
        b.x - 3.0,
        b.y - 3.0,
        b.w + 6.0,
        b.h + 6.0
    ));
}

fn trace_moment(frame: u32, trace: &MatchTrace, config: RenderConfig) -> f32 {
    config.game_start()
        + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
}

fn norm(value: f32, total: f32) -> f32 {
    (value / total).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::{build_topology, Machine};

    #[test]
    fn shot_bank_overlay_exposes_all_slots_and_concurrency() {
        let topology = build_topology();
        let trace = Machine::run_match("m3-shot-overlay", 5000);
        let svg = render(&topology, &trace, RenderConfig::default());
        assert!(svg.contains("id=\"m3-enemy-shot-bank\""));
        for slot in 0..ENEMY_SHOT_SLOTS {
            assert!(svg.contains(&format!("data-enemy-shot-slot=\"{slot}\"")));
            assert!(svg.contains(&format!(
                "data-enemy-shot-slot=\"{slot}\" data-enemy-shot-active=\"1\""
            )));
        }
        assert!(svg.contains("data-enemy-shot-active-count=\"2\"")
            || svg.contains("data-enemy-shot-active-count=\"3\""));
    }

    #[test]
    fn shot_bank_sampling_is_bounded_and_keeps_hardware_use() {
        let trace = Machine::run_match("m3-shot-sampling", 5000);
        let sampled = sampled_frames(&trace.frames);
        assert!(sampled.len() <= MAX_PRESENTED_FRAMES + 12);
        assert!(sampled
            .iter()
            .any(|frame| frame.enemy_shots.iter().flatten().count() >= 2));
        for slot in 0..ENEMY_SHOT_SLOTS {
            assert!(sampled.iter().any(|frame| frame.enemy_shots[slot].is_some()));
        }
    }
}
