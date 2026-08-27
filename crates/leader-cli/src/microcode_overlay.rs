use leader_core::{
    execute_address, execute_row_kind, ExecuteRowKind, MatchTrace, MicroAddressSource, Topology,
};
use leader_svg::RenderConfig;

#[derive(Debug, Clone, Copy, Default)]
struct SourceSampler {
    fetch_seen: usize,
    sequential_seen: usize,
    execute_seen: usize,
    dispatch_seen: usize,
    call_seen: usize,
    return_seen: usize,
    fetch_stride: usize,
    sequential_stride: usize,
    execute_stride: usize,
    dispatch_stride: usize,
    call_stride: usize,
    return_stride: usize,
}

impl SourceSampler {
    fn for_trace(trace: &MatchTrace) -> Self {
        let mut counts = [0_usize; 5];
        let mut execute_count = 0_usize;
        for event in &trace.micro_addresses {
            if event.source == MicroAddressSource::Sequential && event.address >= 0x80 {
                execute_count += 1;
            } else {
                counts[source_index(event.source)] += 1;
            }
        }
        Self {
            fetch_stride: (counts[0] / 24).max(1),
            sequential_stride: (counts[1] / 120).max(1),
            execute_stride: (execute_count / 96).max(1),
            dispatch_stride: (counts[2] / 60).max(1),
            call_stride: (counts[3] / 70).max(1),
            return_stride: (counts[4] / 70).max(1),
            ..Self::default()
        }
    }

    fn take(&mut self, source: MicroAddressSource, address: u8) -> bool {
        if source == MicroAddressSource::Sequential && address >= 0x80 {
            let take = self.execute_seen % self.execute_stride == 0;
            self.execute_seen += 1;
            return take;
        }
        let (seen, stride) = match source {
            MicroAddressSource::FetchStart => (&mut self.fetch_seen, self.fetch_stride),
            MicroAddressSource::Sequential => (&mut self.sequential_seen, self.sequential_stride),
            MicroAddressSource::Dispatch => (&mut self.dispatch_seen, self.dispatch_stride),
            MicroAddressSource::RoutineCall => (&mut self.call_seen, self.call_stride),
            MicroAddressSource::RoutineReturn => (&mut self.return_seen, self.return_stride),
        };
        let take = *seen % stride == 0;
        *seen += 1;
        take
    }
}

const fn source_index(source: MicroAddressSource) -> usize {
    match source {
        MicroAddressSource::FetchStart => 0,
        MicroAddressSource::Sequential => 1,
        MicroAddressSource::Dispatch => 2,
        MicroAddressSource::RoutineCall => 3,
        MicroAddressSource::RoutineReturn => 4,
    }
}

#[must_use]
pub fn apply(mut svg: String, topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    if trace.total_frames == 0 || trace.micro_addresses.is_empty() { return svg; }
    let overlay = render(topology, trace, config);
    let Some(svg_close) = svg.rfind("</svg>") else { return svg; };
    let Some(world_close) = svg[..svg_close].rfind("</g>") else { return svg; };
    svg.insert_str(world_close, &overlay);
    svg
}

fn render(topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    let total = config.total();
    let mut sampler = SourceSampler::for_trace(trace);
    let mut out = String::with_capacity(380_000);
    out.push_str("<g id=\"f3-microcode\">\n");

    for event in &trace.micro_addresses {
        if !sampler.take(event.source, event.address) { continue; }
        let moment = trace_moment(event.frame, event.ordinal, trace, config) + 0.008;
        let address_color = transition_color(event.source, event.address);
        pulse_group(&mut out, moment, total, event.before, event.address, event.source, |out| {
            glow_node(out, topology, "microAddr", address_color);
            for bit in 0..8 {
                if event.address & (1 << bit) != 0 { glow_node(out, topology, &format!("microAddrBit{bit}"), address_color); }
            }
            glow_node(out, topology, "microRom", "#ef7caf");
            for (bit, id, color) in [
                (0, "ctrlRegWrite", "#67d9b3"), (1, "ctrlAlu", "#f7ce62"),
                (2, "ctrlMemRead", "#4bc8f3"), (3, "ctrlMemWrite", "#ff9b71"),
                (4, "ctrlPcLoad", "#e8e677"), (5, "ctrlStack", "#ef7caf"),
                (6, "ctrlWait", "#72d4e7"), (7, "ctrlHalt", "#ff6961"),
            ] {
                if event.control_bits & (1 << bit) != 0 { glow_node(out, topology, id, color); }
            }
            render_execute_path(out, topology, event.opcode, event.address, event.control_bits);
        });
    }
    out.push_str("</g>\n");
    out
}

