use leader_core::{BusTransactionKind, MatchTrace, MicroPhase, Topology};
use leader_svg::RenderConfig;

#[must_use]
pub fn apply(mut svg: String, topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    if trace.total_frames == 0 {
        return svg;
    }
    let overlay = render(topology, trace, config);
    let Some(svg_close) = svg.rfind("</svg>") else { return svg; };
    let Some(world_close) = svg[..svg_close].rfind("</g>") else { return svg; };
    svg.insert_str(world_close, &overlay);
    svg
}

fn render(topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    let cpu_stride = (trace.micro_cycles.len() / 260).max(1);
    let peripheral_count = trace
        .bus_transactions
        .iter()
        .filter(|event| matches!(event.kind, BusTransactionKind::Dma | BusTransactionKind::Scanout))
        .count();
    let peripheral_stride = (peripheral_count / 80).max(1);
    let total = config.total();
    let mut out = String::with_capacity(180_000);
    out.push_str("<g id=\"f3-timing\">\n");

    for event in trace.micro_cycles.iter().step_by(cpu_stride) {
        let moment = trace_moment(event.frame, event.ordinal, trace, config);
        let (node, color) = match event.phase {
            MicroPhase::T0 => ("phase0", "#67d9b3"),
            MicroPhase::T1 => ("phase1", "#4bc8f3"),
            MicroPhase::T2 => ("phase2", "#ef7caf"),
        };
        phase_pulse(&mut out, topology, node, moment, total, color);
    }

    let mut peripheral_index = 0usize;
    for event in &trace.bus_transactions {
        let (phase_a, phase_b) = match event.kind {
            BusTransactionKind::Dma => (Some("phase0"), Some("phase1")),
            BusTransactionKind::Scanout => (Some("phase1"), None),
            _ => continue,
        };
        let take = peripheral_index % peripheral_stride == 0;
        peripheral_index += 1;
        if !take {
            continue;
        }
        let moment = trace_moment(event.frame, event.ordinal, trace, config);
        if let Some(node) = phase_a {
            phase_pulse(&mut out, topology, node, moment, total, "#72d4e7");
        }
        if let Some(node) = phase_b {
            phase_pulse(&mut out, topology, node, moment + 0.024, total, "#72d4e7");
        }
    }

    out.push_str("</g>\n");
    out
}

fn phase_pulse(out: &mut String, topology: &Topology, id: &str, moment: f32, total: f32, color: &str) {
    let Some(node) = topology.node(id) else { return; };
    let b = node.bounds;
    let k1 = norm(moment, total);
    let k2 = norm(moment + 0.018, total);
    let k3 = norm(moment + 0.09, total);
    out.push_str(&format!(
        "<g opacity=\"0\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/><rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"8\" fill=\"{}\" fill-opacity=\".24\" stroke=\"{}\" stroke-width=\"9\" filter=\"url(#glow)\"/></g>\n",
        b.x - 3.0, b.y - 3.0, b.w + 6.0, b.h + 6.0, color, color
    ));
}

fn trace_moment(frame: u32, ordinal: u16, trace: &MatchTrace, config: RenderConfig) -> f32 {
    config.game_start()
        + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
        + f32::from(ordinal.min(31)) * 0.0025
}

fn norm(value: f32, total: f32) -> f32 { (value / total).clamp(0.0, 1.0) }

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::{build_topology, Machine};

    #[test]
    fn timing_overlay_is_driven_by_native_cpu_t_states() {
        let topology = build_topology();
        let trace = Machine::run_match("timing-overlay", 120);
        assert!(trace.micro_cycles.iter().any(|event| event.phase == MicroPhase::T0));
        assert!(trace.micro_cycles.iter().any(|event| event.phase == MicroPhase::T1));
        assert!(trace.micro_cycles.iter().any(|event| event.phase == MicroPhase::T2));
        let rendered = render(&topology, &trace, RenderConfig::default());
        assert!(rendered.contains("id=\"f3-timing\""));
        assert!(rendered.len() > 500);
    }

    #[test]
    fn timing_overlay_does_not_depend_on_semantic_samples() {
        let topology = build_topology();
        let mut trace = Machine::run_match("timing-overlay-native-only", 120);
        let config = RenderConfig::default();
        let baseline = render(&topology, &trace, config);
        trace.micro_samples.clear();
        assert_eq!(render(&topology, &trace, config), baseline);
    }
}
