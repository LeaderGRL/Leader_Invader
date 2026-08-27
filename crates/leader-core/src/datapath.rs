use crate::logic::{logic_trace, ripple_add, ripple_sub, AluOp, AluTrace};
use crate::trace::{MatchTrace, PhaseKind};

/// Bit-accurate state for the F3 fetch/decode critical path.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DatapathState {
    pub pc: u16,
    pub mar: u16,
    pub mdr: u8,
    pub ir: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatapathEvent {
    pub frame: u32,
    pub ordinal: u16,
    pub phase: PhaseKind,
    pub state: DatapathState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AluDatapathEvent {
    pub frame: u32,
    pub ordinal: u16,
    pub trace: AluTrace,
}

#[must_use]
pub fn derive_datapath(trace: &MatchTrace) -> Vec<DatapathEvent> {
    let mut state = DatapathState::default();
    let mut events = Vec::with_capacity(trace.micro_samples.len());

    for sample in &trace.micro_samples {
        match sample.phase {
            PhaseKind::Fetch => {
                state.pc = sample.pc;
                if let Some(address) = sample.address {
                    state.mar = address;
                }
                if let Some(data) = sample.data {
                    state.mdr = data;
                }
            }
            PhaseKind::Decode => {
                if sample.address == Some(sample.pc) {
                    if let Some(opcode) = sample.data {
                        state.ir = opcode;
                    }
                }
            }
            _ => {}
        }

        events.push(DatapathEvent {
            frame: sample.frame,
            ordinal: sample.ordinal,
            phase: sample.phase,
            state,
        });
    }
    events
}

/// Reconstructs exact full-adder slice state for arithmetic instructions from the
/// causal instruction trace. Immediate operands are the real bytes fetched by the
/// CPU. The pre-operation lhs is recovered from the recorded result using modular
/// byte arithmetic, then the result is recomputed through the F3 ripple network.
#[must_use]
pub fn derive_alu_datapath(trace: &MatchTrace) -> Vec<AluDatapathEvent> {
    let mut events = Vec::new();

    for (index, sample) in trace.micro_samples.iter().enumerate() {
        if sample.phase != PhaseKind::Alu {
            continue;
        }
        let Some(result) = sample.data else {
            continue;
        };
        let operands = instruction_operand_bytes(trace, index, sample.pc);
        let derived = match sample.control.as_str() {
            "ADDI" => operands.last().copied().map(|rhs| {
                let lhs = result.wrapping_sub(rhs);
                ripple_add(lhs, rhs, false, AluOp::Add)
            }),
            "SUBI" => operands.last().copied().map(|rhs| {
                let lhs = result.wrapping_add(rhs);
                ripple_sub(lhs, rhs, AluOp::Sub)
            }),
            "CMPI" => operands.last().copied().map(|rhs| {
                let lhs = result.wrapping_add(rhs);
                ripple_sub(lhs, rhs, AluOp::Compare)
            }),
            "INC" => Some(ripple_add(result.wrapping_sub(1), 1, false, AluOp::Add)),
            "DEC" => Some(ripple_sub(result.wrapping_add(1), 1, AluOp::Sub)),
            "ANDI" => operands.last().copied().map(|rhs| {
                logic_trace(AluOp::And, result, rhs, result)
            }),
            "ORI" => operands.last().copied().map(|rhs| {
                logic_trace(AluOp::Or, result, rhs, result)
            }),
            "XORI" => operands.last().copied().map(|rhs| {
                logic_trace(AluOp::Xor, result, rhs, result)
            }),
            "LDI" | "MOV" => Some(logic_trace(AluOp::Pass, result, 0, result)),
            _ => None,
        };

        if let Some(trace) = derived {
            events.push(AluDatapathEvent {
                frame: sample.frame,
                ordinal: sample.ordinal,
                trace,
            });
        }
    }
    events
}

fn instruction_operand_bytes(trace: &MatchTrace, alu_index: usize, pc: u16) -> Vec<u8> {
    let decode_index = trace.micro_samples[..alu_index]
        .iter()
        .rposition(|sample| {
            sample.phase == PhaseKind::Decode
                && sample.pc == pc
                && sample.address == Some(pc)
                && sample.data.is_some()
        });
    let Some(decode_index) = decode_index else {
        return Vec::new();
    };
    trace.micro_samples[decode_index + 1..alu_index]
        .iter()
        .filter(|sample| sample.phase == PhaseKind::Fetch && sample.pc == pc)
        .filter_map(|sample| sample.data)
        .collect()
}

#[must_use]
pub const fn bit16(value: u16, bit: usize) -> bool {
    value & (1_u16 << bit) != 0
}

#[must_use]
pub const fn bit8(value: u8, bit: usize) -> bool {
    value & (1_u8 << bit) != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Machine;

    #[test]
    fn fetch_latches_real_pc_mar_mdr_and_decode_latches_ir() {
        let trace = Machine::run_match("f3-fetch", 5000);
        let events = derive_datapath(&trace);
        let fetch = trace
            .micro_samples
            .iter()
            .position(|sample| sample.phase == PhaseKind::Fetch)
            .expect("fetch sample");
        let sample = &trace.micro_samples[fetch];
        assert_eq!(events[fetch].state.pc, sample.pc);
        assert_eq!(events[fetch].state.mar, sample.address.expect("fetch address"));
        assert_eq!(events[fetch].state.mdr, sample.data.expect("fetch byte"));

        let decode = trace
            .micro_samples
            .iter()
            .position(|sample| sample.phase == PhaseKind::Decode && sample.data.is_some())
            .expect("decode sample");
        assert_eq!(events[decode].state.ir, trace.micro_samples[decode].data.unwrap());
    }

    #[test]
    fn current_rom_compare_is_recomputed_by_real_ripple_chain() {
        let trace = Machine::run_match("f3-alu", 5000);
        let events = derive_alu_datapath(&trace);
        let compare = events
            .iter()
            .find(|event| event.trace.op == AluOp::Compare)
            .expect("CMPI ripple event");
        assert_eq!(
            compare.trace.result,
            compare.trace.lhs.wrapping_sub(compare.trace.rhs)
        );
    }
}
