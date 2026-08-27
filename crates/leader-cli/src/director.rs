use leader_core::{MatchTrace, Rect, Topology};
use leader_svg::RenderConfig;

const VIEW_ASPECT: f32 = 864.0 / 484.0;

#[must_use]
pub fn apply_camera(svg: String, topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    let Some(start) = svg.find("<animate attributeName=\"viewBox\"") else { return svg; };
    let Some(relative_end) = svg[start..].find("/>") else { return svg; };
    let end = start + relative_end + 2;
    let replacement = camera_animation(topology, trace, config);
    let mut out = String::with_capacity(svg.len() + replacement.len());
    out.push_str(&svg[..start]);
    out.push_str(&replacement);
    out.push_str(&svg[end..]);
    out
}

fn camera_animation(topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    let total = config.total();
    let full = aspect_rect(Rect::new(0.0, 0.0, topology.width, topology.height), 0.0);
    let mut track = vec![(0.0, full)];

    // Assembly: approach -> tight close-up -> hold. Each subsystem gets a visible shot.
    let mut groups = topology.groups.clone();
    groups.sort_by_key(|group| group.assembly_rank);
    let span = config.assembly_seconds / groups.len().max(1) as f32;
    for (index, group) in groups.iter().enumerate() {
        let t = index as f32 * span;
        track.push((t, focus(group.bounds, 240.0)));
        track.push((t + span * 0.20, focus(group.bounds, 38.0)));
        track.push((t + span * 0.78, focus(group.bounds, 38.0)));
        track.push((t + span * 0.94, focus(group.bounds, 150.0)));
    }
    track.push((config.assembly_seconds, full));

    // Boot: deliberately follow the causal path instead of cycling arbitrary regions.
    let boot = config.assembly_seconds;
    shot_group(&mut track, topology, boot + 0.25, "clk", 24.0);
    shot_group(&mut track, topology, boot + 1.25, "pc", 30.0);
    shot_group(&mut track, topology, boot + 2.35, "romsys", 28.0);
    shot_group(&mut track, topology, boot + 3.55, "decode", 28.0);
    shot_group(&mut track, topology, boot + 4.75, "regs", 34.0);
    shot_group(&mut track, topology, boot + 5.95, "alu", 24.0);
    shot_group(&mut track, topology, boot + 7.10, "ramsys", 55.0);
    shot_group(&mut track, topology, boot + 8.15, "gpu", 34.0);
    track.push((config.game_start(), full));

    // First seconds of execution: an instruction travels through the actual machine.
    let game = config.game_start();
    shot_group(&mut track, topology, game + 0.35, "pc", 18.0);
    shot_group(&mut track, topology, game + 1.55, "romsys", 20.0);
    shot_group(&mut track, topology, game + 2.80, "decode", 18.0);
    shot_group(&mut track, topology, game + 4.10, "regs", 22.0);
    shot_group(&mut track, topology, game + 5.35, "alu", 14.0);
    shot_group(&mut track, topology, game + 6.75, "ramsys", 35.0);
    shot_group(&mut track, topology, game + 8.15, "bus", 26.0);
    shot_group(&mut track, topology, game + 9.55, "vramsys", 20.0);
    shot_group(&mut track, topology, game + 10.95, "gpu", 20.0);

    // Let the audience understand where the picture comes from, then enter the monitor.
    if let Some(display) = topology.node("display") {
        track.push((game + 12.20, focus(display.bounds, 140.0)));
        track.push((game + 13.40, focus(display.bounds, 35.0)));
        track.push((game + 15.20, display_screen(display.bounds)));

        // Mid-game micro cutaways are tied to actual kill progress, not decorative timing.
        for (kill_index, target_group) in [(8usize, "alu"), (16, "ramsys"), (24, "gpu")] {
            if let Some(kill) = trace.kills.get(kill_index.saturating_sub(1)) {
                let fraction = kill.frame as f32 / trace.total_frames.max(1) as f32;
                let t = game + fraction * config.game_seconds;
                if t > game + 17.0 && t < config.game_end() - 8.0 {
                    track.push((t - 0.45, display_screen(display.bounds)));
                    shot_group(&mut track, topology, t, target_group, 18.0);
                    track.push((t + 0.85, display_screen(display.bounds)));
                }
            }
        }
        track.push((config.game_end() - 2.8, display_screen(display.bounds)));
        track.push((config.game_end() - 0.55, focus(display.bounds, 18.0)));
    }

    // Outro reveals the entire machine that produced the game.
    track.push((config.game_end() + 1.2, full));
    track.push((total - 0.2, full));
    track.sort_by(|left, right| left.0.total_cmp(&right.0));
    dedupe_times(&mut track);

    let values = track.iter().map(|(_, r)| format!("{:.1} {:.1} {:.1} {:.1}", r.x, r.y, r.w, r.h)).collect::<Vec<_>>().join(";");
    let keys = track.iter().map(|(t, _)| format!("{:.6}", norm(*t, total))).collect::<Vec<_>>().join(";");
    format!("<animate attributeName=\"viewBox\" values=\"{values}\" keyTimes=\"{keys}\" dur=\"{total:.3}s\" repeatCount=\"indefinite\" calcMode=\"linear\"/>")
}

fn shot_group(track: &mut Vec<(f32, Rect)>, topology: &Topology, time: f32, id: &str, pad: f32) {
    if let Some(group) = topology.group(id) { track.push((time, focus(group.bounds, pad))); }
}

fn display_screen(bounds: Rect) -> Rect {
    // The rendered 128x96 framebuffer sits inside the display node with a small bezel.
    aspect_rect(Rect::new(bounds.x + 35.0, bounds.y + 34.0, bounds.w - 70.0, bounds.h - 66.0), 0.0)
}

fn focus(bounds: Rect, padding: f32) -> Rect { aspect_rect(bounds, padding) }

fn aspect_rect(bounds: Rect, padding: f32) -> Rect {
    let mut x = bounds.x - padding;
    let mut y = bounds.y - padding;
    let mut w = (bounds.w + padding * 2.0).max(1.0);
    let mut h = (bounds.h + padding * 2.0).max(1.0);
    let aspect = w / h;
    if aspect > VIEW_ASPECT {
        let wanted_h = w / VIEW_ASPECT;
        y -= (wanted_h - h) * 0.5;
        h = wanted_h;
    } else {
        let wanted_w = h * VIEW_ASPECT;
        x -= (wanted_w - w) * 0.5;
        w = wanted_w;
    }
    Rect::new(x, y, w, h)
}

fn dedupe_times(track: &mut Vec<(f32, Rect)>) {
    let mut last = -1.0_f32;
    for (time, _) in track.iter_mut() {
        if *time <= last { *time = last + 0.001; }
        last = *time;
    }
}

fn norm(value: f32, total: f32) -> f32 { (value / total).clamp(0.0, 1.0) }

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::{build_topology, Machine};

    #[test]
    fn director_replaces_renderer_camera_with_many_closeups() {
        let topology = build_topology();
        let trace = Machine::run_match("director-test", 5000);
        let source = "<svg><animate attributeName=\"viewBox\" values=\"0 0 1 1\"/></svg>".to_owned();
        let output = apply_camera(source, &topology, &trace, RenderConfig::default());
        assert!(output.matches("attributeName=\"viewBox\"").count() == 1);
        assert!(output.matches(';').count() > 40);
        assert!(output.contains("dur=\"138.000s\""));
    }
}
