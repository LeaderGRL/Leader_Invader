use leader_core::{ControlLatchEvent, ControlLatchKind, MatchTrace, Topology};
use leader_svg::RenderConfig;

#[must_use]
pub fn apply(
    mut svg: String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
) -> String {
    if trace.total_frames == 0 || trace.control_latch_events.is_empty() {
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
    let stride = (trace.control_latch_events.len() / 160).max(1);
    let total = config.total();
    let mut out = String::with_capacity(110_000);
    out.push_str("<g id=\"f3-control-state-latches\">\n");

    for event in trace.control_latch_events.iter().step_by(stride) {
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
    event: ControlLatchEvent,
) {
    let (node, control_node, color) = latch_visual(event.kind);
    let moment = trace_moment(event.frame, event.ordinal, trace, config) + 0.016;
    let k1 = norm(moment, total);
    let k2 = norm(moment + 0.020, total);
    let k3 = norm(moment + 0.130, total);
    out.push_str(&format!(
        "<g opacity=\"0\" data-control-state=\"{}\" data-control-value=\"{:04X}\" data-control-valid=\"{}\" data-control-owner=\"{}\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>",
        event.kind.as_str(),
        event.value,
        u8::from(event.valid),
        event.control
    ));
    glow_node(out, topology, control_node, "#ef7caf", true);
    glow_node(out, topology, node, color, event.valid || event.kind != ControlLatchKind::PcSelect);
    out.push_str("</g>\n");
}

const fn latch_visual(kind: ControlLatchKind) -> (&'static str, &'static str, &'static str) {
    match kind {
        ControlLatchKind::AddressLo => ("addrLoLatch", "ctrlAddrLo", "#4bc8f3"),
        ControlLatchKind::AddressHi => ("addrHiLatch", "ctrlAddrHi", "#4bc8f3"),
        ControlLatchKind::Condition => ("conditionLatch", "ctrlCondition", "#ef7caf"),
        ControlLatchKind::PcSelect => ("pcSelectLatch", "ctrlPcSelect", "#e8e677"),
        ControlLatchKind::RegSelect => ("regSelectLatch", "ctrlRegSelect", "#67d9b3"),
    }
}

fn glow_node(out: &mut String, topology: &Topology, id: &str, color: &str, active: bool) {
    let Some(node) = topology.node(id) else {
        return;
    };
    let b = node.bounds;
    let opacity = if active { 0.24 } else { 0.05 };
    let stroke_opacity = if active { 1.0 } else { 0.22 };
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
    fn control_state_overlay_uses_exact_latch_stream_only() {
        let topology = build_topology();
        let mut trace = Machine::run_match("f3-control-state-values", 120);
        let config = RenderConfig::default();
        let baseline = render(&topology, &trace, config);

        assert!(!trace.control_latch_events.is_empty());
        assert!(baseline.contains("id=\"f3-control-state-latches\""));
        assert!(baseline.contains("data-control-state=\""));
        assert!(baseline.contains("data-control-value=\""));
        assert!(baseline.contains("data-control-valid=\""));
        assert!(baseline.contains("data-control-owner=\""));

        for kind in [
            ControlLatchKind::AddressLo,
            ControlLatchKind::AddressHi,
            ControlLatchKind::Condition,
            ControlLatchKind::PcSelect,
            ControlLatchKind::RegSelect,
        ] {
            assert!(trace.control_latch_events.iter().any(|event| event.kind == kind));
        }
        assert!(trace.control_latch_events.iter().any(|event| {
            event.kind == ControlLatchKind::PcSelect && !event.valid
        }));

        trace.micro_samples.clear();
        trace.micro_addresses.clear();
        assert_eq!(render(&topology, &trace, config), baseline);
    }
}