fn render_execute_path(out: &mut String, topology: &Topology, opcode: u8, address: u8, control_bits: u8) {
    let Some(base) = execute_address(opcode) else { return; };
    if address < base || address >= base.saturating_add(5) { return; }
    let step = address - base;
    let Some(kind) = execute_row_kind(opcode, step) else { return; };

    match kind {
        ExecuteRowKind::Operand => {
            let mux = if step == 0 { "readMuxA" } else { "readMuxB" };
            glow_node(out, topology, mux, "#4bc8f3");
        }
        ExecuteRowKind::Address => {
            glow_node(out, topology, "addrBuf", "#4bc8f3");
            for bit in 0..16 { glow_node(out, topology, &format!("marBit{bit}"), "#4bc8f3"); }
        }
        ExecuteRowKind::AluSelect => {
            glow_node(out, topology, "readMuxA", "#4bc8f3");
            glow_node(out, topology, "readMuxB", "#4bc8f3");
            glow_node(out, topology, "aluSel", "#f7ce62");
        }
        ExecuteRowKind::Propagate => {
            glow_node(out, topology, "readMuxA", "#4bc8f3");
            glow_node(out, topology, "readMuxB", "#4bc8f3");
            glow_node(out, topology, "aluSel", "#f7ce62");
            for bit in 0..8 {
                glow_node(out, topology, &format!("xorB{bit}"), "#f7ce62");
                glow_node(out, topology, &format!("orC{bit}"), "#ff9b71");
                glow_node(out, topology, &format!("muxR{bit}"), "#67d9b3");
            }
            glow_flags(out, topology);
        }
        ExecuteRowKind::Memory => {
            glow_node(out, topology, "addrBuf", "#4bc8f3");
            glow_node(out, topology, "dataBuf", "#67d9b3");
            glow_node(out, topology, "ctrlBuf", "#ff9b71");
        }
        ExecuteRowKind::Commit => {
            if control_bits & 0b10 != 0 {
                glow_node(out, topology, "aluSel", "#f7ce62");
                for bit in 0..8 { glow_node(out, topology, &format!("muxR{bit}"), "#67d9b3"); }
                glow_flags(out, topology);
            }
            if control_bits & 0b1 != 0 {
                glow_node(out, topology, "writeDec", "#67d9b3");
                glow_node(out, topology, "writeBus", "#67d9b3");
            }
        }
        ExecuteRowKind::Idle => {}
    }
}

fn glow_flags(out: &mut String, topology: &Topology) {
    glow_node(out, topology, "flagZ", "#e8e677");
    glow_node(out, topology, "flagC", "#ff9b71");
    glow_node(out, topology, "flagN", "#ef7caf");
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

fn pulse_group<F>(out: &mut String, moment: f32, total: f32, before: u8, address: u8, source: MicroAddressSource, render: F)
where F: FnOnce(&mut String) {
    let k1 = norm(moment, total); let k2 = norm(moment + 0.026, total); let k3 = norm(moment + 0.125, total);
    out.push_str(&format!(
        "<g opacity=\"0\" data-uaddr-from=\"{before:02X}\" data-uaddr=\"{address:02X}\" data-usource=\"{}\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>", source.as_str()
    ));
    render(out); out.push_str("</g>\n");
}

fn trace_moment(frame: u32, ordinal: u16, trace: &MatchTrace, config: RenderConfig) -> f32 {
    config.game_start() + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds + f32::from(ordinal.min(63)) * 0.0018
}

fn glow_node(out: &mut String, topology: &Topology, id: &str, color: &str) {
    let Some(node) = topology.node(id) else { return; }; let b = node.bounds;
    out.push_str(&format!(
        "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"7\" fill=\"{}\" fill-opacity=\".22\" stroke=\"{}\" stroke-width=\"8\" filter=\"url(#glow)\"/>",
        b.x - 3.0, b.y - 3.0, b.w + 6.0, b.h + 6.0, color, color
    ));
}

fn norm(value: f32, total: f32) -> f32 { (value / total).clamp(0.0, 1.0) }

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::{build_topology, isa::op, Machine};

    #[test]
    fn sampler_preserves_each_transition_class() {
        let trace = Machine::run_match("microcode-sampler", 5000); let mut sampler = SourceSampler::for_trace(&trace);
        let mut rendered_source = [false; 5]; let mut rendered_execute = false;
        for event in &trace.micro_addresses {
            if sampler.take(event.source, event.address) {
                rendered_source[source_index(event.source)] = true;
                rendered_execute |= event.source == MicroAddressSource::Sequential && event.address >= 0x80;
            }
        }
        for (index, seen) in rendered_source.into_iter().enumerate() { assert!(seen, "microaddress source {index} was lost during sampling"); }
        assert!(rendered_execute, "execute-row progression was lost during sampling");
    }

    #[test]
    fn complete_match_contains_cmpi_and_memory_five_row_progression() {
        let trace = Machine::run_match("microcode-five-row", 5000);
        for opcode in [op::CMPI, op::LD, op::ST] {
            let base = execute_address(opcode).unwrap();
            for step in 0..5 {
                assert!(trace.micro_addresses.iter().any(|event| event.opcode == opcode && event.address == base + step), "missing opcode {opcode:02X} execute row {step}");
            }
        }
    }

    #[test]
    fn real_replay_renders_semantic_micro_pc_transitions() {
        let topology = build_topology(); let trace = Machine::run_match("microcode-overlay", 5000);
        assert!(!trace.micro_addresses.is_empty()); assert!(trace.micro_addresses.iter().any(|event| event.address == 0x00));
        assert!(trace.micro_addresses.iter().any(|event| event.address >= 0x80)); assert!(trace.micro_addresses.iter().any(|event| event.source == MicroAddressSource::Dispatch));
        assert!(trace.micro_addresses.iter().any(|event| event.source == MicroAddressSource::RoutineCall)); assert!(trace.micro_addresses.iter().any(|event| event.source == MicroAddressSource::RoutineReturn));
        for bit in 0..8 { assert!(topology.node(&format!("microAddrBit{bit}")).is_some()); }
        let rendered = render(&topology, &trace, RenderConfig::default());
        assert!(rendered.contains("id=\"f3-microcode\"")); assert!(rendered.contains("data-uaddr=\"")); assert!(rendered.contains("data-usource=\"dispatch\""));
        assert!(rendered.contains("data-usource=\"routine_call\"")); assert!(rendered.contains("data-usource=\"routine_return\"")); assert!(rendered.len() > 1000);
    }
}
