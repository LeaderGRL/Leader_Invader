use leader_core::{MatchTrace, Topology, INTERNAL_CONTROL_NODES};
use leader_svg::RenderConfig;

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
    let stride = (trace.micro_addresses.len() / 140).max(1);
    let total = config.total();
    let mut out = String::with_capacity(160_000);
    out.push_str("<g id=\"f3-internal-control-bank\">\n");

    for event in trace.micro_addresses.iter().step_by(stride) {
        let internal = (event.control_bits >> 8) & 0xffff;
        if internal == 0 {
            continue;
        }
        let moment = trace_moment(event.frame, event.ordinal, trace, config) + 0.012;
        let k1 = norm(moment, total);
        let k2 = norm(moment + 0.024, total);
        let k3 = norm(moment + 0.135, total);
        out.push_str(&format!(
            "<g opacity=\"0\" data-uinternal=\"{internal:04X}\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>"
        ));

        for (bit, (id, _, _)) in INTERNAL_CONTROL_NODES.iter().enumerate() {
            if internal & (1_u32 << bit) != 0 {
                glow_node(&mut out, topology, id, control_color(bit));
            }
        }
        out.push_str("</g>\n");
    }

    out.push_str("</g>\n");
    out
}

fn control_color(bit: usize) -> &'static str {
    match bit {
        0..=3 => "#4bc8f3",
        4..=7 => "#f7ce62",
        8..=11 => "#e8e677",
        _ => "#67d9b3",
    }
}

fn glow_node(out: &mut String, topology: &Topology, id: &str, color: &str) {
    let Some(node) = topology.node(id) else {
        return;
    };
    let b = node.bounds;
    out.push_str(&format!(
        "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"5\" fill=\"{color}\" fill-opacity=\".28\" stroke=\"{color}\" stroke-width=\"7\" filter=\"url(#glow)\"/>",
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
    fn internal_control_bank_is_native_and_exposes_real_bits() {
        let topology = build_topology();
        let mut trace = Machine::run_match("f3-control-bank", 120);
        let config = RenderConfig::default();
        let baseline = render(&topology, &trace, config);
        assert!(baseline.contains("id=\"f3-internal-control-bank\""));
        assert!(baseline.contains("data-uinternal=\""));

        for (id, _, _) in INTERNAL_CONTROL_NODES {
            assert!(topology.node(id).is_some(), "missing {id}");
        }

        trace.micro_samples.clear();
        assert_eq!(render(&topology, &trace, config), baseline);
    }
}
