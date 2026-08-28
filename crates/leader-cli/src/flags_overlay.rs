use leader_core::{FlagEvent, MatchTrace, Topology};
use leader_svg::RenderConfig;

#[must_use]
pub fn apply(
    mut svg: String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
) -> String {
    if trace.total_frames == 0 || trace.flag_events.is_empty() {
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
    let stride = (trace.flag_events.len() / 120).max(1);
    let total = config.total();
    let mut out = String::with_capacity(90_000);
    out.push_str("<g id=\"f3-native-flags\">\n");

    for event in trace.flag_events.iter().step_by(stride) {
        render_event(&mut out, topology, trace, config, total, *event);
    }

    out.push_str("</g>\n");
    out
}

fn render_event(
    out: &mut String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
    total: f32,
    event: FlagEvent,
) {
    let moment = trace_moment(event.frame, event.ordinal, trace, config) + 0.014;
    let k1 = norm(moment, total);
    let k2 = norm(moment + 0.020, total);
    let k3 = norm(moment + 0.130, total);
    out.push_str(&format!(
        "<g opacity=\"0\" data-flags-packed=\"{:01X}\" data-flag-z=\"{}\" data-flag-c=\"{}\" data-flag-l=\"{}\" data-flag-control=\"{}\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>",
        event.packed(),
        u8::from(event.zero),
        u8::from(event.carry),
        u8::from(event.less),
        event.control
    ));

    glow_node(out, topology, "ctrlFlagsLoad", "#ef7caf", true);
    glow_node(out, topology, "flagZ", "#67d9b3", event.zero);
    glow_node(out, topology, "flagC", "#ffe16a", event.carry);
    glow_node(out, topology, "flagN", "#ff9b71", event.less);
    out.push_str("</g>\n");
}

fn glow_node(out: &mut String, topology: &Topology, id: &str, color: &str, active: bool) {
    let Some(node) = topology.node(id) else {
        return;
    };
    let b = node.bounds;
    let opacity = if active { 0.26 } else { 0.06 };
    let stroke_opacity = if active { 1.0 } else { 0.25 };
    out.push_str(&format!(
        "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"7\" fill=\"{color}\" fill-opacity=\"{opacity:.2}\" stroke=\"{color}\" stroke-opacity=\"{stroke_opacity:.2}\" stroke-width=\"8\" filter=\"url(#glow)\"/>",
        b.x - 3.0,
        b.y - 3.0,
        b.w + 6.0,
        b.h + 6.0
    ));
}

fn trace_moment(frame: u32, ordinal: u16, trace: &MatchTrace, config: RenderConfig) -> f32 {
    config.game_start()
        + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
        + f32::from(ordinal.min(63)) * 0.0018
}

fn norm(value: f32, total: f32) -> f32 {
    (value / total).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::{build_topology, Machine};

    #[test]
    fn exact_flags_render_without_semantic_samples() {
        let topology = build_topology();
        let mut trace = Machine::run_match("f3-flags-overlay", 120);
        let config = RenderConfig::default();
        let baseline = render(&topology, &trace, config);

        assert!(!trace.flag_events.is_empty());
        assert!(baseline.contains("id=\"f3-native-flags\""));
        assert!(baseline.contains("data-flags-packed=\""));
        assert!(baseline.contains("data-flag-z=\""));
        assert!(baseline.contains("data-flag-c=\""));
        assert!(baseline.contains("data-flag-l=\""));

        trace.micro_samples.clear();
        assert_eq!(render(&topology, &trace, config), baseline);
    }
}
