use leader_core::{bit16, bit8, MatchTrace, MicroPhase, Topology};
use leader_svg::RenderConfig;

#[must_use]
pub fn apply(
    mut svg: String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
) -> String {
    if trace.total_frames == 0 || trace.micro_cycles.is_empty() {
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
    let stride = (trace.micro_cycles.len() / 150).max(1);
    let total = config.total();
    let mut out = String::with_capacity(260_000);
    out.push_str("<g id=\"f3-native-microcycles\">\n");

    for event in trace.micro_cycles.iter().step_by(stride) {
        let moment = trace_moment(event.frame, event.ordinal, trace, config) + 0.010;
        let k1 = norm(moment, total);
        let k2 = norm(moment + 0.018, total);
        let k3 = norm(moment + 0.105, total);
        out.push_str(&format!(
            "<g opacity=\"0\" data-micro-phase=\"{}\" data-micro-kind=\"{}\" data-micro-pc=\"{:04X}\" data-micro-mar=\"{:04X}\" data-micro-mdr=\"{:02X}\" data-micro-ir=\"{:02X}\" data-micro-control=\"{}\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>",
            event.phase.as_str(),
            event.kind.as_str(),
            event.pc,
            event.mar,
            event.mdr,
            event.ir,
            event.control
        ));

        let (phase_node, phase_color) = match event.phase {
            MicroPhase::T0 => ("phase0", "#67d9b3"),
            MicroPhase::T1 => ("phase1", "#4bc8f3"),
            MicroPhase::T2 => ("phase2", "#ef7caf"),
        };
        glow_node(&mut out, topology, phase_node, phase_color);

        for bit in 0..16 {
            if bit16(event.pc, bit) {
                glow_node(&mut out, topology, &format!("pcBit{bit}"), "#67d9b3");
            }
            if bit16(event.mar, bit) {
                glow_node(&mut out, topology, &format!("marBit{bit}"), "#f2ae4f");
            }
        }
        for bit in 0..8 {
            if bit8(event.mdr, bit) {
                glow_node(&mut out, topology, &format!("mdrBit{bit}"), "#4bc8f3");
            }
            if bit8(event.ir, bit) {
                glow_node(&mut out, topology, &format!("irBit{bit}"), "#ef7caf");
            }
        }

        out.push_str("</g>\n");
    }

    out.push_str("</g>\n");
    out
}

fn glow_node(out: &mut String, topology: &Topology, id: &str, color: &str) {
    let Some(node) = topology.node(id) else {
        return;
    };
    let b = node.bounds;
    out.push_str(&format!(
        "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"7\" fill=\"{color}\" fill-opacity=\".15\" stroke=\"{color}\" stroke-width=\"7\" filter=\"url(#glow)\"/>",
        b.x - 2.0,
        b.y - 2.0,
        b.w + 4.0,
        b.h + 4.0
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
    fn microcycle_overlay_exposes_exact_native_latch_state() {
        let topology = build_topology();
        let mut trace = Machine::run_match("f3-microcycle-overlay", 120);
        let config = RenderConfig::default();
        let baseline = render(&topology, &trace, config);

        assert!(baseline.contains("id=\"f3-native-microcycles\""));
        assert!(baseline.contains("data-micro-phase=\"t0\""));
        assert!(baseline.contains("data-micro-phase=\"t1\""));
        assert!(baseline.contains("data-micro-phase=\"t2\""));
        assert!(baseline.contains("data-micro-pc=\""));
        assert!(baseline.contains("data-micro-mar=\""));
        assert!(baseline.contains("data-micro-mdr=\""));
        assert!(baseline.contains("data-micro-ir=\""));

        trace.micro_samples.clear();
        assert_eq!(render(&topology, &trace, config), baseline);
    }
}
