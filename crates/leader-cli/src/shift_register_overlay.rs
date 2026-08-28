use leader_core::{MatchTrace, ShiftRegisterEventKind, Topology};
use leader_svg::RenderConfig;

#[must_use]
pub fn apply(
    mut svg: String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
) -> String {
    if trace.shift_register_events.is_empty() || trace.total_frames == 0 {
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
    let total = config.total();
    let mut out = String::with_capacity(18_000);
    out.push_str("<g id=\"m3-shift-register\">\n");

    for event in &trace.shift_register_events {
        let moment = trace_moment(event.frame, event.ordinal, trace, config) + 0.010;
        let k1 = norm(moment, total);
        let k2 = norm(moment + 0.025, total);
        let k3 = norm(moment + 0.210, total);

        match event.kind {
            ShiftRegisterEventKind::DataWrite {
                before,
                after,
                input,
            } => {
                out.push_str(&format!(
                    "<g opacity=\"0\" data-shift-kind=\"data_write\" data-shift-state=\"{before:04X}:{after:04X}\" data-shift-input=\"{input:02X}\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>"
                ));
                glow(topology, &mut out, "shiftHi", "#67d9b3");
                glow(topology, &mut out, "shiftLo", "#67d9b3");
                out.push_str("</g>\n");
            }
            ShiftRegisterEventKind::OffsetWrite {
                before,
                after,
                input,
            } => {
                out.push_str(&format!(
                    "<g opacity=\"0\" data-shift-kind=\"offset_write\" data-shift-offset=\"{before}:{after}\" data-shift-input=\"{input:02X}\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>"
                ));
                glow(topology, &mut out, "shiftOffset", "#e8e677");
                out.push_str("</g>\n");
            }
            ShiftRegisterEventKind::Read {
                value,
                offset,
                result,
            } => {
                out.push_str(&format!(
                    "<g opacity=\"0\" data-shift-kind=\"read\" data-shift-state=\"{value:04X}\" data-shift-offset=\"{offset}\" data-shift-result=\"{result:02X}\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>"
                ));
                for id in ["shiftHi", "shiftLo", "shiftOffset", "shiftMux", "shiftOut"] {
                    glow(topology, &mut out, id, "#4bc8f3");
                }
                out.push_str("</g>\n");
            }
        }
    }

    out.push_str("</g>\n");
    out
}

fn glow(topology: &Topology, out: &mut String, id: &str, color: &str) {
    let Some(node) = topology.node(id) else {
        return;
    };
    let b = node.bounds;
    out.push_str(&format!(
        "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"7\" fill=\"{color}\" fill-opacity=\".24\" stroke=\"{color}\" stroke-width=\"7\" filter=\"url(#glow)\"/>",
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
    fn native_shift_overlay_exposes_exact_boot_values() {
        let topology = build_topology();
        let trace = Machine::run_match("m3-shift-overlay", 120);
        let svg = render(&topology, &trace, RenderConfig::default());
        assert!(svg.contains("id=\"m3-shift-register\""));
        assert!(svg.contains("data-shift-state=\"0000:1200\""));
        assert!(svg.contains("data-shift-state=\"1200:3412\""));
        assert!(svg.contains("data-shift-offset=\"0:3\""));
        assert!(svg.contains("data-shift-result=\"A0\""));
    }
}
