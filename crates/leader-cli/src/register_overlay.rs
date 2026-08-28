use leader_core::{bit8, MatchTrace, Topology};
use leader_svg::RenderConfig;

#[must_use]
pub fn apply(
    mut svg: String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
) -> String {
    if trace.total_frames == 0 || trace.register_writes.is_empty() {
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
    let stride = (trace.register_writes.len() / 120).max(1);
    let total = config.total();
    let mut out = String::with_capacity(150_000);
    out.push_str("<g id=\"f3-native-registers\">\n");

    for event in trace.register_writes.iter().step_by(stride) {
        let moment = trace_moment(event.frame, event.ordinal, trace, config) + 0.018;
        let k1 = norm(moment, total);
        let k2 = norm(moment + 0.022, total);
        let k3 = norm(moment + 0.135, total);
        out.push_str(&format!(
            "<g opacity=\"0\" data-reg=\"{}\" data-reg-before=\"{:02X}\" data-reg-after=\"{:02X}\" data-reg-control=\"{}\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>",
            event.reg.name(),
            event.before,
            event.after,
            event.control
        ));

        glow_node(&mut out, topology, "regSelectLatch", "#67d9b3");
        glow_node(&mut out, topology, "writeDec", "#ef7caf");
        glow_node(&mut out, topology, "writeBus", "#4bc8f3");
        for bit in 0..8 {
            let id = format!("reg{}{bit}", event.reg.name());
            let before = bit8(event.before, bit);
            let after = bit8(event.after, bit);
            if after {
                glow_node(&mut out, topology, &id, "#67d9b3");
            } else if before != after {
                glow_node(&mut out, topology, &id, "#ff9b71");
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
        "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"7\" fill=\"{color}\" fill-opacity=\".18\" stroke=\"{color}\" stroke-width=\"8\" filter=\"url(#glow)\"/>",
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
    fn register_overlay_exposes_exact_native_writeback() {
        let topology = build_topology();
        let mut trace = Machine::run_match("f3-register-overlay", 120);
        let config = RenderConfig::default();
        let baseline = render(&topology, &trace, config);

        assert!(baseline.contains("id=\"f3-native-registers\""));
        assert!(baseline.contains("data-reg=\""));
        assert!(baseline.contains("data-reg-before=\""));
        assert!(baseline.contains("data-reg-after=\""));
        assert!(baseline.contains("data-reg-control=\""));

        trace.micro_samples.clear();
        assert_eq!(render(&topology, &trace, config), baseline);
    }
}
