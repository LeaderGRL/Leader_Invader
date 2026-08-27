use leader_core::{MatchTrace, MicroAddressSource, Topology};
use leader_svg::RenderConfig;

#[must_use]
pub fn apply(mut svg: String, topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
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
    let stride = (trace.micro_addresses.len() / 320).max(1);
    let total = config.total();
    let mut out = String::with_capacity(260_000);
    out.push_str("<g id=\"f3-microcode\">\n");

    for event in trace.micro_addresses.iter().step_by(stride) {
        let moment = trace_moment(event.frame, event.ordinal, trace, config) + 0.008;
        let address_color = transition_color(event.source, event.address);

        pulse_group(
            &mut out,
            moment,
            total,
            event.before,
            event.address,
            event.source,
            |out| {
                glow_node(out, topology, "microAddr", address_color);
                for bit in 0..8 {
                    if event.address & (1 << bit) != 0 {
                        glow_node(out, topology, &format!("microAddrBit{bit}"), address_color);
                    }
                }

                // The physical ROM row selected by the semantic µPC emits the exact
                // eight visible control bits recorded at that transition.
                glow_node(out, topology, "microRom", "#ef7caf");
                for (bit, id, color) in [
                    (0, "ctrlRegWrite", "#67d9b3"),
                    (1, "ctrlAlu", "#f7ce62"),
                    (2, "ctrlMemRead", "#4bc8f3"),
                    (3, "ctrlMemWrite", "#ff9b71"),
                    (4, "ctrlPcLoad", "#e8e677"),
                    (5, "ctrlStack", "#ef7caf"),
                    (6, "ctrlWait", "#72d4e7"),
                    (7, "ctrlHalt", "#ff6961"),
                ] {
                    if event.control_bits & (1 << bit) != 0 {
                        glow_node(out, topology, id, color);
                    }
                }
            },
        );
    }

    out.push_str("</g>\n");
    out
}

fn transition_color(source: MicroAddressSource, address: u8) -> &'static str {
    match source {
        MicroAddressSource::Dispatch => "#f7ce62",
        MicroAddressSource::RoutineCall => "#4bc8f3",
        MicroAddressSource::RoutineReturn => "#67d9b3",
        MicroAddressSource::FetchStart => "#ef7caf",
        MicroAddressSource::Sequential if address >= 0x80 => "#f7ce62",
        MicroAddressSource::Sequential => "#ef7caf",
    }
}

fn pulse_group<F>(
    out: &mut String,
    moment: f32,
    total: f32,
    before: u8,
    address: u8,
    source: MicroAddressSource,
    render: F,
) where
    F: FnOnce(&mut String),
{
    let k1 = norm(moment, total);
    let k2 = norm(moment + 0.026, total);
    let k3 = norm(moment + 0.125, total);
    out.push_str(&format!(
        "<g opacity=\"0\" data-uaddr-from=\"{before:02X}\" data-uaddr=\"{address:02X}\" data-usource=\"{}\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>",
        source.as_str()
    ));
    render(out);
    out.push_str("</g>\n");
}

fn trace_moment(frame: u32, ordinal: u16, trace: &MatchTrace, config: RenderConfig) -> f32 {
    config.game_start()
        + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
        + f32::from(ordinal.min(63)) * 0.0018
}

fn glow_node(out: &mut String, topology: &Topology, id: &str, color: &str) {
    let Some(node) = topology.node(id) else {
        return;
    };
    let b = node.bounds;
    out.push_str(&format!(
        "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"7\" fill=\"{}\" fill-opacity=\".22\" stroke=\"{}\" stroke-width=\"8\" filter=\"url(#glow)\"/>",
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
    fn real_replay_renders_semantic_micro_pc_transitions() {
        let topology = build_topology();
        let trace = Machine::run_match("microcode-overlay", 5000);
        assert!(!trace.micro_addresses.is_empty());
        assert!(trace.micro_addresses.iter().any(|event| event.address == 0x00));
        assert!(trace.micro_addresses.iter().any(|event| event.address >= 0x80));
        assert!(trace.micro_addresses.iter().any(|event| event.source == MicroAddressSource::Dispatch));
        assert!(trace.micro_addresses.iter().any(|event| event.source == MicroAddressSource::RoutineCall));
        assert!(trace.micro_addresses.iter().any(|event| event.source == MicroAddressSource::RoutineReturn));
        for bit in 0..8 {
            assert!(topology.node(&format!("microAddrBit{bit}")).is_some());
        }
        let rendered = render(&topology, &trace, RenderConfig::default());
        assert!(rendered.contains("id=\"f3-microcode\""));
        assert!(rendered.contains("data-uaddr=\""));
        assert!(rendered.contains("data-usource=\"dispatch\""));
        assert!(rendered.contains("data-usource=\"routine_call\""));
        assert!(rendered.contains("data-usource=\"routine_return\""));
        assert!(rendered.len() > 1000);
    }
}
