use leader_core::{bit8, MatchTrace, Topology};
use leader_svg::RenderConfig;

#[must_use]
pub fn apply(
    mut svg: String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
) -> String {
    if trace.total_frames == 0 || trace.alu_events.is_empty() {
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
    let stride = (trace.alu_events.len() / 110).max(1);
    let total = config.total();
    let mut out = String::with_capacity(220_000);
    out.push_str("<g id=\"f3-native-alu\">\n");

    for event in trace.alu_events.iter().step_by(stride) {
        let alu = event.trace;
        let moment = trace_moment(event.frame, event.ordinal, trace, config) + 0.014;
        let k1 = norm(moment, total);
        let k2 = norm(moment + 0.024, total);
        let k3 = norm(moment + 0.145, total);
        out.push_str(&format!(
            "<g opacity=\"0\" data-alu-op=\"{}\" data-alu-lhs=\"{:02X}\" data-alu-rhs=\"{:02X}\" data-alu-rhs-effective=\"{:02X}\" data-alu-result=\"{:02X}\" data-alu-carry-chain=\"{:03X}\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>",
            alu.op.as_str(),
            alu.lhs,
            alu.rhs,
            alu.rhs_effective,
            alu.result,
            alu.carry_chain
        ));

        glow_node(&mut out, topology, "readMuxA", "#4bc8f3");
        glow_node(&mut out, topology, "readMuxB", "#4bc8f3");
        glow_node(&mut out, topology, "aluSel", "#f7ce62");

        for bit in 0..8 {
            let a = bit8(alu.lhs, bit);
            let b = bit8(alu.rhs_effective, bit);
            let carry_in = alu.carry_in(bit);
            let xor_ab = a ^ b;
            let sum = xor_ab ^ carry_in;
            let generate = a & b;
            let propagate = xor_ab & carry_in;

            if xor_ab {
                glow_node(&mut out, topology, &format!("xorA{bit}"), "#f7ce62");
            }
            if sum {
                glow_node(&mut out, topology, &format!("xorB{bit}"), "#f7ce62");
            }
            if generate {
                glow_node(&mut out, topology, &format!("andA{bit}"), "#ff9b71");
            }
            if propagate {
                glow_node(&mut out, topology, &format!("andB{bit}"), "#ff9b71");
            }
            if alu.carry_out(bit) {
                glow_node(&mut out, topology, &format!("orC{bit}"), "#ffe16a");
            }
            if bit8(alu.result, bit) {
                glow_node(&mut out, topology, &format!("muxR{bit}"), "#67d9b3");
            }
        }

        if alu.result == 0 {
            glow_node(&mut out, topology, "flagZ", "#ef7caf");
        }
        if alu.final_carry() {
            glow_node(&mut out, topology, "flagC", "#ef7caf");
        }
        if matches!(alu.op.as_str(), "sub" | "compare") && !alu.final_carry() {
            glow_node(&mut out, topology, "flagN", "#ef7caf");
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
        "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"8\" fill=\"{color}\" fill-opacity=\".18\" stroke=\"{color}\" stroke-width=\"9\" filter=\"url(#glow)\"/>",
        b.x - 3.0,
        b.y - 3.0,
        b.w + 6.0,
        b.h + 6.0
    ));
}

fn trace_moment(frame: u32, ordinal: u16, trace: &MatchTrace, config: RenderConfig) -> f32 {
    config.game_start()
        + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
        + f32::from(ordinal.min(31)) * 0.0025
}

fn norm(value: f32, total: f32) -> f32 {
    (value / total).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::{build_topology, Machine};

    #[test]
    fn alu_overlay_exposes_exact_native_operands_result_and_carry() {
        let topology = build_topology();
        let mut trace = Machine::run_match("f3-alu-overlay", 120);
        let config = RenderConfig::default();
        let baseline = render(&topology, &trace, config);

        assert!(baseline.contains("id=\"f3-native-alu\""));
        assert!(baseline.contains("data-alu-op=\""));
        assert!(baseline.contains("data-alu-lhs=\""));
        assert!(baseline.contains("data-alu-rhs=\""));
        assert!(baseline.contains("data-alu-result=\""));
        assert!(baseline.contains("data-alu-carry-chain=\""));

        trace.micro_samples.clear();
        assert_eq!(render(&topology, &trace, config), baseline);
    }
}
