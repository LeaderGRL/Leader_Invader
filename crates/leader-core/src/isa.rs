use crate::logic::{
    logic_trace, ripple_add, ripple_decrement16, ripple_increment16, ripple_sub, AluOp, AluTrace,
    PcIncrementTrace,
};
use crate::microcode::{
    control_word_at, decode as decode_microcode, execute_address, uaddr, MicroAddressTransition,
    MicroOp, MicroSequencer,
};
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
pub enum Reg {
    A = 0,
    B = 1,
    C = 2,
    D = 3,
    X = 4,
    Y = 5,
    T = 6,
    U = 7,
}

impl Reg {
    pub const ALL: [Self; 8] = [Self::A, Self::B, Self::C, Self::D, Self::X, Self::Y, Self::T, Self::U];

    #[must_use]
    pub const fn code(self) -> u8 { self as u8 }

    #[must_use]
    pub const fn name(self) -> &'static str {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicroPhase { T0, T1, T2 }

impl MicroPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self { Self::T0 => "t0", Self::T1 => "t1", Self::T2 => "t2" }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicroCycleKind {
    FetchAddress,
    FetchData,
    DecodeLatch,
    OperandAddress,
    OperandData,
    OperandReady,
    MemoryAddress,
    MemoryRead,
    MemoryWriteData,
    MemoryWriteCommit,
}

impl MicroCycleKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FetchAddress => "fetch_address",
            Self::FetchData => "fetch_data",
            Self::DecodeLatch => "decode_latch",
            Self::OperandAddress => "operand_address",
            Self::OperandData => "operand_data",
            Self::OperandReady => "operand_ready",
            Self::MemoryAddress => "memory_address",
            Self::MemoryRead => "memory_read",
            Self::MemoryWriteData => "memory_write_data",
            Self::MemoryWriteCommit => "memory_write_commit",
        }
    }
}

