use crate::logic::{
    logic_trace, ripple_add, ripple_decrement16, ripple_increment16, ripple_sub, AluOp, AluTrace,
    PcIncrementTrace,
};
use crate::microcode::{decode as decode_microcode, MicroOp};
use crate::trace::PhaseKind;

pub mod op {
    pub const NOP: u8 = 0x00;
    pub const LDI: u8 = 0x10;
    pub const LD: u8 = 0x11;
    pub const ST: u8 = 0x12;
    pub const MOV: u8 = 0x13;
    pub const ADD: u8 = 0x20;
    pub const ADDI: u8 = 0x21;
    pub const SUBI: u8 = 0x22;
    pub const ANDI: u8 = 0x23;
    pub const ORI: u8 = 0x24;
    pub const XORI: u8 = 0x25;
    pub const INC: u8 = 0x26;
    pub const DEC: u8 = 0x27;
    pub const CMP: u8 = 0x28;
    pub const CMPI: u8 = 0x29;
    pub const JMP: u8 = 0x30;
    pub const JZ: u8 = 0x31;
    pub const JNZ: u8 = 0x32;
    pub const JLT: u8 = 0x33;
    pub const JGE: u8 = 0x34;
    pub const JC: u8 = 0x35;
    pub const CALL: u8 = 0x36;
    pub const RET: u8 = 0x37;
    pub const WAIT_VBLANK: u8 = 0xFE;
    pub const HALT: u8 = 0xFF;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Reg { A = 0, B = 1, C = 2, D = 3, X = 4, Y = 5, T = 6, U = 7 }

impl Reg {
    pub const ALL: [Self; 8] = [Self::A, Self::B, Self::C, Self::D, Self::X, Self::Y, Self::T, Self::U];
    #[must_use] pub const fn code(self) -> u8 { self as u8 }
    #[must_use] pub const fn name(self) -> &'static str {
        match self {
            Self::A => "A", Self::B => "B", Self::C => "C", Self::D => "D",
            Self::X => "X", Self::Y => "Y", Self::T => "T", Self::U => "U",
        }
    }
    fn from_code(value: u8) -> Option<Self> { Self::ALL.get(value as usize).copied() }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags { pub zero: bool, pub carry: bool, pub less: bool }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome { Continue, WaitVBlank, Halted, Fault { pc: u16, opcode: u8 } }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcSource { Jump, Branch, Call, Return }

impl PcSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self { Self::Jump => "jump", Self::Branch => "branch", Self::Call => "call", Self::Return => "return" }
    }
}

