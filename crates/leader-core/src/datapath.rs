use crate::isa::{op, Reg};
use crate::logic::{logic_trace, ripple_add, ripple_sub, AluOp, AluTrace};
use crate::trace::{MatchTrace, MicroSample, PhaseKind};

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
    pub pc: u16,
    pub trace: AluTrace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterDatapathEvent {
    pub frame: u32,
    pub ordinal: u16,
    pub pc: u16,
    pub reg: Reg,
    pub before: u8,
    pub after: u8,
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

/// Returns exact full-adder state for the ALU operations executed by the CPU.
///
/// F3 arithmetic is now semantically computed by the same ripple implementation in
/// `isa.rs`. The renderer repeats that pure operation over the actual fetched
/// operands so the generated SVG can remain a compact declarative replay.
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
            "ANDI" => operands
                .last()
                .copied()
                .map(|rhs| logic_trace(AluOp::And, result, rhs, result)),
            "ORI" => operands
                .last()
                .copied()
                .map(|rhs| logic_trace(AluOp::Or, result, rhs, result)),
            "XORI" => operands
                .last()
                .copied()
                .map(|rhs| logic_trace(AluOp::Xor, result, rhs, result)),
            "LDI" | "MOV" => Some(logic_trace(AluOp::Pass, result, 0, result)),
            _ => None,
        };

        if let Some(alu_trace) = derived {
            events.push(AluDatapathEvent {
                frame: sample.frame,
                ordinal: sample.ordinal,
                pc: sample.pc,
                trace: alu_trace,
            });
        }
    }
    events
}

/// Replays register-file write enables from the real instruction stream.
///
/// The state starts at CPU reset (all eight registers are zero). Each event is
/// sourced from either the instruction's CPU ALU result or its CPU memory read,
/// never from presentation heuristics.
#[must_use]
pub fn derive_register_datapath(trace: &MatchTrace) -> Vec<RegisterDatapathEvent> {
    let mut registers = [0_u8; 8];
    let mut events = Vec::new();
    let samples = &trace.micro_samples;
    let mut index = 0;

    while index < samples.len() {
        let decode = &samples[index];
        if !is_opcode_decode(decode) {
            index += 1;
            continue;
        }
        let opcode = decode.data.expect("opcode decode has data");
        let end = next_opcode_decode(samples, index + 1).unwrap_or(samples.len());
        let instruction = &samples[index + 1..end];
        let operands = instruction
            .iter()
            .filter(|sample| sample.phase == PhaseKind::Fetch)
            .filter_map(|sample| sample.data)
            .collect::<Vec<_>>();

        if let Some(reg) = destination_register(opcode, &operands) {
            if let Some(source) = register_write_source(opcode, instruction) {
                let slot = &mut registers[reg as usize];
                let before = *slot;
                *slot = source.data.expect("register source has value");
                events.push(RegisterDatapathEvent {
                    frame: source.frame,
                    ordinal: source.ordinal,
                    pc: decode.pc,
                    reg,
                    before,
                    after: *slot,
                });
            }
        }

        index = end;
    }

    events
}

fn is_opcode_decode(sample: &MicroSample) -> bool {
    sample.phase == PhaseKind::Decode
        && sample.address == Some(sample.pc)
        && sample.data.is_some()
}

fn next_opcode_decode(samples: &[MicroSample], start: usize) -> Option<usize> {
    samples[start..]
        .iter()
        .position(is_opcode_decode)
        .map(|offset| start + offset)
}

fn destination_register(opcode: u8, operands: &[u8]) -> Option<Reg> {
    let writes_register = matches!(
        opcode,
        op::LDI
            | op::LD
            | op::MOV
            | op::ADD
            | op::ADDI
            | op::SUBI
            | op::ANDI
            | op::ORI
            | op::XORI
            | op::INC
            | op::DEC
    );
    writes_register
        .then(|| operands.first().copied())
        .flatten()
        .and_then(reg_from_code)
}

fn reg_from_code(code: u8) -> Option<Reg> {
    match code {
        0 => Some(Reg::A),
        1 => Some(Reg::B),
        2 => Some(Reg::C),
        3 => Some(Reg::D),
        4 => Some(Reg::X),
        5 => Some(Reg::Y),
        6 => Some(Reg::T),
        7 => Some(Reg::U),
        _ => None,
    }
}

fn register_write_source(opcode: u8, instruction: &[MicroSample]) -> Option<&MicroSample> {
    if opcode == op::LD {
        return instruction
            .iter()
            .find(|sample| sample.phase == PhaseKind::MemoryRead && sample.control == "CPU_READ");
    }

    instruction.iter().find(|sample| {
        sample.phase == PhaseKind::Alu
            && matches!(
                sample.control.as_str(),
                "LDI"
                    | "MOV"
                    | "ADD"
                    | "ADDI"
                    | "SUBI"
                    | "ANDI"
                    | "ORI"
                    | "XORI"
                    | "INC"
                    | "DEC"
            )
    })
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
        .filter(|sample| sample.phase == PhaseKind::Fetch)
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

    #[test]
    fn register_file_replay_tracks_real_a_writes() {
        let trace = Machine::run_match("f3-regs", 5000);
        let writes = derive_register_datapath(&trace);
        assert!(!writes.is_empty());
        let first_a = writes
            .iter()
            .find(|event| event.reg == Reg::A)
            .expect("A write");
        assert_eq!(first_a.before, 0);
        assert!(writes.iter().any(|event| event.reg == Reg::A && event.after == 1));
    }
}
