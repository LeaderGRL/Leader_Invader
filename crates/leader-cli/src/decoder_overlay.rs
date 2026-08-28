use leader_core::{MatchTrace, MicroCycleKind, Topology};
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
    let decode_count = trace
        .micro_cycles
        .iter()
        .filter(|event| event.kind == MicroCycleKind::DecodeLatch)
        .count();
    if decode_count == 0 {
        return String::new();
    }

    let stride = (decode_count / 96).max(1);
    let mut seen = 0usize;
    let mut out = String::with_capacity(180_000);
    out.push_str("<g id=\"f3-native-decoder\">\n");

    for event in trace
        .micro_cycles
        .iter()
        .filter(|event| event.kind == MicroCycleKind::DecodeLatch)
    {
        let take = seen % stride == 0;
        seen += 1;
        if !take {
            continue;
        }

        let opcode = event.ir;
        let hi = opcode >> 4;
        let lo = opcode & 0x0f;
        let moment = trace_moment(event.frame, event.ordinal, trace, config) + 0.010;
        let total = config.total();
        let k1 = norm(moment, total);
        let k2 = norm(moment + 0.026, total);
        let k3 = norm(moment + 0.145, total);

        out.push_str(&format!(
            "<g opacity=\"0\" data-opcode=\"{opcode:02X}\" data-decode-hi=\"{hi:X}\" data-decode-lo=\"{lo:X}\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>"
        ));

        for bit in 0..8 {
            if opcode & (1 << bit) != 0 {
                glow_node(&mut out, topology, &format!("irBit{bit}"), "#ef7caf");
            }
        }
        glow_node(&mut out, topology, "opHi", "#4bc8f3");
        glow_node(&mut out, topology, "opLo", "#4bc8f3");
        glow_node(&mut out, topology, "decA", "#f7ce62");
        glow_node(&mut out, topology, "decB", "#f7ce62");
        glow_node(&mut out, topology, "microAddr", "#67d9b3");

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
        "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"10\" fill=\"none\" stroke=\"{color}\" stroke-width=\"10\" filter=\"url(#glow)\"/>",
        b.x - 4.0,
        b.y - 4.0,
        b.w + 8.0,
        b.h + 8.0
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
    fn decoder_overlay_is_native_microcycle_driven() {
        let topology = build_topology();
        let mut trace = Machine::run_match("f3-native-decoder", 5000);
        let config = RenderConfig::default();
        let baseline = render(&topology, &trace, config);
        assert!(baseline.contains("data-opcode=\""));
        assert!(baseline.contains("data-decode-hi=\""));
        assert!(baseline.contains("data-decode-lo=\""));

        trace.micro_samples.clear();
        assert_eq!(render(&topology, &trace, config), baseline);
    }
}
