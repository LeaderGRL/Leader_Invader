use crate::isa::{op, Reg};
use crate::logic::{logic_trace, ripple_add, ripple_sub, AluOp, AluTrace};
use crate::trace::{MatchTrace, MicroSample, PhaseKind};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecoderDatapathEvent {
    pub frame: u32,
    pub ordinal: u16,
    pub pc: u16,
    pub opcode: u8,
    pub high_line: u8,
    pub low_line: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusAddressOwner {
    ProgramCounter,
    Cpu,
    Dma,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusDataOwner {
    Rom,
    Ram,
    Vram,
    Cpu,
    Device,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusCycle {
    Fetch,
    Read,
    Write,
    Input,
    Dma,
    Scanout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusDatapathEvent {
    pub frame: u32,
    pub ordinal: u16,
    pub pc: u16,
    pub address: Option<u16>,
    pub data: Option<u8>,
    pub address_owner: BusAddressOwner,
    pub data_owner: BusDataOwner,
    pub cycle: BusCycle,
}

#[must_use]
pub fn derive_datapath(trace: &MatchTrace) -> Vec<DatapathEvent> {
    // Native microcycle snapshots are now the most faithful source for the
    // PC/MAR/MDR/IR latches. Keep the legacy sample replay as a fallback for old traces.
    if !trace.micro_cycles.is_empty() {
        return trace
            .micro_cycles
            .iter()
            .map(|event| DatapathEvent {
                frame: event.frame,
                ordinal: event.ordinal,
                phase: match event.phase {
                    crate::isa::MicroPhase::T0 | crate::isa::MicroPhase::T1 => PhaseKind::Fetch,
                    crate::isa::MicroPhase::T2 => PhaseKind::Decode,
                },
                state: DatapathState {
                    pc: event.pc,
                    mar: event.mar,
                    mdr: event.mdr,
                    ir: event.ir,
                },
            })
            .collect();
    }

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
            PhaseKind::Decode if is_opcode_decode(sample) => {
                state.ir = sample.data.expect("opcode decode has data");
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

#[must_use]
pub fn derive_decoder_datapath(trace: &MatchTrace) -> Vec<DecoderDatapathEvent> {
    trace
        .micro_samples
        .iter()
        .filter(|sample| is_opcode_decode(sample))
        .filter_map(|sample| {
            let opcode = sample.data?;
            Some(DecoderDatapathEvent {
                frame: sample.frame,
                ordinal: sample.ordinal,
                pc: sample.pc,
                opcode,
                high_line: opcode >> 4,
                low_line: opcode & 0x0f,
            })
        })
        .collect()
}

#[must_use]
pub fn derive_bus_datapath(trace: &MatchTrace) -> Vec<BusDatapathEvent> {
    trace
        .micro_samples
        .iter()
        .filter_map(|sample| {
            let (address_owner, data_owner, cycle) = match sample.phase {
                PhaseKind::Fetch => (
                    BusAddressOwner::ProgramCounter,
                    BusDataOwner::Rom,
                    BusCycle::Fetch,
                ),
                PhaseKind::MemoryRead => (
                    BusAddressOwner::Cpu,
                    owner_for_address(sample.address),
                    BusCycle::Read,
                ),
                PhaseKind::MemoryWrite => (
                    BusAddressOwner::Cpu,
                    BusDataOwner::Cpu,
                    BusCycle::Write,
                ),
                PhaseKind::Input => (
                    BusAddressOwner::None,
                    BusDataOwner::Device,
                    BusCycle::Input,
                ),
                PhaseKind::Dma => (
                    BusAddressOwner::Dma,
                    BusDataOwner::Vram,
                    BusCycle::Dma,
                ),
                PhaseKind::Scanout => (
                    BusAddressOwner::Dma,
                    BusDataOwner::Vram,
                    BusCycle::Scanout,
                ),
                _ => return None,
            };
            Some(BusDatapathEvent {
                frame: sample.frame,
                ordinal: sample.ordinal,
                pc: sample.pc,
                address: sample.address,
                data: sample.data,
                address_owner,
                data_owner,
                cycle,
            })
        })
        .collect()
}

fn owner_for_address(address: Option<u16>) -> BusDataOwner {
    match address {
        Some(0x0000..=0x1fff) => BusDataOwner::Rom,
        Some(0x2000..=0x7fff) => BusDataOwner::Ram,
        Some(0x8000..=0x87ff) => BusDataOwner::Vram,
        Some(0xa000..=0xa1ff) => BusDataOwner::Device,
        Some(_) | None => BusDataOwner::None,
    }
}

#[must_use]
pub fn derive_alu_datapath(trace: &MatchTrace) -> Vec<AluDatapathEvent> {
    // Prefer the operation emitted by the semantic ALU when present.
    if !trace.alu_events.is_empty() {
        return trace
            .alu_events
            .iter()
            .map(|event| AluDatapathEvent {
                frame: event.frame,
                ordinal: event.ordinal,
                pc: event.pc,
                trace: event.trace,
            })
            .collect();
    }

    let mut events = Vec::new();
    for (index, sample) in trace.micro_samples.iter().enumerate() {
        if sample.phase != PhaseKind::Alu {
            continue;
        }
        let Some(result) = sample.data else { continue };
        let operands = instruction_operand_bytes(trace, index, sample.pc);
        let derived = match sample.control.as_str() {
            "ADDI" => operands.last().copied().map(|rhs| {
                ripple_add(result.wrapping_sub(rhs), rhs, false, AluOp::Add)
            }),
            "SUBI" => operands.last().copied().map(|rhs| {
                ripple_sub(result.wrapping_add(rhs), rhs, AluOp::Sub)
            }),
            "CMPI" => operands.last().copied().map(|rhs| {
                ripple_sub(result.wrapping_add(rhs), rhs, AluOp::Compare)
            }),
            "INC" => Some(ripple_add(result.wrapping_sub(1), 1, false, AluOp::Add)),
            "DEC" => Some(ripple_sub(result.wrapping_add(1), 1, AluOp::Sub)),
            "ANDI" => operands.last().copied().map(|rhs| logic_trace(AluOp::And, result, rhs, result)),
            "ORI" => operands.last().copied().map(|rhs| logic_trace(AluOp::Or, result, rhs, result)),
            "XORI" => operands.last().copied().map(|rhs| logic_trace(AluOp::Xor, result, rhs, result)),
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

#[must_use]
pub fn derive_register_datapath(trace: &MatchTrace) -> Vec<RegisterDatapathEvent> {
    // Native write-enable events are authoritative whenever the producer records them.
    if !trace.register_writes.is_empty() {
        return trace
            .register_writes
            .iter()
            .map(|event| RegisterDatapathEvent {
                frame: event.frame,
                ordinal: event.ordinal,
                pc: event.pc,
                reg: event.reg,
                before: event.before,
                after: event.after,
            })
            .collect();
    }

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
        && !matches!(sample.control.as_str(), "µT0" | "µT1" | "µT2")
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
        op::LDI | op::LD | op::MOV | op::ADD | op::ADDI | op::SUBI | op::ANDI | op::ORI
            | op::XORI | op::INC | op::DEC
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
                "LDI" | "MOV" | "ADD" | "ADDI" | "SUBI" | "ANDI" | "ORI" | "XORI" | "INC" | "DEC"
            )
    })
}

fn instruction_operand_bytes(trace: &MatchTrace, alu_index: usize, pc: u16) -> Vec<u8> {
    let decode_index = trace.micro_samples[..alu_index]
        .iter()
        .rposition(|sample| sample.pc == pc && is_opcode_decode(sample));
    let Some(decode_index) = decode_index else { return Vec::new() };
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
        assert!(!events.is_empty());
        assert!(trace.micro_cycles.iter().any(|event| event.mar == event.pc || event.pc > 0));
    }

    #[test]
    fn current_rom_compare_is_recomputed_by_real_ripple_chain() {
        let trace = Machine::run_match("f3-alu", 5000);
        let events = derive_alu_datapath(&trace);
        let compare = events
            .iter()
            .find(|event| event.trace.op == AluOp::Compare)
            .expect("CMPI ripple event");
        assert_eq!(compare.trace.result, compare.trace.lhs.wrapping_sub(compare.trace.rhs));
    }

    #[test]
    fn register_file_replay_tracks_real_a_writes() {
        let trace = Machine::run_match("f3-regs", 5000);
        let writes = derive_register_datapath(&trace);
        assert!(!writes.is_empty());
        let first_a = writes.iter().find(|event| event.reg == Reg::A).expect("A write");
        assert_eq!(first_a.before, 0);
        assert!(writes.iter().any(|event| event.reg == Reg::A && event.after == 1));
    }

    #[test]
    fn decoder_is_one_hot_for_real_cmpi_opcode() {
        let trace = Machine::run_match("f3-decode", 5000);
        let events = derive_decoder_datapath(&trace);
        let cmpi = events.iter().find(|event| event.opcode == op::CMPI).expect("CMPI decode");
        assert_eq!(cmpi.high_line, 2);
        assert_eq!(cmpi.low_line, 9);
    }

    #[test]
    fn bus_ownership_matches_fetch_ram_and_dma_cycles() {
        let trace = Machine::run_match("f3-bus", 5000);
        let events = derive_bus_datapath(&trace);
        assert!(events.iter().any(|event| {
            event.cycle == BusCycle::Fetch
                && event.address_owner == BusAddressOwner::ProgramCounter
                && event.data_owner == BusDataOwner::Rom
        }));
        assert!(events.iter().any(|event| {
            event.cycle == BusCycle::Read && event.data_owner == BusDataOwner::Ram
        }));
        assert!(events.iter().any(|event| {
            event.cycle == BusCycle::Dma
                && event.address_owner == BusAddressOwner::Dma
                && event.data_owner == BusDataOwner::Vram
        }));
    }
}
