use leader_core::{bit16, bit8, derive_datapath, MatchTrace, Rect, Topology};
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
    let Some(camera_start) = svg.find("<svg class=\"animated\" id=\"camera\"") else { return svg; };
    let Some(open_rel_end) = svg[camera_start..].find('>') else { return svg; };
    let open_end = camera_start + open_rel_end + 1;
    let opening = &svg[camera_start..open_end];
    let Some(viewbox_start_rel) = opening.find("viewBox=\"") else { return svg; };
    let viewbox_value_start = camera_start + viewbox_start_rel + "viewBox=\"".len();
    let Some(viewbox_value_end_rel) = svg[viewbox_value_start..].find('"') else { return svg; };
    let viewbox_value_end = viewbox_value_start + viewbox_value_end_rel;
    svg.replace_range(viewbox_value_start..viewbox_value_end, &format!("0 0 {VIEW_W:.0} {VIEW_H:.0}"));

    let Some(camera_start) = svg.find("<svg class=\"animated\" id=\"camera\"") else { return svg; };
    let Some(open_rel_end) = svg[camera_start..].find('>') else { return svg; };
    let open_end = camera_start + open_rel_end + 1;
    let background = "<rect width=\"100%\" height=\"100%\" fill=\"#07101a\"/>";
    let world_insert = svg[open_end..]
        .find(background)
        .map_or(open_end, |offset| open_end + offset + background.len());

    let css = camera_css(topology, trace, config);
    svg.insert_str(world_insert, &format!("{css}<g id=\"camera-world\">"));

    let Some(old_camera_start) = svg.find("<animate attributeName=\"viewBox\"") else { return svg; };
    let Some(old_camera_end_rel) = svg[old_camera_start..].find("/>") else { return svg; };
    let old_camera_end = old_camera_start + old_camera_end_rel + 2;
    let f3 = render_f3_datapath(topology, trace, config);
    svg.replace_range(old_camera_start..old_camera_end, &format!("{f3}</g>"));
    svg
}

fn render_f3_datapath(topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    if trace.total_frames == 0 { return String::new(); }
    let events = derive_datapath(trace);
    let stride = (events.len() / 180).max(1);
    let total = config.total();
    let mut out = String::with_capacity(500_000);
    out.push_str("<g id=\"f3-datapath\">\n");
    for event in events.iter().step_by(stride) {
        if !matches!(event.phase, leader_core::PhaseKind::Fetch | leader_core::PhaseKind::Decode) { continue; }
        let moment = config.game_start()
            + event.frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
            + f32::from(event.ordinal.min(12)) * 0.003;
        let k1 = norm(moment, total);
        let k2 = norm(moment + 0.035, total);
        let k3 = norm(moment + 0.16, total);
        out.push_str(&format!("<g opacity=\"0\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>"));
        for bit in 0..16 {
            if bit16(event.state.pc, bit) { glow_node(&mut out, topology, &format!("pcBit{bit}"), "#67d9b3"); }
            if bit16(event.state.mar, bit) { glow_node(&mut out, topology, &format!("marBit{bit}"), "#f2ae4f"); }
        }
        for bit in 0..8 {
            if bit8(event.state.mdr, bit) { glow_node(&mut out, topology, &format!("mdrBit{bit}"), "#4bc8f3"); }
            if bit8(event.state.ir, bit) { glow_node(&mut out, topology, &format!("irBit{bit}"), "#ef7caf"); }
        }
        out.push_str("</g>\n");
    }
    out.push_str("</g>\n");
    out
}

fn glow_node(out: &mut String, topology: &Topology, id: &str, color: &str) {
    let Some(node) = topology.node(id) else { return; };
    let b = node.bounds;
    out.push_str(&format!(
        "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"8\" fill=\"{}\" fill-opacity=\".18\" stroke=\"{}\" stroke-width=\"9\" filter=\"url(#glow)\"/>",
        b.x - 3.0, b.y - 3.0, b.w + 6.0, b.h + 6.0, color, color
    ));
}

fn camera_css(topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    let total = config.total();
    let track = camera_track(topology, trace, config);
    let mut rules = String::with_capacity(track.len() * 80);
    for (time, rect) in track {
        let percent = norm(time, total) * 100.0;
        let matrix = view_matrix(rect);
        rules.push_str(&format!("{percent:.6}%{{transform:matrix({:.7},0,0,{:.7},{:.3},{:.3})}}", matrix.scale, matrix.scale, matrix.tx, matrix.ty));
    }
    format!("<style>@keyframes leaderCamera{{{rules}}}#camera-world{{transform-box:view-box;transform-origin:0 0;animation:leaderCamera {total:.3}s linear infinite}}</style>")
}

#[derive(Debug, Clone, Copy)]
struct ViewMatrix { scale: f32, tx: f32, ty: f32 }

