use leader_core::{bit16, derive_pc_datapath, MatchTrace, PcDatapathKind, PcSource, Topology};
use leader_svg::RenderConfig;

#[must_use]
pub fn apply(mut svg: String, topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    if trace.total_frames == 0 {
        return svg;
    }

    let overlay = render(topology, trace, config);
    let Some(svg_close) = svg.rfind("</svg>") else {
        return svg;
    };
    let Some(world_close_rel) = svg[..svg_close].rfind("</g>") else {
        return svg;
    };
    svg.insert_str(world_close_rel, &overlay);
    svg
}

fn render(topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    let events = derive_pc_datapath(trace);
    let stride = (events.len() / 150).max(1);
    let total = config.total();
    let mut out = String::with_capacity(240_000);
    out.push_str("<g id=\"f3-pc\">\n");

    for event in events.iter().step_by(stride) {
        let moment = trace_moment(event.frame, event.ordinal, trace, config);
        match event.kind {
            PcDatapathKind::Increment(increment) => {
                pulse_group(
                    &mut out,
                    moment,
                    total,
                    "increment",
                    increment.before,
                    increment.after,
                    if increment.low_byte_carry() { "1" } else { "0" },
                    |out| {
                        glow_node(out, topology, "pcIncLo", "#67d9b3");
                        if increment.low_byte_carry() {
                            glow_node(out, topology, "pcCarry", "#ffe16a");
                            glow_node(out, topology, "pcIncHi", "#67d9b3");
                        }
                        for bit in 0..16 {
                            if bit16(increment.after, bit) {
                                glow_node(out, topology, &format!("pcBit{bit}"), "#67d9b3");
                            }
                        }
                    },
                );
            }
            PcDatapathKind::Load {
                before,
                after,
                source,
            } => {
                let color = match source {
                    PcSource::Jump => "#ff9b71",
                    PcSource::Branch => "#e8e677",
                    PcSource::Call => "#ef7caf",
                    PcSource::Return => "#72d4e7",
                };
                pulse_group(
                    &mut out,
                    moment,
                    total,
                    source.as_str(),
                    before,
                    after,
                    "0",
                    |out| {
                        glow_node(out, topology, "pcSelectLatch", color);
                        glow_node(out, topology, "pcMuxLo", color);
                        glow_node(out, topology, "pcMuxHi", color);
                        for bit in 0..16 {
                            if bit16(after, bit) {
                                glow_node(out, topology, &format!("pcBit{bit}"), color);
                            }
                        }
                    },
                );
            }
        }
    }

    out.push_str("</g>\n");
    out
}

fn pulse_group<F>(
    out: &mut String,
    moment: f32,
    total: f32,
    kind: &str,
    before: u16,
    after: u16,
    carry: &str,
    render: F,
) where
    F: FnOnce(&mut String),
{
    let k1 = norm(moment, total);
    let k2 = norm(moment + 0.025, total);
    let k3 = norm(moment + 0.13, total);
    out.push_str(&format!(
        "<g opacity=\"0\" data-pc-kind=\"{kind}\" data-pc-before=\"{before:04X}\" data-pc-after=\"{after:04X}\" data-pc-low-carry=\"{carry}\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>"
    ));
    render(out);
    out.push_str("</g>\n");
}

fn trace_moment(frame: u32, ordinal: u16, trace: &MatchTrace, config: RenderConfig) -> f32 {
    config.game_start()
        + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
        + f32::from(ordinal.min(15)) * 0.0035
}

fn glow_node(out: &mut String, topology: &Topology, id: &str, color: &str) {
    let Some(node) = topology.node(id) else {
        return;
    };
    let b = node.bounds;
    out.push_str(&format!(
        "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"8\" fill=\"{}\" fill-opacity=\".20\" stroke=\"{}\" stroke-width=\"9\" filter=\"url(#glow)\"/>",
        b.x - 3.0,
        b.y - 3.0,
        b.w + 6.0,
        b.h + 6.0,
        color,
        color
    ));
}

fn norm(value: f32, total: f32) -> f32 {
    (value / total).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::{build_topology, Machine};

    #[test]
    fn overlay_contains_exact_pc_activity_for_real_match() {
        let topology = build_topology();
        let trace = Machine::run_match("pc-overlay", 5000);
        let rendered = render(&topology, &trace, RenderConfig::default());
        assert!(rendered.contains("id=\"f3-pc\""));
        assert!(rendered.contains("data-pc-kind=\"increment\""));
        assert!(rendered.contains("data-pc-kind=\"call\""));
        assert!(rendered.contains("data-pc-kind=\"return\""));
        assert!(rendered.contains("data-pc-before=\""));
        assert!(rendered.contains("data-pc-after=\""));
        assert!(rendered.len() > 1000);
    }

    #[test]
    fn pc_overlay_does_not_depend_on_semantic_samples() {
        let topology = build_topology();
        let mut trace = Machine::run_match("pc-overlay-native-only", 120);
        let config = RenderConfig::default();
        let baseline = render(&topology, &trace, config);
        trace.micro_samples.clear();
        assert_eq!(render(&topology, &trace, config), baseline);
    }
}