pub trait Bus {
    fn fetch8(&mut self, pc: u16) -> u8;
    fn read8(&mut self, pc: u16, address: u16) -> u8;
    fn write8(&mut self, pc: u16, address: u16, value: u8);
    fn trace_decode(&mut self, pc: u16, opcode: u8, mnemonic: &'static str);
    fn trace_alu(&mut self, pc: u16, value: u8, control: &'static str);
    fn trace_control(&mut self, pc: u16, control: &'static str);
    fn trace_alu_exact(&mut self, pc: u16, trace: AluTrace, control: &'static str) { self.trace_alu(pc, trace.result, control); }
    fn trace_register_write(&mut self, _pc: u16, _reg: Reg, _before: u8, _after: u8, _control: &'static str) {}
    fn trace_pc_increment(&mut self, _trace: PcIncrementTrace) {}
    fn trace_pc_load(&mut self, _before: u16, _after: u16, _source: PcSource, _control: &'static str) {}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cpu {
    regs: [u8; 8],
    pc: u16,
    sp: u16,
    mar: u16,
    mdr: u8,
    ir: u8,
    flags: Flags,
    halted: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self { regs: [0; 8], pc: 0, sp: 0x7FFF, mar: 0, mdr: 0, ir: 0, flags: Flags::default(), halted: false }
    }
}

impl Cpu {
    #[must_use] pub const fn pc(&self) -> u16 { self.pc }
    #[must_use] pub const fn sp(&self) -> u16 { self.sp }
    #[must_use] pub const fn mar(&self) -> u16 { self.mar }
    #[must_use] pub const fn mdr(&self) -> u8 { self.mdr }
    #[must_use] pub const fn ir(&self) -> u8 { self.ir }
    #[must_use] pub const fn flags(&self) -> Flags { self.flags }
    #[must_use] pub fn reg(&self, reg: Reg) -> u8 { self.regs[reg as usize] }

    pub fn step<B: Bus>(&mut self, bus: &mut B) -> StepOutcome {
        if self.halted { return StepOutcome::Halted; }
        let pc = self.pc;
        let opcode = self.next8(bus);
        self.ir = opcode;
        let Some(micro) = decode_microcode(self.ir) else { return self.fault(pc, self.ir); };
        bus.trace_decode(pc, self.ir, micro.mnemonic);

        match micro.operation {
            MicroOp::Nop => StepOutcome::Continue,
            MicroOp::LoadImmediate => {
                let Some(reg) = self.next_reg(bus) else { return self.fault(pc, opcode); };
                let value = self.next8(bus);
                self.write_reg(bus, pc, reg, value, micro.mnemonic);
                self.flags = Flags { zero: value == 0, carry: false, less: false };
                bus.trace_alu_exact(pc, logic_trace(AluOp::Pass, value, 0, value), micro.mnemonic);
                StepOutcome::Continue
            }
            MicroOp::LoadMemory => {
                let Some(reg) = self.next_reg(bus) else { return self.fault(pc, opcode); };
                let address = self.next16(bus);
                let value = self.read_memory(bus, pc, address);
                self.write_reg(bus, pc, reg, value, micro.mnemonic);
                self.flags.zero = value == 0;
                self.flags.less = false;
                StepOutcome::Continue
            }
            MicroOp::StoreMemory => {
                let address = self.next16(bus);
                let Some(reg) = self.next_reg(bus) else { return self.fault(pc, opcode); };
                self.write_memory(bus, pc, address, self.regs[reg as usize]);
                StepOutcome::Continue
            }
            MicroOp::Move => {
                let Some(dst) = self.next_reg(bus) else { return self.fault(pc, opcode); };
                let Some(src) = self.next_reg(bus) else { return self.fault(pc, opcode); };
                let value = self.regs[src as usize];
                self.write_reg(bus, pc, dst, value, micro.mnemonic);
                self.flags.zero = value == 0;
                self.flags.less = false;
                bus.trace_alu_exact(pc, logic_trace(AluOp::Pass, value, 0, value), micro.mnemonic);
                StepOutcome::Continue
            }
            MicroOp::Add => {
                let Some(dst) = self.next_reg(bus) else { return self.fault(pc, opcode); };
                let Some(src) = self.next_reg(bus) else { return self.fault(pc, opcode); };
                let trace = ripple_add(self.regs[dst as usize], self.regs[src as usize], false, AluOp::Add);
                self.commit_arithmetic(bus, pc, dst, trace, micro.mnemonic);
                StepOutcome::Continue
            }
            MicroOp::AddImmediate => self.immediate_arithmetic(bus, pc, opcode, AluOp::Add, micro.mnemonic),
            MicroOp::SubImmediate => self.immediate_arithmetic(bus, pc, opcode, AluOp::Sub, micro.mnemonic),
            MicroOp::AndImmediate => self.immediate_logic(bus, pc, opcode, AluOp::And, micro.mnemonic),
            MicroOp::OrImmediate => self.immediate_logic(bus, pc, opcode, AluOp::Or, micro.mnemonic),
            MicroOp::XorImmediate => self.immediate_logic(bus, pc, opcode, AluOp::Xor, micro.mnemonic),
            MicroOp::Increment => self.unary_arithmetic(bus, pc, opcode, true),
            MicroOp::Decrement => self.unary_arithmetic(bus, pc, opcode, false),
            MicroOp::Compare => {
                let Some(lhs_reg) = self.next_reg(bus) else { return self.fault(pc, opcode); };
                let Some(rhs_reg) = self.next_reg(bus) else { return self.fault(pc, opcode); };
                let trace = ripple_sub(self.regs[lhs_reg as usize], self.regs[rhs_reg as usize], AluOp::Compare);
                self.commit_compare(bus, pc, trace, micro.mnemonic);
                StepOutcome::Continue
            }
            MicroOp::CompareImmediate => {
                let Some(reg) = self.next_reg(bus) else { return self.fault(pc, opcode); };
                let rhs = self.next8(bus);
                let trace = ripple_sub(self.regs[reg as usize], rhs, AluOp::Compare);
                self.commit_compare(bus, pc, trace, micro.mnemonic);
                StepOutcome::Continue
            }
            MicroOp::Jump => {
                let target = self.next16(bus);
                self.load_pc(bus, target, PcSource::Jump, micro.mnemonic);
                bus.trace_control(pc, micro.mnemonic);
                StepOutcome::Continue
            }
            MicroOp::JumpZero => self.branch(bus, pc, self.flags.zero, micro.mnemonic),
            MicroOp::JumpNotZero => self.branch(bus, pc, !self.flags.zero, micro.mnemonic),
            MicroOp::JumpLess => self.branch(bus, pc, self.flags.less, micro.mnemonic),
            MicroOp::JumpGreaterEqual => self.branch(bus, pc, !self.flags.less, micro.mnemonic),
            MicroOp::JumpCarry => self.branch(bus, pc, self.flags.carry, micro.mnemonic),
            MicroOp::Call => {
                let target = self.next16(bus);
                let ret = self.pc;
                self.push(bus, pc, (ret >> 8) as u8);
                self.push(bus, pc, ret as u8);
                self.load_pc(bus, target, PcSource::Call, micro.mnemonic);
                bus.trace_control(pc, micro.mnemonic);
                StepOutcome::Continue
            }
            MicroOp::Return => {
                let lo = self.pop(bus, pc);
                let hi = self.pop(bus, pc);
                self.load_pc(bus, u16::from_le_bytes([lo, hi]), PcSource::Return, micro.mnemonic);
                bus.trace_control(pc, micro.mnemonic);
                StepOutcome::Continue
            }
            MicroOp::WaitVBlank => { bus.trace_control(pc, micro.mnemonic); StepOutcome::WaitVBlank }
            MicroOp::Halt => { self.halted = true; bus.trace_control(pc, micro.mnemonic); StepOutcome::Halted }
        }
    }

    fn next8<B: Bus>(&mut self, bus: &mut B) -> u8 {
        self.mar = self.pc;
        self.mdr = bus.fetch8(self.mar);
        let increment = ripple_increment16(self.pc);
        self.pc = increment.after;
        bus.trace_pc_increment(increment);
        self.mdr
    }

    fn next16<B: Bus>(&mut self, bus: &mut B) -> u16 { u16::from_le_bytes([self.next8(bus), self.next8(bus)]) }
    fn next_reg<B: Bus>(&mut self, bus: &mut B) -> Option<Reg> { Reg::from_code(self.next8(bus)) }

    fn read_memory<B: Bus>(&mut self, bus: &mut B, pc: u16, address: u16) -> u8 {
        self.mar = address;
        self.mdr = bus.read8(pc, self.mar);
        self.mdr
    }

    fn write_memory<B: Bus>(&mut self, bus: &mut B, pc: u16, address: u16, value: u8) {
        self.mar = address;
        self.mdr = value;
        bus.write8(pc, self.mar, self.mdr);
    }

    fn load_pc<B: Bus>(&mut self, bus: &mut B, target: u16, source: PcSource, control: &'static str) {
        let before = self.pc;
        self.pc = target;
        bus.trace_pc_load(before, target, source, control);
    }

    fn write_reg<B: Bus>(&mut self, bus: &mut B, pc: u16, reg: Reg, value: u8, control: &'static str) {
        let slot = &mut self.regs[reg as usize];
        let before = *slot;
        *slot = value;
        bus.trace_register_write(pc, reg, before, value, control);
    }

    fn immediate_arithmetic<B: Bus>(&mut self, bus: &mut B, pc: u16, opcode: u8, op: AluOp, control: &'static str) -> StepOutcome {
        let Some(reg) = self.next_reg(bus) else { return self.fault(pc, opcode); };
        let rhs = self.next8(bus);
        let lhs = self.regs[reg as usize];
        let trace = match op {
            AluOp::Add => ripple_add(lhs, rhs, false, op),
            AluOp::Sub => ripple_sub(lhs, rhs, op),
            _ => unreachable!("immediate arithmetic only supports add/sub"),
        };
        self.commit_arithmetic(bus, pc, reg, trace, control);
        StepOutcome::Continue
    }

    fn immediate_logic<B: Bus>(&mut self, bus: &mut B, pc: u16, opcode: u8, op: AluOp, control: &'static str) -> StepOutcome {
        let Some(reg) = self.next_reg(bus) else { return self.fault(pc, opcode); };
        let rhs = self.next8(bus);
        let lhs = self.regs[reg as usize];
        let result = match op { AluOp::And => lhs & rhs, AluOp::Or => lhs | rhs, AluOp::Xor => lhs ^ rhs, _ => unreachable!("logic op") };
        let trace = logic_trace(op, lhs, rhs, result);
        self.write_reg(bus, pc, reg, result, control);
        self.flags = Flags { zero: result == 0, carry: false, less: false };
        bus.trace_alu_exact(pc, trace, control);
        StepOutcome::Continue
    }

    fn unary_arithmetic<B: Bus>(&mut self, bus: &mut B, pc: u16, opcode: u8, increment: bool) -> StepOutcome {
        let Some(reg) = self.next_reg(bus) else { return self.fault(pc, opcode); };
        let lhs = self.regs[reg as usize];
        let (trace, control) = if increment { (ripple_add(lhs, 1, false, AluOp::Add), "INC") } else { (ripple_sub(lhs, 1, AluOp::Sub), "DEC") };
        self.commit_arithmetic(bus, pc, reg, trace, control);
        StepOutcome::Continue
    }

    fn commit_arithmetic<B: Bus>(&mut self, bus: &mut B, pc: u16, reg: Reg, trace: AluTrace, control: &'static str) {
        self.write_reg(bus, pc, reg, trace.result, control);
        self.flags = Flags { zero: trace.result == 0, carry: trace.final_carry(), less: !trace.final_carry() && matches!(trace.op, AluOp::Sub) };
        bus.trace_alu_exact(pc, trace, control);
    }

    fn commit_compare<B: Bus>(&mut self, bus: &mut B, pc: u16, trace: AluTrace, control: &'static str) {
        self.flags = Flags { zero: trace.result == 0, carry: trace.final_carry(), less: !trace.final_carry() };
        bus.trace_alu_exact(pc, trace, control);
    }

    fn branch<B: Bus>(&mut self, bus: &mut B, pc: u16, condition: bool, control: &'static str) -> StepOutcome {
        let target = self.next16(bus);
        if condition { self.load_pc(bus, target, PcSource::Branch, control); }
        bus.trace_control(pc, control);
        StepOutcome::Continue
    }

    fn push<B: Bus>(&mut self, bus: &mut B, pc: u16, value: u8) {
        self.sp = ripple_decrement16(self.sp).after;
        self.write_memory(bus, pc, self.sp, value);
    }

    fn pop<B: Bus>(&mut self, bus: &mut B, pc: u16) -> u8 {
        let value = self.read_memory(bus, pc, self.sp);
        self.sp = ripple_increment16(self.sp).after;
        value
    }

    fn fault(&mut self, pc: u16, opcode: u8) -> StepOutcome { self.halted = true; StepOutcome::Fault { pc, opcode } }
}

#[must_use]
pub const fn mnemonic(value: u8) -> &'static str {
    match decode_microcode(value) { Some(instruction) => instruction.mnemonic, None => "FAULT" }
}

#[must_use]
pub const fn phase_for_opcode(value: u8) -> PhaseKind {
    match decode_microcode(value) {
        Some(instruction) if instruction.control.mem_read => PhaseKind::MemoryRead,
        Some(instruction) if instruction.control.mem_write => PhaseKind::MemoryWrite,
        Some(instruction) if instruction.control.alu_enable => PhaseKind::Alu,
        _ => PhaseKind::Decode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestBus {
        memory: Vec<u8>, exact_alu: Vec<AluTrace>, writes: Vec<(Reg, u8, u8)>,
        pc_increments: Vec<PcIncrementTrace>, pc_loads: Vec<(u16, u16, PcSource, &'static str)>,
    }
    impl Default for TestBus {
        fn default() -> Self { Self { memory: vec![0; 65_536], exact_alu: vec![], writes: vec![], pc_increments: vec![], pc_loads: vec![] } }
    }
    impl Bus for TestBus {
        fn fetch8(&mut self, pc: u16) -> u8 { self.memory[pc as usize] }
        fn read8(&mut self, _pc: u16, address: u16) -> u8 { self.memory[address as usize] }
        fn write8(&mut self, _pc: u16, address: u16, value: u8) { self.memory[address as usize] = value; }
        fn trace_decode(&mut self, _pc: u16, _opcode: u8, _mnemonic: &'static str) {}
        fn trace_alu(&mut self, _pc: u16, _value: u8, _control: &'static str) {}
        fn trace_control(&mut self, _pc: u16, _control: &'static str) {}
        fn trace_alu_exact(&mut self, _pc: u16, trace: AluTrace, _control: &'static str) { self.exact_alu.push(trace); }
        fn trace_register_write(&mut self, _pc: u16, reg: Reg, before: u8, after: u8, _control: &'static str) { self.writes.push((reg, before, after)); }
        fn trace_pc_increment(&mut self, trace: PcIncrementTrace) { self.pc_increments.push(trace); }
        fn trace_pc_load(&mut self, before: u16, after: u16, source: PcSource, control: &'static str) { self.pc_loads.push((before, after, source, control)); }
    }

    #[test]
    fn load_add_store_uses_exact_ripple_path() {
        let mut bus = TestBus::default();
        let program = [op::LDI, Reg::A.code(), 4, op::ADDI, Reg::A.code(), 6, op::ST, 0x80, 0, Reg::A.code(), op::HALT];
        bus.memory[..program.len()].copy_from_slice(&program);
        let mut cpu = Cpu::default();
        for _ in 0..4 { cpu.step(&mut bus); }
        assert_eq!(bus.memory[0x80], 10);
        assert_eq!(bus.exact_alu[1].result, 10);
        assert!(bus.writes.contains(&(Reg::A, 4, 10)));
        assert_eq!(cpu.mar(), 0x80);
        assert_eq!(cpu.mdr(), 10);
    }

    #[test]
    fn opcode_fetch_latches_mar_mdr_and_ir_semantically() {
        let mut bus = TestBus::default();
        bus.memory[0x0123] = op::NOP;
        let mut cpu = Cpu::default();
        cpu.pc = 0x0123;
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.ir(), op::NOP);
        assert_eq!(cpu.mar(), 0x0123);
        assert_eq!(cpu.mdr(), op::NOP);
        assert_eq!(cpu.pc(), 0x0124);
    }

    #[test]
    fn fetch_pc_advance_is_the_exact_ripple_incrementer() {
        let mut bus = TestBus::default();
        bus.memory[0x00FF] = op::NOP;
        let mut cpu = Cpu::default(); cpu.pc = 0x00FF;
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        let increment = bus.pc_increments.last().copied().expect("pc increment");
        assert_eq!(increment.after, 0x0100); assert!(increment.low_byte_carry()); assert_eq!(cpu.pc(), increment.after);
    }

    #[test]
    fn jump_and_branch_select_nonsequential_pc_mux_sources() {
        let mut bus = TestBus::default();
        let program = [op::JMP, 0x04, 0, op::HALT, op::JZ, 0x09, 0, op::HALT, op::HALT, op::HALT];
        bus.memory[..program.len()].copy_from_slice(&program);
        let mut cpu = Cpu::default();
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue); assert_eq!(cpu.pc(), 4); assert_eq!(bus.pc_loads[0].2, PcSource::Jump);
        cpu.flags.zero = true;
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue); assert_eq!(cpu.pc(), 9); assert_eq!(bus.pc_loads[1].2, PcSource::Branch);
    }

    #[test]
    fn undefined_opcode_faults_because_control_rom_has_no_entry() {
        let mut bus = TestBus::default(); bus.memory[0] = 0xAA; let mut cpu = Cpu::default();
        assert_eq!(cpu.step(&mut bus), StepOutcome::Fault { pc: 0, opcode: 0xAA });
    }

    #[test]
    fn call_and_return_move_stack_pointer_through_ripple_networks() {
        let mut bus = TestBus::default();
        let program = [op::CALL, 0x05, 0, op::HALT, op::HALT, op::RET];
        bus.memory[..program.len()].copy_from_slice(&program);
        let mut cpu = Cpu::default();
        let initial = cpu.sp();
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.sp(), initial.wrapping_sub(2)); assert_eq!(cpu.pc(), 5);
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.sp(), initial); assert_eq!(cpu.pc(), 3);
        assert_eq!(cpu.mar(), initial.wrapping_sub(1));
    }
}
