use crate::trace::{MatchTrace, MicroSample, PhaseKind};
use crate::MicroCycleKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecoderDatapathEvent {
    pub frame: u32,
    pub ordinal: u16,
    pub pc: u16,
    pub opcode: u8,
    pub high_line: u8,
    pub low_line: u8,
}

#[must_use]
pub fn derive_decoder_datapath(trace: &MatchTrace) -> Vec<DecoderDatapathEvent> {
    if !trace.micro_cycles.is_empty() {
        return trace
            .micro_cycles
            .iter()
            .filter(|event| event.kind == MicroCycleKind::DecodeLatch)
            .map(|event| decode_event(event.frame, event.ordinal, event.pc, event.ir))
            .collect();
    }

    trace
        .micro_samples
        .iter()
        .filter(|sample| is_opcode_decode(sample))
        .filter_map(|sample| {
            sample
                .data
                .map(|opcode| decode_event(sample.frame, sample.ordinal, sample.pc, opcode))
        })
        .collect()
}

fn decode_event(frame: u32, ordinal: u16, pc: u16, opcode: u8) -> DecoderDatapathEvent {
    DecoderDatapathEvent {
        frame,
        ordinal,
        pc,
        opcode,
        high_line: opcode >> 4,
        low_line: opcode & 0x0f,
    }
}

fn is_opcode_decode(sample: &MicroSample) -> bool {
    sample.phase == PhaseKind::Decode
        && !matches!(sample.control.as_str(), "µT0" | "µT1" | "µT2")
        && sample.address == Some(sample.pc)
        && sample.data.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{isa::op, Machine};

    #[test]
    fn native_decode_latches_are_authoritative() {
        let trace = Machine::run_match("f3-decoder-core", 5000);
        let baseline = derive_decoder_datapath(&trace);
        assert!(!baseline.is_empty());
        assert!(baseline.iter().any(|event| event.opcode == op::CMPI));

        let mut native_only = trace.clone();
        native_only.micro_samples.clear();
        assert_eq!(derive_decoder_datapath(&native_only), baseline);
    }

    #[test]
    fn native_decode_lines_match_ir_nibbles() {
        let trace = Machine::run_match("f3-decoder-lines", 5000);
        for event in derive_decoder_datapath(&trace) {
            assert_eq!(event.high_line, event.opcode >> 4);
            assert_eq!(event.low_line, event.opcode & 0x0f);
        }
    }

    #[test]
    fn historical_sample_fallback_remains_available() {
        let mut trace = Machine::run_match("f3-decoder-legacy", 5000);
        trace.micro_cycles.clear();
        let events = derive_decoder_datapath(&trace);
        assert!(!events.is_empty());
        assert!(events.iter().any(|event| event.opcode == op::CMPI));
    }
}
