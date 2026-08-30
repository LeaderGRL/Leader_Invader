use leader_core::{build_navigation, MatchTrace, NavigationModel, Rect, Topology};
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

    let css = camera_css(topology, trace, config);
    svg.insert_str(world_insert, &format!("{css}<g id=\"camera-world\">"));

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
    _trace: &MatchTrace,
    config: RenderConfig,
) -> Vec<(f32, Rect)> {
    let total = config.total();
    let navigation = build_navigation(topology);
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
    hold_view(&mut track, &navigation, boot + 0.20, "clk", 0.72);
    hold_view(&mut track, &navigation, boot + 1.20, "pc", 0.78);
    hold_view(&mut track, &navigation, boot + 2.25, "romsys", 0.82);
    hold_view(&mut track, &navigation, boot + 3.35, "decode", 0.46);
    hold_view(&mut track, &navigation, boot + 3.88, "decode.microcode", 0.38);
    hold_view(&mut track, &navigation, boot + 4.45, "regs", 0.48);
    hold_view(&mut track, &navigation, boot + 4.98, "alu", 0.36);
    hold_view(&mut track, &navigation, boot + 5.40, "alu.ripple", 0.48);
    hold_view(&mut track, &navigation, boot + 6.05, "bus", 0.32);
    hold_view(&mut track, &navigation, boot + 6.43, "bus.stack", 0.38);
    hold_view(&mut track, &navigation, boot + 6.90, "gpu", 0.28);
    hold_view(&mut track, &navigation, boot + 7.24, "gpu.scanout", 0.66);
    track.push((config.game_start(), full));

    let game = config.game_start();
    let global_observe_end = (game + 7.0).min(config.game_end() - 8.0);
    track.push((game + 0.35, full));
    track.push((global_observe_end, full));
    if let Some(display) = topology.node("display") {
        track.push((global_observe_end + 0.55, focus(display.bounds, 210.0)));
        track.push((global_observe_end + 1.35, focus(display.bounds, 92.0)));
        track.push((global_observe_end + 2.20, display_screen(display.bounds)));
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

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::{build_topology, Machine, NavigationLevel};

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
        assert!(output.contains("id=\"camera-world\""));
        assert!(!output.contains("id=\"f3-datapath\""));
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
        assert!(view_matrix(aspect_rect(microcode, 0.0)).scale > view_matrix(aspect_rect(full, 0.0)).scale * 4.0);
    }

    #[test]
    fn director_has_real_nested_detail_views() {
        let topology = build_topology();
        let navigation = build_navigation(&topology);
        for id in ["decode.microcode", "alu.ripple", "bus.stack", "gpu.scanout"] {
            let view = navigation.view_for_module(id).expect("detail view");
            assert_eq!(view.level, NavigationLevel::Detail);
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