fn view_matrix(rect: Rect) -> ViewMatrix {
    let scale = (VIEW_W / rect.w).min(VIEW_H / rect.h);
    let rendered_w = rect.w * scale;
    let rendered_h = rect.h * scale;
    let tx = (VIEW_W - rendered_w) * 0.5 - rect.x * scale;
    let ty = (VIEW_H - rendered_h) * 0.5 - rect.y * scale;
    ViewMatrix { scale, tx, ty }
}

fn camera_track(topology: &Topology, _trace: &MatchTrace, config: RenderConfig) -> Vec<(f32, Rect)> {
    let total = config.total();
    let full = aspect_rect(Rect::new(0.0, 0.0, topology.width, topology.height), 0.0);
    let mut track = vec![(0.0, full)];
    let mut groups = topology.groups.clone();
    groups.sort_by_key(|group| group.assembly_rank);
    let span = config.assembly_seconds / groups.len().max(1) as f32;
    for (index, group) in groups.iter().enumerate() {
        let start = index as f32 * span;
        if index == 0 { track.push((0.55, full)); }
        track.push((start + 0.70, focus(group.bounds, 180.0)));
        track.push((start + span * 0.34, focus(group.bounds, 34.0)));
        track.push((start + span * 0.78, focus(group.bounds, 34.0)));
        track.push((start + span * 0.96, focus(group.bounds, 130.0)));
    }
    track.push((config.assembly_seconds, full));

    let boot = config.assembly_seconds;
    hold_group(&mut track, topology, boot + 0.20, "clk", 22.0, 0.72);
    hold_group(&mut track, topology, boot + 1.20, "pc", 26.0, 0.78);
    hold_group(&mut track, topology, boot + 2.25, "romsys", 24.0, 0.82);
    hold_group(&mut track, topology, boot + 3.35, "decode", 24.0, 0.82);
    hold_group(&mut track, topology, boot + 4.45, "regs", 28.0, 0.82);
    hold_group(&mut track, topology, boot + 5.55, "alu", 20.0, 0.82);
    hold_group(&mut track, topology, boot + 6.65, "ramsys", 40.0, 0.72);
    hold_group(&mut track, topology, boot + 7.70, "gpu", 28.0, 0.80);
    track.push((config.game_start(), full));

    let game = config.game_start();
    let global_observe_end = (game + 15.0).min(config.game_end() - 8.0);
    track.push((game + 0.40, full));
    track.push((global_observe_end, full));

    if let Some(display) = topology.node("display") {
        track.push((global_observe_end + 0.80, focus(display.bounds, 180.0)));
        track.push((global_observe_end + 1.80, focus(display.bounds, 48.0)));
        track.push((global_observe_end + 2.80, display_screen(display.bounds)));
        track.push((config.game_end() + config.outro_seconds - 0.20, display_screen(display.bounds)));
    }
    track.push((total - 0.05, full));
    track.sort_by(|left, right| left.0.total_cmp(&right.0));
    dedupe_times(&mut track);
    track
}

fn hold_group(track: &mut Vec<(f32, Rect)>, topology: &Topology, time: f32, id: &str, padding: f32, hold: f32) {
    if let Some(group) = topology.group(id) {
        let shot = focus(group.bounds, padding);
        track.push((time, shot));
        track.push((time + hold, shot));
    }
}

fn display_screen(bounds: Rect) -> Rect {
    aspect_rect(Rect::new(bounds.x + 43.0, bounds.y + 45.0, 128.0 * 2.42 + 20.0, 96.0 * 2.42 + 20.0), 0.0)
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

fn dedupe_times(track: &mut [(f32, Rect)]) {
    let mut last = -1.0_f32;
    for (time, _) in track {
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
    fn director_replaces_viewbox_animation_with_css_world_camera() {
        let topology = build_topology();
        let trace = Machine::run_match("director-test", 5000);
        let source = format!("<svg><svg class=\"animated\" id=\"camera\" width=\"864\" height=\"484\" viewBox=\"0 0 {} {}\"><rect width=\"100%\" height=\"100%\" fill=\"#07101a\"/><g id=\"content\"/><animate attributeName=\"viewBox\" values=\"0 0 1 1\"/></svg></svg>", topology.width, topology.height);
        let output = apply_camera(source, &topology, &trace, RenderConfig::default());
        assert!(!output.contains("attributeName=\"viewBox\""));
        assert!(output.contains("viewBox=\"0 0 864 484\""));
        assert!(output.contains("@keyframes leaderCamera"));
        assert!(output.contains("id=\"camera-world\""));
        assert!(output.contains("id=\"f3-datapath\""));
    }

    #[test]
    fn closeup_matrix_is_much_larger_than_establishing_shot() {
        let topology = build_topology();
        let full = aspect_rect(Rect::new(0.0, 0.0, topology.width, topology.height), 0.0);
        let clock = focus(topology.group("clk").expect("clock group").bounds, 22.0);
        assert!(view_matrix(clock).scale > view_matrix(full).scale * 4.0);
    }
}
