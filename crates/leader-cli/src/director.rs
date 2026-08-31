use std::fmt::Write;

use leader_core::{
    build_navigation, CameraView, DetailDensity, MatchTrace, NavigationLevel, NavigationModel, Rect,
    Topology,
};
use leader_svg::RenderConfig;

const VIEW_W: f32 = 864.0;
const VIEW_H: f32 = 484.0;
const VIEW_ASPECT: f32 = VIEW_W / VIEW_H;

#[derive(Debug, Clone)]
struct CameraCue {
    time: f32,
    rect: Rect,
    view_id: String,
    detail_lod: bool,
}

#[must_use]
pub fn apply_camera(
    mut svg: String,
    topology: &Topology,
    _trace: &MatchTrace,
    config: RenderConfig,
) -> String {
    let navigation = build_navigation(topology);
    let track = camera_track(&navigation, topology, config);
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

    let css = camera_css(&navigation, &track, config.total());
    let overlay = navigation_overlay(&navigation, &track, config.total());
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

fn navigation_overlay(
    navigation: &NavigationModel,
    track: &[CameraCue],
    total: f32,
) -> String {
    let mut out = String::with_capacity(navigation.views.len() * 360 + track.len() * 180);
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

    let _ = write!(
        out,
        "<g id=\"navigation-scenes\" data-scene-count=\"{}\" display=\"none\" aria-hidden=\"true\">",
        track.len()
    );
    for cue in track {
        let _ = write!(
            out,
            "<g data-scene-time=\"{:.3}\" data-scene-progress=\"{:.6}\" data-scene-view=\"{}\" data-scene-detail=\"{}\" data-scene-x=\"{:.1}\" data-scene-y=\"{:.1}\" data-scene-w=\"{:.1}\" data-scene-h=\"{:.1}\"/>",
            cue.time,
            norm(cue.time, total),
            xml_escape(&cue.view_id),
            cue.detail_lod,
            cue.rect.x,
            cue.rect.y,
            cue.rect.w,
            cue.rect.h
        );
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

fn camera_css(navigation: &NavigationModel, track: &[CameraCue], total: f32) -> String {
    let mut camera_rules = String::with_capacity(track.len() * 80);
    let mut node_kind_rules = String::with_capacity(track.len() * 32);
    let mut node_title_rules = String::with_capacity(track.len() * 32);
    let mut wire_rules = String::with_capacity(track.len() * 32);
    let mut group_rules = String::with_capacity(track.len() * 32);
    let mut group_label_rules = String::with_capacity(track.len() * 32);

    for cue in track {
        let percent = norm(cue.time, total) * 100.0;
        let matrix = view_matrix(cue.rect);
        let machine = cue.view_id == navigation.default_view;
        let (node_kind_opacity, node_title_opacity, wire_opacity, group_opacity, group_label_opacity) =
            if cue.detail_lod {
                (1.0, 1.0, 0.62, 0.22, 0.28)
            } else if machine {
                (0.12, 0.56, 0.20, 0.72, 0.82)
            } else {
                (0.42, 0.84, 0.36, 0.48, 0.62)
            };

        camera_rules.push_str(&format!(
            "{percent:.6}%{{transform:matrix({:.7},0,0,{:.7},{:.3},{:.3})}}",
            matrix.scale, matrix.scale, matrix.tx, matrix.ty
        ));
        node_kind_rules.push_str(&format!(
            "{percent:.6}%{{opacity:{node_kind_opacity:.2}}}"
        ));
        node_title_rules.push_str(&format!(
            "{percent:.6}%{{opacity:{node_title_opacity:.2}}}"
        ));
        wire_rules.push_str(&format!("{percent:.6}%{{opacity:{wire_opacity:.2}}}"));
        group_rules.push_str(&format!("{percent:.6}%{{opacity:{group_opacity:.2}}}"));
        group_label_rules.push_str(&format!(
            "{percent:.6}%{{opacity:{group_label_opacity:.2}}}"
        ));
    }
    node_kind_rules.push_str("100%{opacity:.12}");
    node_title_rules.push_str("100%{opacity:.56}");
    wire_rules.push_str("100%{opacity:.20}");
    group_rules.push_str("100%{opacity:.72}");
    group_label_rules.push_str("100%{opacity:.82}");

    let mut focus_css = String::with_capacity(navigation.views.len() * track.len() * 28);
    for (index, view) in navigation
        .views
        .iter()
        .filter(|view| view.level != NavigationLevel::Machine)
        .enumerate()
    {
        let animation = format!("leaderNavFocus{index}");
        let mut rules = String::with_capacity(track.len() * 28);
        for cue in track {
            let percent = norm(cue.time, total) * 100.0;
            let opacity = focus_opacity(navigation, view, cue);
            rules.push_str(&format!("{percent:.6}%{{opacity:{opacity:.2}}}"));
        }
        rules.push_str("100%{opacity:.05}");
        focus_css.push_str(&format!(
            "@keyframes {animation}{{{rules}}}.nav-boundary[data-view=\"{}\"]{{animation:{animation} {total:.3}s linear infinite}}",
            xml_escape(&view.id)
        ));
    }

    format!(
        "<style>@keyframes leaderCamera{{{camera_rules}}}@keyframes leaderNodeKindLod{{{node_kind_rules}}}@keyframes leaderNodeTitleLod{{{node_title_rules}}}@keyframes leaderWireLod{{{wire_rules}}}@keyframes leaderGroupLod{{{group_rules}}}@keyframes leaderGroupLabelLod{{{group_label_rules}}}{focus_css}#camera-world{{transform-box:view-box;transform-origin:0 0;animation:leaderCamera {total:.3}s linear infinite}}.nav-boundary{{pointer-events:none;opacity:.05}}.nav-boundary rect{{fill:#08131d;fill-opacity:.08;vector-effect:non-scaling-stroke}}.nav-boundary text{{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;letter-spacing:2px;vector-effect:non-scaling-stroke}}.nav-subsystem rect{{stroke:#44677f;stroke-width:2;stroke-dasharray:12 10}}.nav-subsystem text{{fill:#5f7f95;font-size:13px;font-weight:700}}.nav-detail rect{{stroke:#80a7bd;stroke-width:1.5;stroke-dasharray:7 7}}.nav-detail text{{fill:#91b5c7;font-size:11px;font-weight:800}}.node-kind{{animation:leaderNodeKindLod {total:.3}s linear infinite}}.node-title{{animation:leaderNodeTitleLod {total:.3}s linear infinite}}.wire{{animation:leaderWireLod {total:.3}s linear infinite}}.group{{animation:leaderGroupLod {total:.3}s linear infinite}}.group-label{{animation:leaderGroupLabelLod {total:.3}s linear infinite}}</style>"
    )
}

fn focus_opacity(navigation: &NavigationModel, view: &CameraView, cue: &CameraCue) -> f32 {
    let exact = cue.view_id == view.id;
    let in_lineage = navigation
        .lineage_for_view(&cue.view_id)
        .iter()
        .any(|candidate| candidate.id == view.id);

    if cue.detail_lod {
        if exact {
            1.0
        } else if in_lineage {
            0.34
        } else {
            0.035
        }
    } else if view.level == NavigationLevel::Subsystem {
        if exact {
            0.76
        } else if in_lineage {
            0.20
        } else {
            0.05
        }
    } else {
        0.025
    }
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
    config: RenderConfig,
) -> Vec<CameraCue> {
    let total = config.total();
    let full_view = navigation.view(&navigation.default_view);
    let full = full_view
        .map(|view| aspect_rect(view.bounds, 0.0))
        .unwrap_or_else(|| aspect_rect(Rect::new(0.0, 0.0, topology.width, topology.height), 0.0));
    let machine_view = full_view
        .map(|view| view.id.clone())
        .unwrap_or_else(|| navigation.default_view.clone());
    let mut track = vec![CameraCue {
        time: 0.0,
        rect: full,
        view_id: machine_view.clone(),
        detail_lod: false,
    }];

    let mut groups = topology.groups.clone();
    groups.sort_by_key(|group| group.assembly_rank);
    let span = config.assembly_seconds / groups.len().max(1) as f32;
    for (index, group) in groups.iter().enumerate() {
        let start = index as f32 * span;
        if index == 0 {
            push_cue(&mut track, 0.55, full, &machine_view, false);
        }
        let Some(view) = navigation.view_for_module(&group.id) else {
            continue;
        };
        let shot = aspect_rect(view.bounds, 0.0);
        push_cue(
            &mut track,
            start + 0.70,
            aspect_rect(shot, 136.0),
            &view.id,
            false,
        );
        push_cue(&mut track, start + span * 0.34, shot, &view.id, false);
        push_cue(&mut track, start + span * 0.78, shot, &view.id, false);
        push_cue(
            &mut track,
            start + span * 0.96,
            aspect_rect(shot, 86.0),
            &view.id,
            false,
        );
    }
    push_cue(
        &mut track,
        config.assembly_seconds,
        full,
        &machine_view,
        false,
    );

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
    push_cue(
        &mut track,
        config.game_start(),
        full,
        &machine_view,
        false,
    );

    let game = config.game_start();
    push_cue(&mut track, game + 0.25, full, &machine_view, false);
    hold_view(&mut track, navigation, game + 0.72, "io.input_irq", 0.46);
    hold_view(&mut track, navigation, game + 1.30, "io.shift_register", 0.56);
    hold_view(&mut track, navigation, game + 1.98, "io.formation", 0.58);
    hold_view(&mut track, navigation, game + 2.68, "io.enemy_shots", 0.66);
    hold_view(&mut track, navigation, game + 3.46, "io.shields", 0.72);
    hold_view(&mut track, navigation, game + 4.30, "gpu.dma", 0.48);
    hold_view(&mut track, navigation, game + 4.90, "gpu.timing", 0.56);
    hold_view(&mut track, navigation, game + 5.58, "gpu.scanout", 0.64);

    let global_observe_end = (game + 6.40).min(config.game_end() - 8.0);
    push_cue(&mut track, global_observe_end, full, &machine_view, false);
    if let Some(display) = topology.node("display") {
        let display_view = navigation
            .view_for_module("gpu.scanout")
            .map(|view| view.id.as_str())
            .unwrap_or(machine_view.as_str());
        push_cue(
            &mut track,
            global_observe_end + 0.40,
            focus(display.bounds, 210.0),
            display_view,
            false,
        );
        push_cue(
            &mut track,
            global_observe_end + 1.10,
            focus(display.bounds, 92.0),
            display_view,
            false,
        );
        push_cue(
            &mut track,
            global_observe_end + 1.85,
            display_screen(display.bounds),
            display_view,
            false,
        );
        push_cue(
            &mut track,
            config.game_end() + config.outro_seconds - 0.20,
            display_screen(display.bounds),
            display_view,
            false,
        );
    }
    push_cue(&mut track, total - 0.05, full, &machine_view, false);
    track.sort_by(|left, right| left.time.total_cmp(&right.time));
    dedupe_times(&mut track);
    track
}

fn push_cue(
    track: &mut Vec<CameraCue>,
    time: f32,
    rect: Rect,
    view_id: &str,
    detail_lod: bool,
) {
    track.push(CameraCue {
        time,
        rect,
        view_id: view_id.to_owned(),
        detail_lod,
    });
}

fn hold_view(
    track: &mut Vec<CameraCue>,
    navigation: &NavigationModel,
    time: f32,
    module_id: &str,
    hold: f32,
) {
    if let Some(view) = navigation.view_for_module(module_id) {
        let shot = aspect_rect(view.bounds, 0.0);
        let detail_lod = view.level == NavigationLevel::Detail;
        push_cue(track, time, shot, &view.id, detail_lod);
        push_cue(track, time + hold, shot, &view.id, detail_lod);
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

fn dedupe_times(track: &mut [CameraCue]) {
    let mut last = -1.0_f32;
    for cue in track {
        if cue.time <= last {
            cue.time = last + 0.001;
        }
        last = cue.time;
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
        assert!(output.contains("@keyframes leaderNodeKindLod"));
        assert!(output.contains("@keyframes leaderWireLod"));
        assert!(output.contains("@keyframes leaderNodeTitleLod"));
        assert!(output.contains("id=\"camera-world\""));
        assert!(output.contains("id=\"navigation-hierarchy\""));
        assert!(output.contains("id=\"navigation-scenes\""));
        assert!(!output.contains("id=\"f3-datapath\""));
    }

    #[test]
    fn navigation_overlay_serializes_inspectable_detail_metadata_and_scene_cues() {
        let topology = build_topology();
        let navigation = build_navigation(&topology);
        let config = RenderConfig::default();
        let track = camera_track(&navigation, &topology, config);
        let overlay = navigation_overlay(&navigation, &track, config.total());
        assert!(overlay.contains("data-module=\"decode.microcode\""));
        assert!(overlay.contains("data-module=\"io.shields\""));
        assert!(overlay.contains("data-module=\"gpu.timing\""));
        assert!(overlay.contains("data-density=\"bit_exact\""));
        assert!(overlay.contains("data-parent=\"view-gpu\""));
        assert!(overlay.contains("data-scene-view=\"view-decode.microcode\""));
        assert!(overlay.contains("data-scene-view=\"view-io.shields\""));
        assert!(overlay.contains("data-scene-detail=\"true\""));
    }

    #[test]
    fn scene_cues_drive_contextual_focus() {
        let topology = build_topology();
        let navigation = build_navigation(&topology);
        let config = RenderConfig::default();
        let track = camera_track(&navigation, &topology, config);
        let microcode = navigation
            .view_for_module("decode.microcode")
            .expect("microcode view");
        let cue = track
            .iter()
            .find(|cue| cue.view_id == microcode.id && cue.detail_lod)
            .expect("microcode detail cue");
        assert_eq!(focus_opacity(&navigation, microcode, cue), 1.0);
        let alu = navigation.view_for_module("alu.ripple").expect("alu view");
        assert!(focus_opacity(&navigation, alu, cue) < 0.1);
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
