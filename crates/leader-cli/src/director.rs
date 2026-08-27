use leader_core::{MatchTrace, Rect, Topology};
use leader_svg::RenderConfig;

const VIEW_W: f32 = 864.0;
const VIEW_H: f32 = 484.0;
const VIEW_ASPECT: f32 = VIEW_W / VIEW_H;

#[must_use]
pub fn apply_camera(
    mut svg: String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
) -> String {
    // GitHub/browser rendering of an animated nested SVG viewBox proved unreliable.
    // Keep a fixed viewport and transform the entire world instead. CSS transforms
    // are the same mechanism already used by the public Leader.svg animation.
    let Some(camera_start) = svg.find("<svg class=\"animated\" id=\"camera\"") else {
        return svg;
    };
    let Some(open_rel_end) = svg[camera_start..].find('>') else {
        return svg;
    };
    let open_end = camera_start + open_rel_end + 1;

    let opening = &svg[camera_start..open_end];
    let Some(viewbox_start_rel) = opening.find("viewBox=\"") else {
        return svg;
    };
    let viewbox_value_start = camera_start + viewbox_start_rel + "viewBox=\"".len();
    let Some(viewbox_value_end_rel) = svg[viewbox_value_start..].find('"') else {
        return svg;
    };
    let viewbox_value_end = viewbox_value_start + viewbox_value_end_rel;
    svg.replace_range(
        viewbox_value_start..viewbox_value_end,
        &format!("0 0 {VIEW_W:.0} {VIEW_H:.0}"),
    );

    // Recompute the opening tag end because replacing the viewBox changed offsets.
    let Some(camera_start) = svg.find("<svg class=\"animated\" id=\"camera\"") else {
        return svg;
    };
    let Some(open_rel_end) = svg[camera_start..].find('>') else {
        return svg;
    };
    let open_end = camera_start + open_rel_end + 1;

    // The renderer writes one viewport background immediately after the nested SVG.
    // Keep it outside the moving world so zoomed shots never expose transparent edges.
    let background = "<rect width=\"100%\" height=\"100%\" fill=\"#07101a\"/>";
    let world_insert = if let Some(bg_rel) = svg[open_end..].find(background) {
        open_end + bg_rel + background.len()
    } else {
        open_end
    };

    let css = camera_css(topology, trace, config);
    svg.insert_str(world_insert, &format!("{css}<g id=\"camera-world\">"));

    // The old renderer camera is always the final element inside the nested SVG.
    // Remove it and use that exact position to close the moving world group.
    let Some(old_camera_start) = svg.find("<animate attributeName=\"viewBox\"") else {
        return svg;
    };
    let Some(old_camera_end_rel) = svg[old_camera_start..].find("/>") else {
        return svg;
    };
    let old_camera_end = old_camera_start + old_camera_end_rel + 2;
    svg.replace_range(old_camera_start..old_camera_end, "</g>");
    svg
}

fn camera_css(topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    let total = config.total();
    let track = camera_track(topology, trace, config);
    let mut rules = String::with_capacity(track.len() * 80);

    for (time, rect) in track {
        let percent = norm(time, total) * 100.0;
        let matrix = view_matrix(rect);
        rules.push_str(&format!(
            "{percent:.6}%{{transform:matrix({:.7},0,0,{:.7},{:.3},{:.3})}}",
            matrix.scale, matrix.scale, matrix.tx, matrix.ty
        ));
    }

    format!(
        "<style>@keyframes leaderCamera{{{rules}}}#camera-world{{transform-box:view-box;transform-origin:0 0;animation:leaderCamera {total:.3}s linear infinite}}</style>"
    )
}

#[derive(Debug, Clone, Copy)]
struct ViewMatrix {
    scale: f32,
    tx: f32,
    ty: f32,
}

fn view_matrix(rect: Rect) -> ViewMatrix {
    let scale = (VIEW_W / rect.w).min(VIEW_H / rect.h);
    let rendered_w = rect.w * scale;
    let rendered_h = rect.h * scale;
    let tx = (VIEW_W - rendered_w) * 0.5 - rect.x * scale;
    let ty = (VIEW_H - rendered_h) * 0.5 - rect.y * scale;
    ViewMatrix { scale, tx, ty }
}