pub trait Bus {
    fn fetch8(&mut self, pc: u16) -> u8;
    fn read8(&mut self, pc: u16, address: u16) -> u8;
    fn write8(&mut self, pc: u16, address: u16, value: u8);
    fn trace_decode(&mut self, pc: u16, opcode: u8, mnemonic: &'static str);
    fn trace_alu(&mut self, pc: u16, value: u8, control: &'static str);
    fn trace_control(&mut self, pc: u16, control: &'static str);

    fn trace_alu_exact(&mut self, pc: u16, trace: AluTrace, control: &'static str) {
        self.trace_alu(pc, trace.result, control);
    }

    fn trace_register_write(&mut self, _pc: u16, _reg: Reg, _before: u8, _after: u8, _control: &'static str) {}
    fn trace_pc_increment(&mut self, _trace: PcIncrementTrace) {}
    fn trace_pc_load(&mut self, _before: u16, _after: u16, _source: PcSource, _control: &'static str) {}
    fn trace_microaddress(&mut self, _transition: MicroAddressTransition, _opcode: u8, _control_bits: u8, _label: &'static str) {}

    fn trace_microcycle(
        &mut self,
        phase: MicroPhase,
        _kind: MicroCycleKind,
        pc: u16,
        _mar: u16,
        _mdr: u8,
        _ir: u8,
        _control: &'static str,
    ) {
        let timing = match phase {
            MicroPhase::T0 => "µT0",
            MicroPhase::T1 => "µT1",
            MicroPhase::T2 => "µT2",
        };
        self.trace_control(pc, timing);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cpu {
    regs: [u8; 8], pc: u16, sp: u16, mar: u16, mdr: u8, ir: u8, phase: MicroPhase,
    micro: MicroSequencer, flags: Flags, halted: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self {
            regs: [0; 8], pc: 0, sp: 0x7FFF, mar: 0, mdr: 0, ir: 0, phase: MicroPhase::T0,
            micro: MicroSequencer::default(), flags: Flags::default(), halted: false,
        }
    }
}

impl Cpu {
    #[must_use] pub const fn pc(&self) -> u16 { self.pc }
    #[must_use] pub const fn sp(&self) -> u16 { self.sp }
    #[must_use] pub const fn mar(&self) -> u16 { self.mar }
    #[must_use] pub const fn mdr(&self) -> u8 { self.mdr }
    #[must_use] pub const fn ir(&self) -> u8 { self.ir }
    #[must_use] pub const fn phase(&self) -> MicroPhase { self.phase }
    #[must_use] pub const fn micro_address(&self) -> u8 { self.micro.address() }
    #[must_use] pub const fn flags(&self) -> Flags { self.flags }
    #[must_use] pub fn reg(&self, reg: Reg) -> u8 { self.regs[reg as usize] }

    pub fn step<B: Bus>(&mut self, bus: &mut B) -> StepOutcome {
        if self.halted { return StepOutcome::Halted; }

        let instruction_pc = self.pc;
        let opcode = self.fetch_opcode(bus);
        let Some(micro) = decode_microcode(self.ir) else { return self.fault(instruction_pc, self.ir); };
        let Some(exec) = execute_address(self.ir) else { return self.fault(instruction_pc, self.ir); };
        let transition = self.micro.dispatch(exec);
        self.publish_microaddress(bus, transition, micro.mnemonic);
        bus.trace_decode(instruction_pc, self.ir, micro.mnemonic);

        match micro.operation {
            MicroOp::Nop => StepOutcome::Continue,
            MicroOp::LoadImmediate => {
                let Some(reg) = self.next_reg(bus) else { return self.fault(instruction_pc, opcode); };
                self.advance_execute(bus, "LDI_VALUE");
                let value = self.next8(bus);
                self.advance_execute(bus, "LDI_COMMIT");
                self.write_reg(bus, instruction_pc, reg, value, micro.mnemonic);
                self.flags = Flags { zero: value == 0, carry: false, less: false };
                bus.trace_alu_exact(instruction_pc, logic_trace(AluOp::Pass, value, 0, value), micro.mnemonic);
                StepOutcome::Continue
            }
            MicroOp::LoadMemory => {
                let Some(reg) = self.next_reg(bus) else { return self.fault(instruction_pc, opcode); };
                let address = self.next16(bus);
                let value = self.read_memory(bus, instruction_pc, address, "LD");
                self.write_reg(bus, instruction_pc, reg, value, micro.mnemonic);
                self.flags.zero = value == 0;
                self.flags.less = false;
                StepOutcome::Continue
            }
            MicroOp::StoreMemory => {
                let address = self.next16(bus);
                let Some(reg) = self.next_reg(bus) else { return self.fault(instruction_pc, opcode); };
                self.write_memory(bus, instruction_pc, address, self.regs[reg as usize], "ST");
                StepOutcome::Continue
            }
            MicroOp::Move => self.binary_register_alu(bus, instruction_pc, opcode, AluOp::Pass, micro.mnemonic, false),
            MicroOp::Add => self.binary_register_alu(bus, instruction_pc, opcode, AluOp::Add, micro.mnemonic, false),
            MicroOp::AddImmediate => self.immediate_arithmetic(bus, instruction_pc, opcode, AluOp::Add, micro.mnemonic),
            MicroOp::SubImmediate => self.immediate_arithmetic(bus, instruction_pc, opcode, AluOp::Sub, micro.mnemonic),
            MicroOp::AndImmediate => self.immediate_logic(bus, instruction_pc, opcode, AluOp::And, micro.mnemonic),
            MicroOp::OrImmediate => self.immediate_logic(bus, instruction_pc, opcode, AluOp::Or, micro.mnemonic),
            MicroOp::XorImmediate => self.immediate_logic(bus, instruction_pc, opcode, AluOp::Xor, micro.mnemonic),
            MicroOp::Increment => self.unary_arithmetic(bus, instruction_pc, opcode, true),
            MicroOp::Decrement => self.unary_arithmetic(bus, instruction_pc, opcode, false),
            MicroOp::Compare => self.binary_register_alu(bus, instruction_pc, opcode, AluOp::Compare, micro.mnemonic, true),
            MicroOp::CompareImmediate => self.compare_immediate(bus, instruction_pc, opcode, micro.mnemonic),
            MicroOp::Jump => {
                let target = self.next16(bus);
                self.load_pc(bus, target, PcSource::Jump, micro.mnemonic);
                bus.trace_control(instruction_pc, micro.mnemonic);
                StepOutcome::Continue
            }
            MicroOp::JumpZero => self.branch(bus, instruction_pc, self.flags.zero, micro.mnemonic),
            MicroOp::JumpNotZero => self.branch(bus, instruction_pc, !self.flags.zero, micro.mnemonic),
            MicroOp::JumpLess => self.branch(bus, instruction_pc, self.flags.less, micro.mnemonic),
            MicroOp::JumpGreaterEqual => self.branch(bus, instruction_pc, !self.flags.less, micro.mnemonic),
            MicroOp::JumpCarry => self.branch(bus, instruction_pc, self.flags.carry, micro.mnemonic),
            MicroOp::Call => {
                let target = self.next16(bus);
                let ret = self.pc;
                self.push(bus, instruction_pc, (ret >> 8) as u8);
                self.push(bus, instruction_pc, ret as u8);
                self.load_pc(bus, target, PcSource::Call, micro.mnemonic);
                bus.trace_control(instruction_pc, micro.mnemonic);
                StepOutcome::Continue
            }
            MicroOp::Return => {
                let lo = self.pop(bus, instruction_pc);
                let hi = self.pop(bus, instruction_pc);
                self.load_pc(bus, u16::from_le_bytes([lo, hi]), PcSource::Return, micro.mnemonic);
                bus.trace_control(instruction_pc, micro.mnemonic);
                StepOutcome::Continue
            }
            MicroOp::WaitVBlank => { bus.trace_control(instruction_pc, micro.mnemonic); StepOutcome::WaitVBlank }
            MicroOp::Halt => { self.halted = true; bus.trace_control(instruction_pc, micro.mnemonic); StepOutcome::Halted }
        }
    }

    fn fetch_opcode<B: Bus>(&mut self, bus: &mut B) -> u8 {
        let transition = self.micro.fetch_start();
        self.publish_microaddress(bus, transition, "FETCH_ADDR");
        self.mar = self.pc;
        self.emit_cycle(bus, MicroPhase::T0, MicroCycleKind::FetchAddress, "FETCH_ADDR");

        let transition = self.micro.advance();
        self.publish_microaddress(bus, transition, "FETCH_DATA");
        self.mdr = bus.fetch8(self.mar);
        let increment = ripple_increment16(self.pc);
        self.pc = increment.after;
        bus.trace_pc_increment(increment);
        self.emit_cycle(bus, MicroPhase::T1, MicroCycleKind::FetchData, "FETCH_DATA");

        let transition = self.micro.advance();
        self.publish_microaddress(bus, transition, "IR_LATCH");
        self.ir = self.mdr;
        self.emit_cycle(bus, MicroPhase::T2, MicroCycleKind::DecodeLatch, "IR_LATCH");
        self.ir
    }

    fn next8<B: Bus>(&mut self, bus: &mut B) -> u8 {
        let transition = self.micro.call(uaddr::OPERAND_T0);
        self.publish_microaddress(bus, transition, "OPERAND_ADDR");
        self.mar = self.pc;
        self.emit_cycle(bus, MicroPhase::T0, MicroCycleKind::OperandAddress, "OPERAND_ADDR");

        let transition = self.micro.advance();
        self.publish_microaddress(bus, transition, "OPERAND_DATA");
        self.mdr = bus.fetch8(self.mar);
        let increment = ripple_increment16(self.pc);
        self.pc = increment.after;
        bus.trace_pc_increment(increment);
        self.emit_cycle(bus, MicroPhase::T1, MicroCycleKind::OperandData, "OPERAND_DATA");

        let transition = self.micro.advance();
        self.publish_microaddress(bus, transition, "OPERAND_READY");
        self.emit_cycle(bus, MicroPhase::T2, MicroCycleKind::OperandReady, "OPERAND_READY");
        let value = self.mdr;

        let transition = self.micro.return_from_routine();
        self.publish_microaddress(bus, transition, "EXEC_RETURN");
        value
    }

    fn next16<B: Bus>(&mut self, bus: &mut B) -> u16 { u16::from_le_bytes([self.next8(bus), self.next8(bus)]) }
    fn next_reg<B: Bus>(&mut self, bus: &mut B) -> Option<Reg> { Reg::from_code(self.next8(bus)) }

    fn read_memory<B: Bus>(&mut self, bus: &mut B, pc: u16, address: u16, control: &'static str) -> u8 {
        let transition = self.micro.call(uaddr::READ_T0);
        self.publish_microaddress(bus, transition, control);
        self.mar = address;
        self.emit_cycle(bus, MicroPhase::T0, MicroCycleKind::MemoryAddress, control);

        let transition = self.micro.advance();
        self.publish_microaddress(bus, transition, control);
        self.mdr = bus.read8(pc, self.mar);
        self.emit_cycle(bus, MicroPhase::T1, MicroCycleKind::MemoryRead, control);

        let transition = self.micro.advance();
        self.publish_microaddress(bus, transition, control);
        self.emit_cycle(bus, MicroPhase::T2, MicroCycleKind::OperandReady, control);
        let value = self.mdr;

        let transition = self.micro.return_from_routine();
        self.publish_microaddress(bus, transition, "EXEC_RETURN");
        value
    }

    fn write_memory<B: Bus>(&mut self, bus: &mut B, pc: u16, address: u16, value: u8, control: &'static str) {
        let transition = self.micro.call(uaddr::WRITE_T0);
        self.publish_microaddress(bus, transition, control);
        self.mar = address;
        self.emit_cycle(bus, MicroPhase::T0, MicroCycleKind::MemoryAddress, control);

        let transition = self.micro.advance();
        self.publish_microaddress(bus, transition, control);
        self.mdr = value;
        self.emit_cycle(bus, MicroPhase::T1, MicroCycleKind::MemoryWriteData, control);

        let transition = self.micro.advance();
        self.publish_microaddress(bus, transition, control);
        bus.write8(pc, self.mar, self.mdr);
        self.emit_cycle(bus, MicroPhase::T2, MicroCycleKind::MemoryWriteCommit, control);

        let transition = self.micro.return_from_routine();
        self.publish_microaddress(bus, transition, "EXEC_RETURN");
    }

    fn advance_execute<B: Bus>(&mut self, bus: &mut B, label: &'static str) {
        let transition = self.micro.advance();
        self.publish_microaddress(bus, transition, label);
    }

    fn publish_microaddress<B: Bus>(&self, bus: &mut B, transition: MicroAddressTransition, label: &'static str) {
        let word = control_word_at(transition.after, self.ir);
        bus.trace_microaddress(transition, self.ir, word.bits(), label);
    }

    fn emit_cycle<B: Bus>(&mut self, bus: &mut B, phase: MicroPhase, kind: MicroCycleKind, control: &'static str) {
        self.phase = phase;
        bus.trace_microcycle(phase, kind, self.pc, self.mar, self.mdr, self.ir, control);
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

    fn binary_register_alu<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        opcode: u8,
        operation: AluOp,
        control: &'static str,
        compare: bool,
    ) -> StepOutcome {
        let Some(lhs_reg) = self.next_reg(bus) else { return self.fault(pc, opcode); };
        self.advance_execute(bus, "ALU_OPERAND_B");
        let Some(rhs_reg) = self.next_reg(bus) else { return self.fault(pc, opcode); };
        let lhs = self.regs[lhs_reg as usize];
        let rhs = self.regs[rhs_reg as usize];

        self.advance_execute(bus, "ALU_SELECT");
        self.advance_execute(bus, "ALU_PROPAGATE");
        let trace = match operation {
            AluOp::Pass => logic_trace(operation, rhs, 0, rhs),
            AluOp::Add => ripple_add(lhs, rhs, false, operation),
            AluOp::Compare => ripple_sub(lhs, rhs, operation),
            _ => unreachable!("binary register ALU operation"),
        };
        if compare {
            self.latch_compare_flags(trace);
        } else if operation == AluOp::Pass {
            self.flags = Flags { zero: trace.result == 0, carry: false, less: false };
        } else {
            self.latch_arithmetic_flags(trace);
        }
        bus.trace_alu_exact(pc, trace, control);

        self.advance_execute(bus, "ALU_COMMIT");
        if !compare {
            self.write_reg(bus, pc, lhs_reg, trace.result, control);
        }
        StepOutcome::Continue
    }

    fn immediate_arithmetic<B: Bus>(&mut self, bus: &mut B, pc: u16, opcode: u8, operation: AluOp, control: &'static str) -> StepOutcome {
        let Some(reg) = self.next_reg(bus) else { return self.fault(pc, opcode); };

        if opcode == op::ADDI {
            self.advance_execute(bus, "ADDI_VALUE");
            let rhs = self.next8(bus);
            self.advance_execute(bus, "ADDI_COMMIT");
            let trace = ripple_add(self.regs[reg as usize], rhs, false, operation);
            self.commit_arithmetic(bus, pc, reg, trace, control);
            return StepOutcome::Continue;
        }

        self.advance_execute(bus, "ALU_OPERAND_B");
        let rhs = self.next8(bus);
        let lhs = self.regs[reg as usize];
        self.advance_execute(bus, "ALU_SELECT");
        self.advance_execute(bus, "ALU_PROPAGATE");
        let trace = match operation {
            AluOp::Sub => ripple_sub(lhs, rhs, operation),
            _ => unreachable!("five-row immediate arithmetic only supports subtraction"),
        };
        self.latch_arithmetic_flags(trace);
        bus.trace_alu_exact(pc, trace, control);
        self.advance_execute(bus, "ALU_COMMIT");
        self.write_reg(bus, pc, reg, trace.result, control);
        StepOutcome::Continue
    }

    fn immediate_logic<B: Bus>(&mut self, bus: &mut B, pc: u16, opcode: u8, operation: AluOp, control: &'static str) -> StepOutcome {
        let Some(reg) = self.next_reg(bus) else { return self.fault(pc, opcode); };
        self.advance_execute(bus, "ALU_OPERAND_B");
        let rhs = self.next8(bus);
        let lhs = self.regs[reg as usize];
        self.advance_execute(bus, "ALU_SELECT");
        self.advance_execute(bus, "ALU_PROPAGATE");
        let result = match operation {
            AluOp::And => lhs & rhs,
            AluOp::Or => lhs | rhs,
            AluOp::Xor => lhs ^ rhs,
            _ => unreachable!("logic operation"),
        };
        let trace = logic_trace(operation, lhs, rhs, result);
        self.flags = Flags { zero: result == 0, carry: false, less: false };
        bus.trace_alu_exact(pc, trace, control);
        self.advance_execute(bus, "ALU_COMMIT");
        self.write_reg(bus, pc, reg, result, control);
        StepOutcome::Continue
    }

    fn unary_arithmetic<B: Bus>(&mut self, bus: &mut B, pc: u16, opcode: u8, increment: bool) -> StepOutcome {
        let Some(reg) = self.next_reg(bus) else { return self.fault(pc, opcode); };
        let lhs = self.regs[reg as usize];
        self.advance_execute(bus, "ALU_CONST_ONE");
        self.advance_execute(bus, "ALU_SELECT");
        self.advance_execute(bus, "ALU_PROPAGATE");
        let (trace, control) = if increment {
            (ripple_add(lhs, 1, false, AluOp::Add), "INC")
        } else {
            (ripple_sub(lhs, 1, AluOp::Sub), "DEC")
        };
        self.latch_arithmetic_flags(trace);
        bus.trace_alu_exact(pc, trace, control);
        self.advance_execute(bus, "ALU_COMMIT");
        self.write_reg(bus, pc, reg, trace.result, control);
        StepOutcome::Continue
    }

    fn compare_immediate<B: Bus>(&mut self, bus: &mut B, pc: u16, opcode: u8, control: &'static str) -> StepOutcome {
        let Some(reg) = self.next_reg(bus) else { return self.fault(pc, opcode); };
        self.advance_execute(bus, "ALU_OPERAND_B");
        let rhs = self.next8(bus);
        let lhs = self.regs[reg as usize];
        self.advance_execute(bus, "ALU_SELECT");
        self.advance_execute(bus, "ALU_PROPAGATE");
        let trace = ripple_sub(lhs, rhs, AluOp::Compare);
        self.latch_compare_flags(trace);
        bus.trace_alu_exact(pc, trace, control);
        self.advance_execute(bus, "ALU_COMMIT");
        StepOutcome::Continue
    }

    fn latch_arithmetic_flags(&mut self, trace: AluTrace) {
        self.flags = Flags {
            zero: trace.result == 0,
            carry: trace.final_carry(),
            less: !trace.final_carry() && matches!(trace.op, AluOp::Sub),
        };
    }

    fn latch_compare_flags(&mut self, trace: AluTrace) {
        self.flags = Flags { zero: trace.result == 0, carry: trace.final_carry(), less: !trace.final_carry() };
    }

    fn commit_arithmetic<B: Bus>(&mut self, bus: &mut B, pc: u16, reg: Reg, trace: AluTrace, control: &'static str) {
        self.latch_arithmetic_flags(trace);
        bus.trace_alu_exact(pc, trace, control);
        self.write_reg(bus, pc, reg, trace.result, control);
    }

    fn branch<B: Bus>(&mut self, bus: &mut B, pc: u16, condition: bool, control: &'static str) -> StepOutcome {
        let target = self.next16(bus);
        if condition { self.load_pc(bus, target, PcSource::Branch, control); }
        bus.trace_control(pc, control);
        StepOutcome::Continue
    }

    fn push<B: Bus>(&mut self, bus: &mut B, pc: u16, value: u8) {
        self.sp = ripple_decrement16(self.sp).after;
        self.write_memory(bus, pc, self.sp, value, "STACK_PUSH");
    }

    fn pop<B: Bus>(&mut self, bus: &mut B, pc: u16) -> u8 {
        let value = self.read_memory(bus, pc, self.sp, "STACK_POP");
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
        cycles: Vec<(MicroPhase, MicroCycleKind, u16, u16, u8, u8)>,
        micro_addresses: Vec<MicroAddressTransition>,
    }

    impl Default for TestBus {
        fn default() -> Self {
            Self { memory: vec![0; 65_536], exact_alu: vec![], writes: vec![], pc_increments: vec![], pc_loads: vec![], cycles: vec![], micro_addresses: vec![] }
        }
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
        fn trace_microaddress(&mut self, transition: MicroAddressTransition, _opcode: u8, _control_bits: u8, _label: &'static str) { self.micro_addresses.push(transition); }
        fn trace_microcycle(&mut self, phase: MicroPhase, kind: MicroCycleKind, pc: u16, mar: u16, mdr: u8, ir: u8, _control: &'static str) {
            self.cycles.push((phase, kind, pc, mar, mdr, ir));
        }
    }

    fn assert_five_execute_rows(cpu: &Cpu, bus: &TestBus, opcode: u8) {
        let base = execute_address(opcode).unwrap();
        for step in 0..5 {
            assert!(bus.micro_addresses.iter().any(|t| t.after == base + step), "missing opcode {opcode:02X} execute row {step}");
        }
        assert_eq!(cpu.micro_address(), base + 4);
    }

    #[test]
    fn opcode_fetch_is_native_t0_t1_t2_and_dispatches_micro_pc() {
        let mut bus = TestBus::default();
        bus.memory[0x0123] = op::NOP;
        let mut cpu = Cpu::default();
        cpu.pc = 0x0123;
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(bus.cycles.len(), 3);
        assert_eq!(bus.cycles[0].1, MicroCycleKind::FetchAddress);
        assert_eq!(bus.cycles[1].1, MicroCycleKind::FetchData);
        assert_eq!(bus.cycles[2].1, MicroCycleKind::DecodeLatch);
        assert_eq!(cpu.micro_address(), execute_address(op::NOP).unwrap());
    }

    #[test]
    fn ldi_traverses_three_execute_rows() {
        let mut bus = TestBus::default();
        let program = [op::LDI, Reg::A.code(), 4, op::HALT];
        bus.memory[..program.len()].copy_from_slice(&program);
        let mut cpu = Cpu::default();
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        let base = execute_address(op::LDI).unwrap();
        assert!(bus.micro_addresses.iter().any(|t| t.after == base));
        assert!(bus.micro_addresses.iter().any(|t| t.after == base + 1 && t.source == crate::microcode::MicroAddressSource::Sequential));
        assert!(bus.micro_addresses.iter().any(|t| t.after == base + 2 && t.source == crate::microcode::MicroAddressSource::Sequential));
        assert_eq!(cpu.micro_address(), base + 2);
        assert_eq!(cpu.reg(Reg::A), 4);
    }

    #[test]
    fn addi_traverses_three_execute_rows_and_real_ripple_alu() {
        let mut bus = TestBus::default();
        let program = [op::LDI, Reg::A.code(), 4, op::ADDI, Reg::A.code(), 6, op::HALT];
        bus.memory[..program.len()].copy_from_slice(&program);
        let mut cpu = Cpu::default();
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        bus.micro_addresses.clear();
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        let base = execute_address(op::ADDI).unwrap();
        assert!(bus.micro_addresses.iter().any(|t| t.after == base));
        assert!(bus.micro_addresses.iter().any(|t| t.after == base + 1));
        assert!(bus.micro_addresses.iter().any(|t| t.after == base + 2));
        assert_eq!(cpu.micro_address(), base + 2);
        assert_eq!(cpu.reg(Reg::A), 10);
        assert_eq!(bus.exact_alu.last().unwrap().result, 10);
    }

    #[test]
    fn register_alu_instructions_are_causal_five_row_programs() {
        for (opcode, lhs, rhs, expected, writes) in [
            (op::MOV, 0x12, 0xA5, 0xA5, true),
            (op::ADD, 7, 9, 16, true),
            (op::CMP, 7, 9, 0xFE, false),
        ] {
            let mut bus = TestBus::default();
            bus.memory[..3].copy_from_slice(&[opcode, Reg::A.code(), Reg::B.code()]);
            let mut cpu = Cpu::default();
            cpu.regs[Reg::A as usize] = lhs;
            cpu.regs[Reg::B as usize] = rhs;
            assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
            assert_five_execute_rows(&cpu, &bus, opcode);
            assert_eq!(bus.exact_alu.last().unwrap().result, expected);
            assert_eq!(bus.writes.iter().any(|(reg, _, after)| *reg == Reg::A && *after == expected), writes);
        }
    }

    #[test]
    fn immediate_alu_instructions_are_causal_five_row_programs() {
        for (opcode, lhs, rhs, expected) in [
            (op::SUBI, 9, 4, 5),
            (op::ANDI, 0b1100, 0b1010, 0b1000),
            (op::ORI, 0b1100, 0b0011, 0b1111),
            (op::XORI, 0b1100, 0b1010, 0b0110),
        ] {
            let mut bus = TestBus::default();
            bus.memory[..3].copy_from_slice(&[opcode, Reg::A.code(), rhs]);
            let mut cpu = Cpu::default();
            cpu.regs[Reg::A as usize] = lhs;
            assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
            assert_five_execute_rows(&cpu, &bus, opcode);
            assert_eq!(cpu.reg(Reg::A), expected);
            assert_eq!(bus.exact_alu.last().unwrap().result, expected);
        }
    }

    #[test]
    fn unary_and_compare_immediate_use_all_five_rows() {
        for (opcode, before, expected) in [(op::INC, 4, 5), (op::DEC, 4, 3)] {
            let mut bus = TestBus::default();
            bus.memory[..2].copy_from_slice(&[opcode, Reg::A.code()]);
            let mut cpu = Cpu::default();
            cpu.regs[Reg::A as usize] = before;
            assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
            assert_five_execute_rows(&cpu, &bus, opcode);
            assert_eq!(cpu.reg(Reg::A), expected);
        }

        let mut bus = TestBus::default();
        bus.memory[..3].copy_from_slice(&[op::CMPI, Reg::A.code(), 9]);
        let mut cpu = Cpu::default();
        cpu.regs[Reg::A as usize] = 7;
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_five_execute_rows(&cpu, &bus, op::CMPI);
        assert!(cpu.flags().less);
        assert!(!cpu.flags().zero);
        assert!(bus.writes.is_empty());
    }

    #[test]
    fn load_add_store_uses_exact_ripple_path() {
        let mut bus = TestBus::default();
        let program = [op::LDI, Reg::A.code(), 4, op::ADDI, Reg::A.code(), 6, op::ST, 0x80, 0, Reg::A.code(), op::HALT];
        bus.memory[..program.len()].copy_from_slice(&program);
        let mut cpu = Cpu::default();
        for _ in 0..3 { assert_eq!(cpu.step(&mut bus), StepOutcome::Continue); }
        assert_eq!(bus.memory[0x80], 10);
        assert_eq!(bus.exact_alu[1].result, 10);
        assert!(bus.writes.contains(&(Reg::A, 4, 10)));
        assert_eq!(cpu.step(&mut bus), StepOutcome::Halted);
    }

    #[test]
    fn fetch_pc_advance_is_the_exact_ripple_incrementer() {
        let mut bus = TestBus::default();
        bus.memory[0x00FF] = op::NOP;
        let mut cpu = Cpu::default(); cpu.pc = 0x00FF;
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        let increment = bus.pc_increments.last().copied().expect("pc increment");
        assert_eq!(increment.after, 0x0100); assert!(increment.low_byte_carry());
    }

    #[test]
    fn jump_and_branch_select_nonsequential_pc_mux_sources() {
        let mut bus = TestBus::default();
        let program = [op::JMP, 0x04, 0, op::HALT, op::JZ, 0x09, 0, op::HALT, op::HALT, op::HALT];
        bus.memory[..program.len()].copy_from_slice(&program);
        let mut cpu = Cpu::default();
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue); assert_eq!(cpu.pc(), 4);
        cpu.flags.zero = true;
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue); assert_eq!(cpu.pc(), 9);
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
    }
}
