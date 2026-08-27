use leader_core::{control_word, derive_decoder_datapath, MatchTrace, Topology};
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
    let Some(world_close) = svg[..svg_close].rfind("</g>") else {
        return svg;
    };
    svg.insert_str(world_close, &overlay);
    svg
}

fn render(topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    let events = derive_decoder_datapath(trace);
    let stride = (events.len() / 105).max(1);
    let total = config.total();
    let mut out = String::with_capacity(180_000);
    out.push_str("<g id=\"f3-microcode\">\n");

    for event in events.iter().step_by(stride) {
        let word = control_word(event.opcode);
        let moment = trace_moment(event.frame, event.ordinal, trace, config) + 0.014;
        pulse_group(&mut out, moment, total, |out| {
            glow_node(out, topology, "microAddr", "#ef7caf");
            glow_node(out, topology, "microRom", "#ef7caf");
            for (active, id, color) in [
                (word.reg_write, "ctrlRegWrite", "#67d9b3"),
                (word.alu_enable, "ctrlAlu", "#f7ce62"),
                (word.mem_read, "ctrlMemRead", "#4bc8f3"),
                (word.mem_write, "ctrlMemWrite", "#ff9b71"),
                (word.pc_load, "ctrlPcLoad", "#e8e677"),
                (word.stack_enable, "ctrlStack", "#ef7caf"),
                (word.wait, "ctrlWait", "#72d4e7"),
                (word.halt, "ctrlHalt", "#ff6961"),
            ] {
                if active {
                    glow_node(out, topology, id, color);
                }
            }
        });
    }

    out.push_str("</g>\n");
    out
}

fn pulse_group<F>(out: &mut String, moment: f32, total: f32, render: F)
where
    F: FnOnce(&mut String),
{
    let k1 = norm(moment, total);
    let k2 = norm(moment + 0.030, total);
    let k3 = norm(moment + 0.145, total);
    out.push_str(&format!(
        "<g opacity=\"0\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>"
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
    fn real_replay_renders_control_rom_activity() {
        let topology = build_topology();
        let trace = Machine::run_match("microcode-overlay", 5000);
        let rendered = render(&topology, &trace, RenderConfig::default());
        assert!(rendered.contains("id=\"f3-microcode\""));
        assert!(rendered.len() > 1000);
    }
}
