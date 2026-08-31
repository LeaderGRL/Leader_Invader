use std::fmt::Write;

use leader_core::{
    build_navigation, CameraView, DetailDensity, MatchTrace, NavigationLevel, NavigationModel, Rect,
    Topology,
};
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
    let navigation = build_navigation(topology);
    annotate_node_membership(&mut svg, &navigation);

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

    let Some(camera_start) = svg.find("<svg class=\"animated\" id=\"camera\"") else {
        return svg;
    };
    let Some(open_rel_end) = svg[camera_start..].find('>') else {
        return svg;
    };
    let open_end = camera_start + open_rel_end + 1;
    let background = "<rect width=\"100%\" height=\"100%\" fill=\"#07101a\"/>";
    let world_insert = svg[open_end..]
        .find(background)
        .map_or(open_end, |offset| open_end + offset + background.len());

    let css = camera_css(&navigation, topology, trace, config);
    let overlay = navigation_overlay(&navigation);
    svg.insert_str(
        world_insert,
        &format!("{css}<g id=\"camera-world\">{overlay}"),
    );

    let Some(old_camera_start) = svg.find("<animate attributeName=\"viewBox\"") else {
        return svg;
    };
    let Some(old_camera_end_rel) = svg[old_camera_start..].find("/>") else {
        return svg;
    };
    let old_camera_end = old_camera_start + old_camera_end_rel + 2;

    // Native overlays own datapath activity. The director only frames the
    // physical hierarchy and closes the shared camera world here.
    svg.replace_range(old_camera_start..old_camera_end, "</g>");
    svg
}

fn annotate_node_membership(svg: &mut String, navigation: &NavigationModel) {
    for module in navigation
        .modules
        .iter()
        .filter(|module| module.level == NavigationLevel::Subsystem)
    {
        for node_id in &module.node_ids {
            insert_node_attribute(
                svg,
                node_id,
                &format!(" data-subsystem=\"{}\"", xml_escape(&module.id)),
            );
        }
    }

    for module in navigation
        .modules
        .iter()
        .filter(|module| module.level == NavigationLevel::Detail)
    {
        let density = navigation
            .view_for_module(&module.id)
            .map(|view| view.density.as_str())
            .unwrap_or(DetailDensity::BitExact.as_str());
        for node_id in &module.node_ids {
            insert_node_attribute(
                svg,
                node_id,
                &format!(
                    " data-detail-module=\"{}\" data-detail-density=\"{density}\"",
                    xml_escape(&module.id)
                ),
            );
        }
    }
}

fn insert_node_attribute(svg: &mut String, node_id: &str, attribute: &str) {
    let marker = format!("<g id=\"node-{}\"", xml_escape(node_id));
    let Some(start) = svg.find(&marker) else {
        return;
    };
    let insert_at = start + marker.len();
    svg.insert_str(insert_at, attribute);
}

fn navigation_overlay(navigation: &NavigationModel) -> String {
    let mut out = String::with_capacity(navigation.views.len() * 360);
    let _ = write!(
        out,
        "<g id=\"navigation-hierarchy\" data-default-view=\"{}\" data-view-count=\"{}\" aria-hidden=\"true\">",
        xml_escape(&navigation.default_view),
        navigation.views.len()
    );
    for view in &navigation.views {
        if view.level == NavigationLevel::Machine {
            continue;
        }
        render_view_boundary(&mut out, view);
    }
    out.push_str("</g>");
    out
}

fn render_view_boundary(out: &mut String, view: &CameraView) {
    let class = match view.level {
        NavigationLevel::Machine => "nav-machine",
        NavigationLevel::Subsystem => "nav-subsystem",
        NavigationLevel::Detail => "nav-detail",
    };
    let parent = view.parent_view.as_deref().unwrap_or("");
    let label_y = view.bounds.y + 18.0;
    let _ = write!(
        out,
        "<g id=\"nav-{}\" class=\"nav-boundary {class}\" data-view=\"{}\" data-module=\"{}\" data-parent=\"{}\" data-density=\"{}\"><rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"14\"/><text x=\"{:.1}\" y=\"{label_y:.1}\">{}</text></g>",
        xml_escape(&view.module_id),
        xml_escape(&view.id),
        xml_escape(&view.module_id),
        xml_escape(parent),
        view.density.as_str(),
        view.bounds.x,
        view.bounds.y,
        view.bounds.w,
        view.bounds.h,
        view.bounds.x + 12.0,
        xml_escape(&view.label)
    );
}

