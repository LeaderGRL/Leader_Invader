use leader_core::{MatchTrace, PhaseKind, Topology};
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
    let native_count = trace
        .micro_samples
        .iter()
        .filter(|sample| matches!(sample.control.as_str(), "µT0" | "µT1" | "µT2"))
        .count();
    let stride = (native_count / 260).max(1);
    let total = config.total();
    let mut out = String::with_capacity(180_000);
    out.push_str("<g id=\"f3-timing\">\n");

    let mut native_index = 0_usize;
    for sample in &trace.micro_samples {
        let base = trace_moment(sample.frame, sample.ordinal, trace, config);
        let native = match sample.control.as_str() {
            "µT0" => Some(("phase0", "#67d9b3")),
            "µT1" => Some(("phase1", "#4bc8f3")),
            "µT2" => Some(("phase2", "#ef7caf")),
            _ => None,
        };

        if let Some((node, color)) = native {
            if native_index % stride == 0 {
                phase_pulse(&mut out, topology, node, base, total, color);
            }
            native_index += 1;
            continue;
        }

        // DMA and raster scanout are peripheral clocks, not CPU T-states.
        // Keep them visible as an independent hardware cadence.
        match sample.phase {
            PhaseKind::Dma => {
                phase_pulse(&mut out, topology, "phase0", base, total, "#72d4e7");
                phase_pulse(&mut out, topology, "phase1", base + 0.024, total, "#72d4e7");
            }
            PhaseKind::Scanout => {
                phase_pulse(&mut out, topology, "phase1", base, total, "#72d4e7");
            }
            _ => {}
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
        let trace = Machine::run_match("timing-overlay", 5000);
        assert!(trace.micro_samples.iter().any(|sample| sample.control == "µT0"));
        assert!(trace.micro_samples.iter().any(|sample| sample.control == "µT1"));
        assert!(trace.micro_samples.iter().any(|sample| sample.control == "µT2"));
        let rendered = render(&topology, &trace, RenderConfig::default());
        assert!(rendered.contains("id=\"f3-timing\""));
        assert!(rendered.len() > 500);
    }
}