fn camera_track(
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
) -> Vec<(f32, Rect)> {
    let total = config.total();
    let full = aspect_rect(Rect::new(0.0, 0.0, topology.width, topology.height), 0.0);
    let mut track = vec![(0.0, full)];

    // Assembly: approach -> tight close-up -> hold -> release.
    let mut groups = topology.groups.clone();
    groups.sort_by_key(|group| group.assembly_rank);
    let span = config.assembly_seconds / groups.len().max(1) as f32;
    for (index, group) in groups.iter().enumerate() {
        let start = index as f32 * span;
        // First group gets a brief establishing shot before the first zoom.
        if index == 0 {
            track.push((0.55, full));
        }
        track.push((start + 0.70, focus(group.bounds, 180.0)));
        track.push((start + span * 0.34, focus(group.bounds, 28.0)));
        track.push((start + span * 0.78, focus(group.bounds, 28.0)));
        track.push((start + span * 0.96, focus(group.bounds, 120.0)));
    }
    track.push((config.assembly_seconds, full));

    // Boot: follow the causal hardware path with deliberately tight framing.
    let boot = config.assembly_seconds;
    hold_group(&mut track, topology, boot + 0.20, "clk", 18.0, 0.72);
    hold_group(&mut track, topology, boot + 1.20, "pc", 20.0, 0.78);
    hold_group(&mut track, topology, boot + 2.25, "romsys", 18.0, 0.82);
    hold_group(&mut track, topology, boot + 3.35, "decode", 18.0, 0.82);
    hold_group(&mut track, topology, boot + 4.45, "regs", 20.0, 0.82);
    hold_group(&mut track, topology, boot + 5.55, "alu", 12.0, 0.82);
    hold_group(&mut track, topology, boot + 6.65, "ramsys", 32.0, 0.72);
    hold_group(&mut track, topology, boot + 7.70, "gpu", 20.0, 0.80);
    track.push((config.game_start(), full));

    // Beginning of execution: visually follow a real instruction through the machine.
    let game = config.game_start();
    hold_group(&mut track, topology, game + 0.25, "pc", 12.0, 0.82);
    hold_group(&mut track, topology, game + 1.40, "romsys", 14.0, 0.82);
    hold_group(&mut track, topology, game + 2.60, "decode", 12.0, 0.86);
    hold_group(&mut track, topology, game + 3.85, "regs", 14.0, 0.86);
    hold_group(&mut track, topology, game + 5.10, "alu", 8.0, 0.92);
    hold_group(&mut track, topology, game + 6.45, "ramsys", 24.0, 0.88);
    hold_group(&mut track, topology, game + 7.75, "bus", 16.0, 0.84);
    hold_group(&mut track, topology, game + 9.00, "vramsys", 12.0, 0.86);
    hold_group(&mut track, topology, game + 10.30, "gpu", 12.0, 0.86);

    if let Some(display) = topology.node("display") {
        // Approach the monitor, then push through the bezel into the framebuffer.
        track.push((game + 11.45, focus(display.bounds, 160.0)));
        track.push((game + 12.35, focus(display.bounds, 42.0)));
        track.push((game + 13.15, display_screen(display.bounds)));
        track.push((game + 14.55, display_screen(display.bounds)));

        // Three brief cutaways are causally tied to kill milestones.
        for (kill_index, target_group) in [(8usize, "alu"), (16, "ramsys"), (24, "gpu")] {
            if let Some(kill) = trace.kills.get(kill_index.saturating_sub(1)) {
                let fraction = kill.frame as f32 / trace.total_frames.max(1) as f32;
                let time = game + fraction * config.game_seconds;
                if time > game + 17.0 && time < config.game_end() - 8.0 {
                    track.push((time - 0.55, display_screen(display.bounds)));
                    if let Some(group) = topology.group(target_group) {
                        track.push((time, focus(group.bounds, 12.0)));
                        track.push((time + 0.72, focus(group.bounds, 12.0)));
                    }
                    track.push((time + 1.15, display_screen(display.bounds)));
                }
            }
        }

        track.push((config.game_end() - 3.2, display_screen(display.bounds)));
        track.push((config.game_end() - 1.0, display_screen(display.bounds)));
        track.push((config.game_end() - 0.30, focus(display.bounds, 18.0)));
    }

    // Pull back enough that the viewer finally understands the complete machine.
    track.push((config.game_end() + 1.35, full));
    track.push((total - 0.20, full));
    track.sort_by(|left, right| left.0.total_cmp(&right.0));
    dedupe_times(&mut track);
    track
}

fn hold_group(
    track: &mut Vec<(f32, Rect)>,
    topology: &Topology,
    time: f32,
    id: &str,
    padding: f32,
    hold: f32,
) {
    if let Some(group) = topology.group(id) {
        let shot = focus(group.bounds, padding);
        track.push((time, shot));
        track.push((time + hold, shot));
    }
}

fn display_screen(bounds: Rect) -> Rect {
    // The framebuffer is rendered at +53,+55 from the display node and is
    // 128×96 scaled by 2.42 in leader-svg. Frame it tightly but keep a hint of bezel.
    aspect_rect(
        Rect::new(bounds.x + 43.0, bounds.y + 45.0, 128.0 * 2.42 + 20.0, 96.0 * 2.42 + 20.0),
        0.0,
    )
}

fn focus(bounds: Rect, padding: f32) -> Rect {
    aspect_rect(bounds, padding)
}

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

fn dedupe_times(track: &mut [(f32, Rect)]) {
    let mut last = -1.0_f32;
    for (time, _) in track {
        if *time <= last {
            *time = last + 0.001;
        }
        last = *time;
    }
}

fn norm(value: f32, total: f32) -> f32 {
    (value / total).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::{build_topology, Machine};

    #[test]
    fn director_replaces_viewbox_animation_with_css_world_camera() {
        let topology = build_topology();
        let trace = Machine::run_match("director-test", 5000);
        let source = format!(
            "<svg><svg class=\"animated\" id=\"camera\" width=\"864\" height=\"484\" viewBox=\"0 0 {} {}\"><rect width=\"100%\" height=\"100%\" fill=\"#07101a\"/><g id=\"content\"/><animate attributeName=\"viewBox\" values=\"0 0 1 1\"/></svg></svg>",
            topology.width, topology.height
        );
        let output = apply_camera(source, &topology, &trace, RenderConfig::default());
        assert!(!output.contains("attributeName=\"viewBox\""));
        assert!(output.contains("viewBox=\"0 0 864 484\""));
        assert!(output.contains("@keyframes leaderCamera"));
        assert!(output.contains("id=\"camera-world\""));
        assert!(output.contains("transform:matrix("));
        assert!(output.contains("animation:leaderCamera 138.000s linear infinite"));
    }

    #[test]
    fn closeup_matrix_is_much_larger_than_establishing_shot() {
        let topology = build_topology();
        let full = aspect_rect(Rect::new(0.0, 0.0, topology.width, topology.height), 0.0);
        let clock = focus(topology.group("clk").expect("clock group").bounds, 18.0);
        let full_matrix = view_matrix(full);
        let clock_matrix = view_matrix(clock);
        assert!(clock_matrix.scale > full_matrix.scale * 5.0);
    }
}