fn camera_css(
    navigation: &NavigationModel,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
) -> String {
    let total = config.total();
    let track = camera_track(navigation, topology, trace, config);
    let mut rules = String::with_capacity(track.len() * 80);
    for (time, rect) in track {
        let percent = norm(time, total) * 100.0;
        let matrix = view_matrix(rect);
        rules.push_str(&format!(
            "{percent:.6}%{{transform:matrix({:.7},0,0,{:.7},{:.3},{:.3})}}",
            matrix.scale, matrix.scale, matrix.tx, matrix.ty
        ));
    }

    let detail_begin = norm((config.assembly_seconds - 0.12).max(0.0), total) * 100.0;
    let detail_full = norm(config.assembly_seconds + 0.24, total) * 100.0;
    let detail_fade = norm(config.game_start() + 6.30, total) * 100.0;
    let detail_end = norm(config.game_start() + 6.85, total) * 100.0;
    let subsystem_soften = norm(config.assembly_seconds + 0.18, total) * 100.0;
    let subsystem_restore = norm(config.game_start() + 6.80, total) * 100.0;

    format!(
        "<style>@keyframes leaderCamera{{{rules}}}@keyframes leaderDetailLod{{0%,{detail_begin:.6}%{{opacity:0}}{detail_full:.6}%,{detail_fade:.6}%{{opacity:1}}{detail_end:.6}%,100%{{opacity:0}}}}@keyframes leaderSubsystemLod{{0%,{detail_begin:.6}%{{opacity:1}}{subsystem_soften:.6}%,{detail_fade:.6}%{{opacity:.28}}{subsystem_restore:.6}%,100%{{opacity:.72}}}}@keyframes leaderNodeKindLod{{0%,{detail_begin:.6}%{{opacity:.16}}{detail_full:.6}%,{detail_fade:.6}%{{opacity:1}}{detail_end:.6}%,100%{{opacity:.16}}}}#camera-world{{transform-box:view-box;transform-origin:0 0;animation:leaderCamera {total:.3}s linear infinite}}.nav-boundary{{pointer-events:none}}.nav-boundary rect{{fill:#08131d;fill-opacity:.08;vector-effect:non-scaling-stroke}}.nav-boundary text{{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;letter-spacing:2px;vector-effect:non-scaling-stroke}}.nav-subsystem{{animation:leaderSubsystemLod {total:.3}s linear infinite}}.nav-subsystem rect{{stroke:#44677f;stroke-width:2;stroke-dasharray:12 10;opacity:.42}}.nav-subsystem text{{fill:#5f7f95;font-size:13px;font-weight:700;opacity:.56}}.nav-detail{{opacity:0;animation:leaderDetailLod {total:.3}s linear infinite}}.nav-detail rect{{stroke:#80a7bd;stroke-width:1.5;stroke-dasharray:7 7;opacity:.50}}.nav-detail text{{fill:#91b5c7;font-size:11px;font-weight:800;opacity:.68}}.node-kind{{animation:leaderNodeKindLod {total:.3}s linear infinite}}</style>"
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
    navigation: &NavigationModel,
    topology: &Topology,
    _trace: &MatchTrace,
    config: RenderConfig,
) -> Vec<(f32, Rect)> {
    let total = config.total();
    let full = navigation
        .view(&navigation.default_view)
        .map(|view| aspect_rect(view.bounds, 0.0))
        .unwrap_or_else(|| aspect_rect(Rect::new(0.0, 0.0, topology.width, topology.height), 0.0));
    let mut track = vec![(0.0, full)];
    let mut groups = topology.groups.clone();
    groups.sort_by_key(|group| group.assembly_rank);
    let span = config.assembly_seconds / groups.len().max(1) as f32;
    for (index, group) in groups.iter().enumerate() {
        let start = index as f32 * span;
        if index == 0 {
            track.push((0.55, full));
        }
        let shot = navigation
            .view_for_module(&group.id)
            .map(|view| aspect_rect(view.bounds, 0.0))
            .unwrap_or_else(|| focus(group.bounds, 44.0));
        track.push((start + 0.70, aspect_rect(shot, 136.0)));
        track.push((start + span * 0.34, shot));
        track.push((start + span * 0.78, shot));
        track.push((start + span * 0.96, aspect_rect(shot, 86.0)));
    }
    track.push((config.assembly_seconds, full));

    let boot = config.assembly_seconds;
    hold_view(&mut track, navigation, boot + 0.15, "clk.phases", 0.48);
    hold_view(&mut track, navigation, boot + 0.72, "pc.fetch", 0.62);
    hold_view(&mut track, navigation, boot + 1.45, "romsys.decode", 0.36);
    hold_view(&mut track, navigation, boot + 1.88, "romsys.pages", 0.42);
    hold_view(&mut track, navigation, boot + 2.40, "decode.instruction", 0.46);
    hold_view(&mut track, navigation, boot + 2.96, "decode.microcode", 0.54);
    hold_view(&mut track, navigation, boot + 3.60, "regs.readwrite", 0.58);
    hold_view(&mut track, navigation, boot + 4.28, "alu.ripple", 0.76);
    hold_view(&mut track, navigation, boot + 5.14, "ramsys.decode", 0.34);
    hold_view(&mut track, navigation, boot + 5.56, "bus.arbitration", 0.42);
    hold_view(&mut track, navigation, boot + 6.08, "bus.stack", 0.52);
    hold_view(&mut track, navigation, boot + 6.70, "vramsys.decode", 0.34);
    hold_view(&mut track, navigation, boot + 7.12, "gpu.dma", 0.42);
    hold_view(&mut track, navigation, boot + 7.64, "gpu.timing", 0.46);
    hold_view(&mut track, navigation, boot + 8.20, "gpu.scanout", 0.66);
    track.push((config.game_start(), full));

    let game = config.game_start();
    track.push((game + 0.25, full));
    hold_view(&mut track, navigation, game + 0.72, "io.input_irq", 0.46);
    hold_view(&mut track, navigation, game + 1.30, "io.shift_register", 0.56);
    hold_view(&mut track, navigation, game + 1.98, "io.formation", 0.58);
    hold_view(&mut track, navigation, game + 2.68, "io.enemy_shots", 0.66);
    hold_view(&mut track, navigation, game + 3.46, "io.shields", 0.72);
    hold_view(&mut track, navigation, game + 4.30, "gpu.dma", 0.48);
    hold_view(&mut track, navigation, game + 4.90, "gpu.timing", 0.56);
    hold_view(&mut track, navigation, game + 5.58, "gpu.scanout", 0.64);

    let global_observe_end = (game + 6.40).min(config.game_end() - 8.0);
    track.push((global_observe_end, full));
    if let Some(display) = topology.node("display") {
        track.push((global_observe_end + 0.40, focus(display.bounds, 210.0)));
        track.push((global_observe_end + 1.10, focus(display.bounds, 92.0)));
        track.push((global_observe_end + 1.85, display_screen(display.bounds)));
        track.push((
            config.game_end() + config.outro_seconds - 0.20,
            display_screen(display.bounds),
        ));
    }
    track.push((total - 0.05, full));
    track.sort_by(|left, right| left.0.total_cmp(&right.0));
    dedupe_times(&mut track);
    track
}

fn hold_view(
    track: &mut Vec<(f32, Rect)>,
    navigation: &NavigationModel,
    time: f32,
    module_id: &str,
    hold: f32,
) {
    if let Some(view) = navigation.view_for_module(module_id) {
        let shot = aspect_rect(view.bounds, 0.0);
        track.push((time, shot));
        track.push((time + hold, shot));
    }
}

fn display_screen(bounds: Rect) -> Rect {
    aspect_rect(
        Rect::new(
            bounds.x + 18.0,
            bounds.y + 20.0,
            128.0 * 2.42 + 72.0,
            96.0 * 2.42 + 72.0,
        ),
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

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::{build_topology, Machine};

    #[test]
    fn director_replaces_viewbox_animation_with_css_world_camera_only() {
        let topology = build_topology();
        let trace = Machine::run_match("director-test", 120);
        let source = format!(
            "<svg><svg class=\"animated\" id=\"camera\" width=\"864\" height=\"484\" viewBox=\"0 0 {} {}\"><rect width=\"100%\" height=\"100%\" fill=\"#07101a\"/><g id=\"content\"/><animate attributeName=\"viewBox\" values=\"0 0 1 1\"/></svg></svg>",
            topology.width, topology.height
        );
        let output = apply_camera(source, &topology, &trace, RenderConfig::default());
        assert!(!output.contains("attributeName=\"viewBox\""));
        assert!(output.contains("viewBox=\"0 0 864 484\""));
        assert!(output.contains("@keyframes leaderCamera"));
        assert!(output.contains("@keyframes leaderDetailLod"));
        assert!(output.contains("id=\"camera-world\""));
        assert!(output.contains("id=\"navigation-hierarchy\""));
        assert!(!output.contains("id=\"f3-datapath\""));
    }

    #[test]
    fn navigation_overlay_serializes_inspectable_detail_metadata() {
        let topology = build_topology();
        let navigation = build_navigation(&topology);
        let overlay = navigation_overlay(&navigation);
        assert!(overlay.contains("data-module=\"decode.microcode\""));
        assert!(overlay.contains("data-module=\"io.shields\""));
        assert!(overlay.contains("data-module=\"gpu.timing\""));
        assert!(overlay.contains("data-density=\"bit_exact\""));
        assert!(overlay.contains("data-parent=\"view-gpu\""));
    }

    #[test]
    fn physical_nodes_receive_navigation_membership_metadata() {
        let topology = build_topology();
        let navigation = build_navigation(&topology);
        let mut svg = String::from(
            "<svg><g id=\"node-microRom\"></g><g id=\"node-shieldAddr\"></g></svg>",
        );
        annotate_node_membership(&mut svg, &navigation);
        assert!(svg.contains("id=\"node-microRom\" data-detail-module=\"decode.microcode\""));
        assert!(svg.contains("data-subsystem=\"decode\""));
        assert!(svg.contains("id=\"node-shieldAddr\" data-detail-module=\"io.shields\""));
        assert!(svg.contains("data-subsystem=\"io\""));
    }

    #[test]
    fn closeup_matrix_is_much_larger_than_establishing_shot() {
        let topology = build_topology();
        let navigation = build_navigation(&topology);
        let full = navigation.view("view-machine").expect("machine view").bounds;
        let microcode = navigation
            .view_for_module("decode.microcode")
            .expect("microcode detail view")
            .bounds;
        assert!(
            view_matrix(aspect_rect(microcode, 0.0)).scale
                > view_matrix(aspect_rect(full, 0.0)).scale * 4.0
        );
    }

    #[test]
    fn director_has_real_nested_detail_views() {
        let topology = build_topology();
        let navigation = build_navigation(&topology);
        for id in [
            "decode.microcode",
            "alu.ripple",
            "bus.stack",
            "io.shields",
            "gpu.timing",
            "gpu.scanout",
        ] {
            let view = navigation.view_for_module(id).expect("detail view");
            assert_eq!(view.level, NavigationLevel::Detail);
            assert_eq!(view.density, DetailDensity::BitExact);
            assert!(view.parent_view.is_some());
        }
    }

    #[test]
    fn display_final_shot_is_less_zoomed_than_previous_crop() {
        let topology = build_topology();
        let display = topology.node("display").expect("display");
        let shot = display_screen(display.bounds);
        assert!(shot.w > 380.0);
        assert!(shot.h > 210.0);
    }
}
