use leader_core::{microcode::internal, MatchTrace, Topology};
use leader_svg::RenderConfig;

const LATCHES: [(u16, &str, &str); 5] = [
    (internal::ADDR_LO_LOAD, "addrLoLatch", "#4bc8f3"),
    (internal::ADDR_HI_LOAD, "addrHiLatch", "#4bc8f3"),
    (internal::CONDITION_LOAD, "conditionLatch", "#ef7caf"),
    (internal::PC_SELECT, "pcSelectLatch", "#e8e677"),
    (internal::REG_SELECT, "regSelectLatch", "#67d9b3"),
];

#[must_use]
pub fn apply(
    mut svg: String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
) -> String {
    if trace.total_frames == 0 || trace.micro_addresses.is_empty() {
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
    let stride = (trace.micro_addresses.len() / 130).max(1);
    let total = config.total();
    let mut out = String::with_capacity(100_000);
    out.push_str("<g id=\"f3-control-state-latches\">\n");

    for event in trace.micro_addresses.iter().step_by(stride) {
        let internal_bits = ((event.control_bits >> 8) & 0xffff) as u16;
        if !LATCHES.iter().any(|(signal, _, _)| internal_bits & *signal != 0) {
            continue;
        }

        let moment = trace_moment(event.frame, event.ordinal, trace, config) + 0.016;
        let k1 = norm(moment, total);
        let k2 = norm(moment + 0.020, total);
        let k3 = norm(moment + 0.120, total);
        out.push_str(&format!(
            "<g opacity=\"0\" data-control-state=\"{internal_bits:04X}\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>"
        ));

        for (signal, id, color) in LATCHES {
            if internal_bits & signal != 0 {
                glow_node(&mut out, topology, id, color);
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
        "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"7\" fill=\"{color}\" fill-opacity=\".22\" stroke=\"{color}\" stroke-width=\"8\" filter=\"url(#glow)\"/>",
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
    fn control_state_latches_are_driven_only_by_native_microcode() {
        let topology = build_topology();
        let mut trace = Machine::run_match("f3-control-state", 120);
        let config = RenderConfig::default();
        let baseline = render(&topology, &trace, config);

        assert!(baseline.contains("id=\"f3-control-state-latches\""));
        assert!(baseline.contains("data-control-state=\""));
        for (_, id, _) in LATCHES {
            assert!(topology.node(id).is_some(), "missing {id}");
        }

        trace.micro_samples.clear();
        assert_eq!(render(&topology, &trace, config), baseline);
    }
}
