use crate::logic::{
    logic_trace, ripple_add, ripple_decrement16, ripple_increment16, ripple_sub, AluOp,
    AluTrace, PcIncrementTrace,
};
use crate::microcode::{
    control_word_at, decode as decode_microcode, execute_address, internal, uaddr, ControlWord,
    MicroAddressTransition, MicroOp, MicroSequencer,
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
    pub const ALL: [Self; 8] = [
        Self::A,
        Self::B,
        Self::C,
        Self::D,
        Self::X,
        Self::Y,
        Self::T,
        Self::U,
    ];

    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::X => "X",
            Self::Y => "Y",
            Self::T => "T",
            Self::U => "U",
        }
    }

    fn from_code(value: u8) -> Option<Self> {
        Self::ALL.get(value as usize).copied()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags {
    pub zero: bool,
    pub carry: bool,
    pub less: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    Continue,
    WaitVBlank,
    Halted,
    Fault { pc: u16, opcode: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcSource {
    Jump,
    Branch,
    Call,
    Return,
}

impl PcSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Jump => "jump",
            Self::Branch => "branch",
            Self::Call => "call",
            Self::Return => "return",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicroPhase {
    T0,
    T1,
    T2,
}

impl MicroPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::T0 => "t0",
            Self::T1 => "t1",
            Self::T2 => "t2",
        }
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

    fn trace_register_write(
        &mut self,
        _pc: u16,
        _reg: Reg,
        _before: u8,
        _after: u8,
        _control: &'static str,
    ) {
    }

    fn trace_pc_increment(&mut self, _trace: PcIncrementTrace) {}

    fn trace_pc_load(
        &mut self,
        _before: u16,
        _after: u16,
        _source: PcSource,
        _control: &'static str,
    ) {
    }

    fn trace_microaddress(
        &mut self,
        _transition: MicroAddressTransition,
        _opcode: u8,
        _control_bits: u8,
        _label: &'static str,
    ) {
    }

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
    regs: [u8; 8],
    pc: u16,
    sp: u16,
    mar: u16,
    mdr: u8,
    ir: u8,
    phase: MicroPhase,
    micro: MicroSequencer,
    alu_a: u8,
    alu_b: u8,
    alu_op: Option<AluOp>,
    flags: Flags,
    halted: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self {
            regs: [0; 8],
            pc: 0,
            sp: 0x7FFF,
            mar: 0,
            mdr: 0,
            ir: 0,
            phase: MicroPhase::T0,
            micro: MicroSequencer::default(),
            alu_a: 0,
            alu_b: 0,
            alu_op: None,
            flags: Flags::default(),
            halted: false,
        }
    }
}

impl Cpu {
    #[must_use]
    pub const fn pc(&self) -> u16 {
        self.pc
    }

    #[must_use]
    pub const fn sp(&self) -> u16 {
        self.sp
    }

    #[must_use]
    pub const fn mar(&self) -> u16 {
        self.mar
    }

    #[must_use]
    pub const fn mdr(&self) -> u8 {
        self.mdr
    }

    #[must_use]
    pub const fn ir(&self) -> u8 {
        self.ir
    }

    #[must_use]
    pub const fn phase(&self) -> MicroPhase {
        self.phase
    }

    #[must_use]
    pub const fn micro_address(&self) -> u8 {
        self.micro.address()
    }

    #[must_use]
    pub const fn flags(&self) -> Flags {
        self.flags
    }

    #[must_use]
    pub fn reg(&self, reg: Reg) -> u8 {
        self.regs[reg as usize]
    }

    pub fn step<B: Bus>(&mut self, bus: &mut B) -> StepOutcome {
        if self.halted {
            return StepOutcome::Halted;
        }

        let instruction_pc = self.pc;
        let opcode = self.fetch_opcode(bus);
        let Some(micro) = decode_microcode(self.ir) else {
            return self.fault(instruction_pc, self.ir);
        };
        let Some(exec) = execute_address(self.ir) else {
            return self.fault(instruction_pc, self.ir);
        };
        let transition = self.micro.dispatch(exec);
        self.publish_microaddress(bus, transition, micro.mnemonic);
        bus.trace_decode(instruction_pc, self.ir, micro.mnemonic);

        match micro.operation {
            MicroOp::Nop => StepOutcome::Continue,
            MicroOp::LoadImmediate => {
                let Some(reg) = self.next_reg(bus) else {
                    return self.fault(instruction_pc, opcode);
                };
                self.advance_execute(bus, "LDI_VALUE");
                let value = self.next8(bus);
                self.advance_execute(bus, "LDI_COMMIT");
                self.write_reg(bus, instruction_pc, reg, value, micro.mnemonic);
                if self.active_control().alu_enable {
                    self.latch_flags(Flags {
                        zero: value == 0,
                        carry: false,
                        less: false,
                    });
                    bus.trace_alu_exact(
                        instruction_pc,
                        logic_trace(AluOp::Pass, value, 0, value),
                        micro.mnemonic,
                    );
                }
                StepOutcome::Continue
            }
            MicroOp::LoadMemory => {
                self.load_memory(bus, instruction_pc, opcode, micro.mnemonic)
            }
            MicroOp::StoreMemory => self.store_memory(bus, instruction_pc, opcode),
            MicroOp::Move => self.binary_register_alu(
                bus,
                instruction_pc,
                opcode,
                AluOp::Pass,
                micro.mnemonic,
                false,
            ),
            MicroOp::Add => self.binary_register_alu(
                bus,
                instruction_pc,
                opcode,
                AluOp::Add,
                micro.mnemonic,
                false,
            ),
            MicroOp::AddImmediate => self.immediate_arithmetic(
                bus,
                instruction_pc,
                opcode,
                AluOp::Add,
                micro.mnemonic,
            ),
            MicroOp::SubImmediate => self.immediate_arithmetic(
                bus,
                instruction_pc,
                opcode,
                AluOp::Sub,
                micro.mnemonic,
            ),
            MicroOp::AndImmediate => self.immediate_logic(
                bus,
                instruction_pc,
                opcode,
                AluOp::And,
                micro.mnemonic,
            ),
            MicroOp::OrImmediate => self.immediate_logic(
                bus,
                instruction_pc,
                opcode,
                AluOp::Or,
                micro.mnemonic,
            ),
            MicroOp::XorImmediate => self.immediate_logic(
                bus,
                instruction_pc,
                opcode,
                AluOp::Xor,
                micro.mnemonic,
            ),
            MicroOp::Increment => self.unary_arithmetic(bus, instruction_pc, opcode, true),
            MicroOp::Decrement => self.unary_arithmetic(bus, instruction_pc, opcode, false),
            MicroOp::Compare => self.binary_register_alu(
                bus,
                instruction_pc,
                opcode,
                AluOp::Compare,
                micro.mnemonic,
                true,
            ),
            MicroOp::CompareImmediate => {
                self.compare_immediate(bus, instruction_pc, opcode, micro.mnemonic)
            }
            MicroOp::Jump => {
                self.control_transfer(bus, instruction_pc, true, PcSource::Jump, micro.mnemonic)
            }
            MicroOp::JumpZero => self.control_transfer(
                bus,
                instruction_pc,
                self.flags.zero,
                PcSource::Branch,
                micro.mnemonic,
            ),
            MicroOp::JumpNotZero => self.control_transfer(
                bus,
                instruction_pc,
                !self.flags.zero,
                PcSource::Branch,
                micro.mnemonic,
            ),
            MicroOp::JumpLess => self.control_transfer(
                bus,
                instruction_pc,
                self.flags.less,
                PcSource::Branch,
                micro.mnemonic,
            ),
            MicroOp::JumpGreaterEqual => self.control_transfer(
                bus,
                instruction_pc,
                !self.flags.less,
                PcSource::Branch,
                micro.mnemonic,
            ),
            MicroOp::JumpCarry => self.control_transfer(
                bus,
                instruction_pc,
                self.flags.carry,
                PcSource::Branch,
                micro.mnemonic,
            ),
            MicroOp::Call => self.call_subroutine(bus, instruction_pc, micro.mnemonic),
            MicroOp::Return => self.return_subroutine(bus, instruction_pc, micro.mnemonic),
            MicroOp::WaitVBlank => {
                bus.trace_control(instruction_pc, micro.mnemonic);
                StepOutcome::WaitVBlank
            }
            MicroOp::Halt => {
                self.halted = true;
                bus.trace_control(instruction_pc, micro.mnemonic);
                StepOutcome::Halted
            }
        }
    }

    fn active_control(&self) -> ControlWord {
        control_word_at(self.micro.address(), self.ir)
    }

    fn internal_enabled(&self, signal: u16) -> bool {
        self.active_control().has_internal(signal)
    }

    fn latch_mar(&mut self, value: u16) {
        if self.internal_enabled(internal::MAR_LOAD) {
            self.mar = value;
        }
    }

    fn latch_mdr(&mut self, value: u8) {
        if self.internal_enabled(internal::MDR_LOAD) {
            self.mdr = value;
        }
    }

    fn latch_ir(&mut self) {
        if self.internal_enabled(internal::IR_LOAD) {
            self.ir = self.mdr;
        }
    }

    fn latch_operand_a(&mut self, value: u8) {
        if self.internal_enabled(internal::OPERAND_A_LOAD) {
            self.alu_a = value;
        }
    }

    fn latch_operand_b(&mut self, value: u8) {
        if self.internal_enabled(internal::OPERAND_B_LOAD) {
            self.alu_b = value;
        }
    }

    fn latch_alu_op(&mut self, operation: AluOp) {
        if self.internal_enabled(internal::ALU_OP_LOAD) {
            self.alu_op = Some(operation);
        }
    }

    fn latched_alu_trace(&self) -> Option<AluTrace> {
        let operation = self.alu_op?;
        Some(match operation {
            AluOp::Pass => logic_trace(operation, self.alu_b, 0, self.alu_b),
            AluOp::Add => ripple_add(self.alu_a, self.alu_b, false, operation),
            AluOp::Sub | AluOp::Compare => ripple_sub(self.alu_a, self.alu_b, operation),
            AluOp::And => logic_trace(
                operation,
                self.alu_a,
                self.alu_b,
                self.alu_a & self.alu_b,
            ),
            AluOp::Or => logic_trace(
                operation,
                self.alu_a,
                self.alu_b,
                self.alu_a | self.alu_b,
            ),
            AluOp::Xor => logic_trace(
                operation,
                self.alu_a,
                self.alu_b,
                self.alu_a ^ self.alu_b,
            ),
        })
    }

    fn propagate_latched_alu<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        control: &'static str,
        compare: bool,
    ) -> Option<AluTrace> {
        if !self.active_control().alu_enable {
            return None;
        }
        let trace = self.latched_alu_trace()?;
        if compare {
            self.latch_compare_flags(trace);
        } else if trace.op == AluOp::Pass {
            self.latch_flags(Flags {
                zero: trace.result == 0,
                carry: false,
                less: false,
            });
        } else if matches!(trace.op, AluOp::And | AluOp::Or | AluOp::Xor) {
            self.latch_flags(Flags {
                zero: trace.result == 0,
                carry: false,
                less: false,
            });
        } else {
            self.latch_arithmetic_flags(trace);
        }
        bus.trace_alu_exact(pc, trace, control);
        Some(trace)
    }

    fn latch_flags(&mut self, value: Flags) {
        if self.internal_enabled(internal::FLAGS_LOAD) {
            self.flags = value;
        }
    }

    fn increment_pc_controlled<B: Bus>(&mut self, bus: &mut B) {
        if !self.internal_enabled(internal::PC_INC) {
            return;
        }
        let increment = ripple_increment16(self.pc);
        self.pc = increment.after;
        bus.trace_pc_increment(increment);
    }

    fn read_bus_enabled(&self) -> bool {
        let control = self.active_control();
        control.mem_read
            && control.has_internal(internal::MDR_LOAD)
            && control.has_internal(internal::BUS_DATA_ENABLE)
    }

    fn write_bus_commit_enabled(&self) -> bool {
        let control = self.active_control();
        control.mem_write
            && control.has_internal(internal::BUS_ADDRESS_ENABLE)
            && control.has_internal(internal::BUS_DATA_ENABLE)
            && control.has_internal(internal::ARCH_COMMIT)
    }

    fn fetch_opcode<B: Bus>(&mut self, bus: &mut B) -> u8 {
        let transition = self.micro.fetch_start();
        self.publish_microaddress(bus, transition, "FETCH_ADDR");
        self.latch_mar(self.pc);
        self.emit_cycle(
            bus,
            MicroPhase::T0,
            MicroCycleKind::FetchAddress,
            "FETCH_ADDR",
        );

        let transition = self.micro.advance();
        self.publish_microaddress(bus, transition, "FETCH_DATA");
        if self.read_bus_enabled() {
            let value = bus.fetch8(self.mar);
            self.latch_mdr(value);
        }
        self.increment_pc_controlled(bus);
        self.emit_cycle(
            bus,
            MicroPhase::T1,
            MicroCycleKind::FetchData,
            "FETCH_DATA",
        );

        let transition = self.micro.advance();
        self.publish_microaddress(bus, transition, "IR_LATCH");
        self.latch_ir();
        self.emit_cycle(
            bus,
            MicroPhase::T2,
            MicroCycleKind::DecodeLatch,
            "IR_LATCH",
        );
        self.ir
    }

    fn next8<B: Bus>(&mut self, bus: &mut B) -> u8 {
        let transition = self.micro.call(uaddr::OPERAND_T0);
        self.publish_microaddress(bus, transition, "OPERAND_ADDR");
        self.latch_mar(self.pc);
        self.emit_cycle(
            bus,
            MicroPhase::T0,
            MicroCycleKind::OperandAddress,
            "OPERAND_ADDR",
        );

        let transition = self.micro.advance();
        self.publish_microaddress(bus, transition, "OPERAND_DATA");
        if self.read_bus_enabled() {
            let value = bus.fetch8(self.mar);
            self.latch_mdr(value);
        }
        self.increment_pc_controlled(bus);
        self.emit_cycle(
            bus,
            MicroPhase::T1,
            MicroCycleKind::OperandData,
            "OPERAND_DATA",
        );

        let transition = self.micro.advance();
        self.publish_microaddress(bus, transition, "OPERAND_READY");
        self.emit_cycle(
            bus,
            MicroPhase::T2,
            MicroCycleKind::OperandReady,
            "OPERAND_READY",
        );
        let value = self.mdr;
        let transition = self.micro.return_from_routine();
        self.publish_microaddress(bus, transition, "EXEC_RETURN");
        value
    }

    fn next_reg<B: Bus>(&mut self, bus: &mut B) -> Option<Reg> {
        Reg::from_code(self.next8(bus))
    }

    fn read_memory<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        address: u16,
        control: &'static str,
    ) -> u8 {
        let transition = self.micro.call(uaddr::READ_T0);
        self.publish_microaddress(bus, transition, control);
        self.latch_mar(address);
        self.emit_cycle(bus, MicroPhase::T0, MicroCycleKind::MemoryAddress, control);

        let transition = self.micro.advance();
        self.publish_microaddress(bus, transition, control);
        if self.read_bus_enabled() {
            let value = bus.read8(pc, self.mar);
            self.latch_mdr(value);
        }
        self.emit_cycle(bus, MicroPhase::T1, MicroCycleKind::MemoryRead, control);

        let transition = self.micro.advance();
        self.publish_microaddress(bus, transition, control);
        self.emit_cycle(bus, MicroPhase::T2, MicroCycleKind::OperandReady, control);
        let value = self.mdr;
        let transition = self.micro.return_from_routine();
        self.publish_microaddress(bus, transition, "EXEC_RETURN");
        value
    }

    fn write_memory<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        address: u16,
        value: u8,
        control: &'static str,
    ) {
        let transition = self.micro.call(uaddr::WRITE_T0);
        self.publish_microaddress(bus, transition, control);
        self.latch_mar(address);
        self.emit_cycle(bus, MicroPhase::T0, MicroCycleKind::MemoryAddress, control);

        let transition = self.micro.advance();
        self.publish_microaddress(bus, transition, control);
        self.latch_mdr(value);
        self.emit_cycle(
            bus,
            MicroPhase::T1,
            MicroCycleKind::MemoryWriteData,
            control,
        );

        let transition = self.micro.advance();
        self.publish_microaddress(bus, transition, control);
        if self.write_bus_commit_enabled() {
            bus.write8(pc, self.mar, self.mdr);
        }
        self.emit_cycle(
            bus,
            MicroPhase::T2,
            MicroCycleKind::MemoryWriteCommit,
            control,
        );
        let transition = self.micro.return_from_routine();
        self.publish_microaddress(bus, transition, "EXEC_RETURN");
    }

    fn advance_execute<B: Bus>(&mut self, bus: &mut B, label: &'static str) {
        let transition = self.micro.advance();
        self.publish_microaddress(bus, transition, label);
    }

    fn publish_microaddress<B: Bus>(
        &self,
        bus: &mut B,
        transition: MicroAddressTransition,
        label: &'static str,
    ) {
        let word = control_word_at(transition.after, self.ir);
        bus.trace_microaddress(transition, self.ir, word.bits(), label);
    }

    fn emit_cycle<B: Bus>(
        &mut self,
        bus: &mut B,
        phase: MicroPhase,
        kind: MicroCycleKind,
        control: &'static str,
    ) {
        self.phase = phase;
        bus.trace_microcycle(
            phase,
            kind,
            self.pc,
            self.mar,
            self.mdr,
            self.ir,
            control,
        );
    }

    fn load_pc<B: Bus>(
        &mut self,
        bus: &mut B,
        target: u16,
        source: PcSource,
        control: &'static str,
    ) {
        if !self.active_control().pc_load {
            return;
        }
        let before = self.pc;
        self.pc = target;
        bus.trace_pc_load(before, target, source, control);
    }

    fn write_reg<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        reg: Reg,
        value: u8,
        control: &'static str,
    ) {
        if !self.active_control().reg_write {
            return;
        }
        let slot = &mut self.regs[reg as usize];
        let before = *slot;
        *slot = value;
        bus.trace_register_write(pc, reg, before, value, control);
    }

    fn load_memory<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        opcode: u8,
        control: &'static str,
    ) -> StepOutcome {
        let Some(reg) = self.next_reg(bus) else {
            return self.fault(pc, opcode);
        };
        self.advance_execute(bus, "LD_ADDR_LO");
        let lo = self.next8(bus);
        self.advance_execute(bus, "LD_ADDR_HI");
        let hi = self.next8(bus);
        let address = u16::from_le_bytes([lo, hi]);
        self.advance_execute(bus, "LD_READ");
        let value = self.read_memory(bus, pc, address, "LD");
        self.advance_execute(bus, "LD_COMMIT");
        self.write_reg(bus, pc, reg, value, control);
        self.latch_flags(Flags {
            zero: value == 0,
            carry: self.flags.carry,
            less: false,
        });
        StepOutcome::Continue
    }

    fn store_memory<B: Bus>(&mut self, bus: &mut B, pc: u16, opcode: u8) -> StepOutcome {
        let lo = self.next8(bus);
        self.advance_execute(bus, "ST_ADDR_HI");
        let hi = self.next8(bus);
        let address = u16::from_le_bytes([lo, hi]);
        self.advance_execute(bus, "ST_SOURCE");
        let Some(reg) = self.next_reg(bus) else {
            return self.fault(pc, opcode);
        };
        let value = self.regs[reg as usize];
        self.advance_execute(bus, "ST_WRITE");
        self.write_memory(bus, pc, address, value, "ST");
        self.advance_execute(bus, "ST_COMMIT");
        StepOutcome::Continue
    }

    fn control_transfer<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        condition: bool,
        source: PcSource,
        control: &'static str,
    ) -> StepOutcome {
        let lo = self.next8(bus);
        self.advance_execute(bus, "TARGET_HI");
        let hi = self.next8(bus);
        let target = u16::from_le_bytes([lo, hi]);
        self.advance_execute(bus, "BRANCH_CONDITION");
        self.advance_execute(bus, "PC_SELECT");
        self.advance_execute(bus, "PC_COMMIT");
        if condition {
            self.load_pc(bus, target, source, control);
        }
        bus.trace_control(pc, control);
        StepOutcome::Continue
    }

    fn call_subroutine<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        control: &'static str,
    ) -> StepOutcome {
        let lo = self.next8(bus);
        self.advance_execute(bus, "CALL_TARGET_HI");
        let hi = self.next8(bus);
        let target = u16::from_le_bytes([lo, hi]);
        let ret = self.pc;
        self.advance_execute(bus, "CALL_PUSH_HI");
        self.push(bus, pc, (ret >> 8) as u8);
        self.advance_execute(bus, "CALL_PUSH_LO");
        self.push(bus, pc, ret as u8);
        self.advance_execute(bus, "CALL_PC_COMMIT");
        self.load_pc(bus, target, PcSource::Call, control);
        bus.trace_control(pc, control);
        StepOutcome::Continue
    }

    fn return_subroutine<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        control: &'static str,
    ) -> StepOutcome {
        let lo = self.pop(bus, pc);
        self.advance_execute(bus, "RET_POP_HI");
        let hi = self.pop(bus, pc);
        let target = u16::from_le_bytes([lo, hi]);
        self.advance_execute(bus, "RET_TARGET");
        self.advance_execute(bus, "RET_PC_SELECT");
        self.advance_execute(bus, "RET_PC_COMMIT");
        self.load_pc(bus, target, PcSource::Return, control);
        bus.trace_control(pc, control);
        StepOutcome::Continue
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
        let Some(lhs_reg) = self.next_reg(bus) else {
            return self.fault(pc, opcode);
        };
        self.latch_operand_a(self.regs[lhs_reg as usize]);

        self.advance_execute(bus, "ALU_OPERAND_B");
        let Some(rhs_reg) = self.next_reg(bus) else {
            return self.fault(pc, opcode);
        };
        self.latch_operand_b(self.regs[rhs_reg as usize]);

        self.advance_execute(bus, "ALU_SELECT");
        self.latch_alu_op(operation);
        self.advance_execute(bus, "ALU_PROPAGATE");
        let Some(trace) = self.propagate_latched_alu(bus, pc, control, compare) else {
            return self.fault(pc, opcode);
        };

        self.advance_execute(bus, "ALU_COMMIT");
        if !compare {
            self.write_reg(bus, pc, lhs_reg, trace.result, control);
        }
        StepOutcome::Continue
    }

    fn immediate_arithmetic<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        opcode: u8,
        operation: AluOp,
        control: &'static str,
    ) -> StepOutcome {
        let Some(reg) = self.next_reg(bus) else {
            return self.fault(pc, opcode);
        };
        if opcode == op::ADDI {
            self.advance_execute(bus, "ADDI_VALUE");
            let rhs = self.next8(bus);
            self.advance_execute(bus, "ADDI_COMMIT");
            if !self.active_control().alu_enable {
                return self.fault(pc, opcode);
            }
            let trace = ripple_add(self.regs[reg as usize], rhs, false, operation);
            self.commit_arithmetic(bus, pc, reg, trace, control);
            return StepOutcome::Continue;
        }

        self.latch_operand_a(self.regs[reg as usize]);
        self.advance_execute(bus, "ALU_OPERAND_B");
        let rhs = self.next8(bus);
        self.latch_operand_b(rhs);
        self.advance_execute(bus, "ALU_SELECT");
        self.latch_alu_op(operation);
        self.advance_execute(bus, "ALU_PROPAGATE");
        let Some(trace) = self.propagate_latched_alu(bus, pc, control, false) else {
            return self.fault(pc, opcode);
        };
        self.advance_execute(bus, "ALU_COMMIT");
        self.write_reg(bus, pc, reg, trace.result, control);
        StepOutcome::Continue
    }

    fn immediate_logic<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        opcode: u8,
        operation: AluOp,
        control: &'static str,
    ) -> StepOutcome {
        let Some(reg) = self.next_reg(bus) else {
            return self.fault(pc, opcode);
        };
        self.latch_operand_a(self.regs[reg as usize]);
        self.advance_execute(bus, "ALU_OPERAND_B");
        let rhs = self.next8(bus);
        self.latch_operand_b(rhs);
        self.advance_execute(bus, "ALU_SELECT");
        self.latch_alu_op(operation);
        self.advance_execute(bus, "ALU_PROPAGATE");
        let Some(trace) = self.propagate_latched_alu(bus, pc, control, false) else {
            return self.fault(pc, opcode);
        };
        self.advance_execute(bus, "ALU_COMMIT");
        self.write_reg(bus, pc, reg, trace.result, control);
        StepOutcome::Continue
    }

    fn unary_arithmetic<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        opcode: u8,
        increment: bool,
    ) -> StepOutcome {
        let Some(reg) = self.next_reg(bus) else {
            return self.fault(pc, opcode);
        };
        self.latch_operand_a(self.regs[reg as usize]);

        self.advance_execute(bus, "ALU_CONST_ONE");
        self.latch_operand_b(1);
        self.advance_execute(bus, "ALU_SELECT");
        let operation = if increment { AluOp::Add } else { AluOp::Sub };
        self.latch_alu_op(operation);
        self.advance_execute(bus, "ALU_PROPAGATE");
        let control = if increment { "INC" } else { "DEC" };
        let Some(trace) = self.propagate_latched_alu(bus, pc, control, false) else {
            return self.fault(pc, opcode);
        };
        self.advance_execute(bus, "ALU_COMMIT");
        self.write_reg(bus, pc, reg, trace.result, control);
        StepOutcome::Continue
    }

    fn compare_immediate<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        opcode: u8,
        control: &'static str,
    ) -> StepOutcome {
        let Some(reg) = self.next_reg(bus) else {
            return self.fault(pc, opcode);
        };
        self.latch_operand_a(self.regs[reg as usize]);
        self.advance_execute(bus, "ALU_OPERAND_B");
        let rhs = self.next8(bus);
        self.latch_operand_b(rhs);
        self.advance_execute(bus, "ALU_SELECT");
        self.latch_alu_op(AluOp::Compare);
        self.advance_execute(bus, "ALU_PROPAGATE");
        let Some(_trace) = self.propagate_latched_alu(bus, pc, control, true) else {
            return self.fault(pc, opcode);
        };
        self.advance_execute(bus, "ALU_COMMIT");
        StepOutcome::Continue
    }

    fn latch_arithmetic_flags(&mut self, trace: AluTrace) {
        self.latch_flags(Flags {
            zero: trace.result == 0,
            carry: trace.final_carry(),
            less: !trace.final_carry() && matches!(trace.op, AluOp::Sub),
        });
    }

    fn latch_compare_flags(&mut self, trace: AluTrace) {
        self.latch_flags(Flags {
            zero: trace.result == 0,
            carry: trace.final_carry(),
            less: !trace.final_carry(),
        });
    }

    fn commit_arithmetic<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        reg: Reg,
        trace: AluTrace,
        control: &'static str,
    ) {
        self.latch_arithmetic_flags(trace);
        bus.trace_alu_exact(pc, trace, control);
        self.write_reg(bus, pc, reg, trace.result, control);
    }

    fn push<B: Bus>(&mut self, bus: &mut B, pc: u16, value: u8) {
        if !self.active_control().stack_enable {
            return;
        }
        self.sp = ripple_decrement16(self.sp).after;
        self.write_memory(bus, pc, self.sp, value, "STACK_PUSH");
    }

    fn pop<B: Bus>(&mut self, bus: &mut B, pc: u16) -> u8 {
        if !self.active_control().stack_enable {
            return 0;
        }
        let value = self.read_memory(bus, pc, self.sp, "STACK_POP");
        self.sp = ripple_increment16(self.sp).after;
        value
    }

    fn fault(&mut self, pc: u16, opcode: u8) -> StepOutcome {
        self.halted = true;
        StepOutcome::Fault { pc, opcode }
    }
}

#[must_use]
pub const fn mnemonic(value: u8) -> &'static str {
    match decode_microcode(value) {
        Some(instruction) => instruction.mnemonic,
        None => "FAULT",
    }
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
        memory: Vec<u8>,
        exact_alu: Vec<AluTrace>,
        writes: Vec<(Reg, u8, u8)>,
        pc_increments: Vec<PcIncrementTrace>,
        pc_loads: Vec<(u16, u16, PcSource, &'static str)>,
        cycles: Vec<(MicroPhase, MicroCycleKind, u16, u16, u8, u8)>,
        micro_addresses: Vec<MicroAddressTransition>,
    }

    impl Default for TestBus {
        fn default() -> Self {
            Self {
                memory: vec![0; 65536],
                exact_alu: vec![],
                writes: vec![],
                pc_increments: vec![],
                pc_loads: vec![],
                cycles: vec![],
                micro_addresses: vec![],
            }
        }
    }

    impl Bus for TestBus {
        fn fetch8(&mut self, pc: u16) -> u8 {
            self.memory[pc as usize]
        }

        fn read8(&mut self, _pc: u16, address: u16) -> u8 {
            self.memory[address as usize]
        }

        fn write8(&mut self, _pc: u16, address: u16, value: u8) {
            self.memory[address as usize] = value;
        }

        fn trace_decode(&mut self, _pc: u16, _opcode: u8, _mnemonic: &'static str) {}

        fn trace_alu(&mut self, _pc: u16, _value: u8, _control: &'static str) {}

        fn trace_control(&mut self, _pc: u16, _control: &'static str) {}

        fn trace_alu_exact(&mut self, _pc: u16, trace: AluTrace, _control: &'static str) {
            self.exact_alu.push(trace);
        }

        fn trace_register_write(
            &mut self,
            _pc: u16,
            reg: Reg,
            before: u8,
            after: u8,
            _control: &'static str,
        ) {
            self.writes.push((reg, before, after));
        }

        fn trace_pc_increment(&mut self, trace: PcIncrementTrace) {
            self.pc_increments.push(trace);
        }

        fn trace_pc_load(
            &mut self,
            before: u16,
            after: u16,
            source: PcSource,
            control: &'static str,
        ) {
            self.pc_loads.push((before, after, source, control));
        }

        fn trace_microaddress(
            &mut self,
            transition: MicroAddressTransition,
            _opcode: u8,
            _bits: u8,
            _label: &'static str,
        ) {
            self.micro_addresses.push(transition);
        }

        fn trace_microcycle(
            &mut self,
            phase: MicroPhase,
            kind: MicroCycleKind,
            pc: u16,
            mar: u16,
            mdr: u8,
            ir: u8,
            _control: &'static str,
        ) {
            self.cycles.push((phase, kind, pc, mar, mdr, ir));
        }
    }

    fn assert_five_execute_rows(cpu: &Cpu, bus: &TestBus, opcode: u8) {
        let base = execute_address(opcode).unwrap();
        for step in 0..5 {
            assert!(
                bus.micro_addresses
                    .iter()
                    .any(|transition| transition.after == base + step),
                "missing opcode {opcode:02X} execute row {step}"
            );
        }
        assert_eq!(cpu.micro_address(), base + 4);
    }

    #[test]
    fn internal_fetch_enables_gate_latches_and_pc_increment() {
        let mut bus = TestBus::default();
        let mut cpu = Cpu::default();
        cpu.ir = op::NOP;
        cpu.pc = 0x00FF;
        cpu.mar = 0xAAAA;
        cpu.mdr = 0x11;
        cpu.micro.dispatch(uaddr::FETCH_T1);
        cpu.latch_mar(0x00FF);
        assert_eq!(cpu.mar, 0xAAAA);
        cpu.micro.dispatch(uaddr::FETCH_T0);
        cpu.latch_mar(0x00FF);
        assert_eq!(cpu.mar, 0x00FF);
        cpu.latch_mdr(0x22);
        assert_eq!(cpu.mdr, 0x11);
        cpu.micro.dispatch(uaddr::FETCH_T1);
        cpu.latch_mdr(0x22);
        assert_eq!(cpu.mdr, 0x22);
        cpu.ir = op::NOP;
        cpu.latch_ir();
        assert_eq!(cpu.ir, op::NOP);
        cpu.micro.dispatch(uaddr::FETCH_T2);
        cpu.latch_ir();
        assert_eq!(cpu.ir, 0x22);
        cpu.ir = op::NOP;
        cpu.micro.dispatch(uaddr::FETCH_T0);
        cpu.increment_pc_controlled(&mut bus);
        assert_eq!(cpu.pc, 0x00FF);
        cpu.micro.dispatch(uaddr::FETCH_T1);
        cpu.increment_pc_controlled(&mut bus);
        assert_eq!(cpu.pc, 0x0100);
        assert!(bus.pc_increments.last().unwrap().low_byte_carry());
    }

    #[test]
    fn shared_routines_require_internal_bus_and_latch_enables() {
        let mut cpu = Cpu::default();
        cpu.ir = op::NOP;
        cpu.mar = 0x1111;
        cpu.mdr = 0x22;
        cpu.micro.dispatch(uaddr::OPERAND_T0);
        cpu.latch_mar(0x3333);
        assert_eq!(cpu.mar, 0x3333);
        assert!(!cpu.read_bus_enabled());
        cpu.micro.dispatch(uaddr::OPERAND_T1);
        assert!(cpu.read_bus_enabled());
        cpu.latch_mdr(0x44);
        assert_eq!(cpu.mdr, 0x44);
        cpu.micro.dispatch(uaddr::WRITE_T1);
        assert!(!cpu.write_bus_commit_enabled());
        cpu.micro.dispatch(uaddr::WRITE_T2);
        assert!(cpu.write_bus_commit_enabled());
    }

    #[test]
    fn flags_latch_requires_flags_load_on_selected_row() {
        let mut cpu = Cpu::default();
        cpu.ir = op::ADD;
        cpu.flags = Flags::default();
        let base = execute_address(op::ADD).unwrap();
        cpu.micro.dispatch(base + 2);
        cpu.latch_flags(Flags {
            zero: true,
            carry: true,
            less: true,
        });
        assert_eq!(cpu.flags, Flags::default());
        cpu.micro.dispatch(base + 3);
        cpu.latch_flags(Flags {
            zero: true,
            carry: true,
            less: false,
        });
        assert_eq!(
            cpu.flags,
            Flags {
                zero: true,
                carry: true,
                less: false
            }
        );
        cpu.ir = op::LDI;
        let ldi = execute_address(op::LDI).unwrap();
        cpu.flags = Flags::default();
        cpu.micro.dispatch(ldi + 2);
        cpu.latch_flags(Flags {
            zero: true,
            carry: false,
            less: false,
        });
        assert!(cpu.flags.zero);
    }

    #[test]
    fn alu_operand_and_operation_latches_require_their_physical_rows() {
        let mut cpu = Cpu::default();
        cpu.ir = op::ADD;
        let base = execute_address(op::ADD).unwrap();

        cpu.micro.dispatch(base + 1);
        cpu.latch_operand_a(0x12);
        assert_eq!(cpu.alu_a, 0);
        cpu.micro.dispatch(base);
        cpu.latch_operand_a(0x12);
        assert_eq!(cpu.alu_a, 0x12);

        cpu.micro.dispatch(base + 2);
        cpu.latch_operand_b(0x34);
        assert_eq!(cpu.alu_b, 0);
        cpu.micro.dispatch(base + 1);
        cpu.latch_operand_b(0x34);
        assert_eq!(cpu.alu_b, 0x34);

        cpu.micro.dispatch(base + 3);
        cpu.latch_alu_op(AluOp::Add);
        assert_eq!(cpu.alu_op, None);
        cpu.micro.dispatch(base + 2);
        cpu.latch_alu_op(AluOp::Add);
        assert_eq!(cpu.alu_op, Some(AluOp::Add));

        cpu.micro.dispatch(base + 3);
        let trace = cpu.latched_alu_trace().unwrap();
        assert_eq!(trace.lhs, 0x12);
        assert_eq!(trace.rhs, 0x34);
        assert_eq!(trace.result, 0x46);
    }

    #[test]
    fn selected_alu_operation_is_not_the_callers_local_operation_after_latching() {
        let mut cpu = Cpu::default();
        cpu.ir = op::ADD;
        let base = execute_address(op::ADD).unwrap();
        cpu.micro.dispatch(base);
        cpu.latch_operand_a(9);
        cpu.micro.dispatch(base + 1);
        cpu.latch_operand_b(4);
        cpu.micro.dispatch(base + 2);
        cpu.latch_alu_op(AluOp::Sub);
        cpu.micro.dispatch(base + 3);
        let stale = cpu.latched_alu_trace().unwrap();
        assert_eq!(stale.result, 5);

        cpu.micro.dispatch(base + 2);
        cpu.latch_alu_op(AluOp::Add);
        cpu.micro.dispatch(base + 3);
        let selected = cpu.latched_alu_trace().unwrap();
        assert_eq!(selected.result, 13);
    }

    #[test]
    fn control_rom_gates_register_pc_and_stack_effects() {
        let mut bus = TestBus::default();
        let mut cpu = Cpu::default();
        cpu.ir = op::ADD;
        cpu.regs[0] = 7;
        cpu.micro.dispatch(execute_address(op::ADD).unwrap() + 3);
        cpu.write_reg(&mut bus, 0, Reg::A, 9, "ADD");
        assert_eq!(cpu.reg(Reg::A), 7);
        cpu.micro.dispatch(execute_address(op::ADD).unwrap() + 4);
        cpu.write_reg(&mut bus, 0, Reg::A, 9, "ADD");
        assert_eq!(cpu.reg(Reg::A), 9);

        cpu.ir = op::JMP;
        cpu.pc = 3;
        cpu.micro.dispatch(execute_address(op::JMP).unwrap() + 3);
        cpu.load_pc(&mut bus, 9, PcSource::Jump, "JMP");
        assert_eq!(cpu.pc(), 3);
        cpu.micro.dispatch(execute_address(op::JMP).unwrap() + 4);
        cpu.load_pc(&mut bus, 9, PcSource::Jump, "JMP");
        assert_eq!(cpu.pc(), 9);

        cpu.ir = op::CALL;
        let initial = cpu.sp();
        cpu.micro.dispatch(execute_address(op::CALL).unwrap() + 1);
        cpu.push(&mut bus, 0, 0xAA);
        assert_eq!(cpu.sp(), initial);
        cpu.micro.dispatch(execute_address(op::CALL).unwrap() + 2);
        cpu.push(&mut bus, 0, 0xAA);
        assert_eq!(cpu.sp(), initial.wrapping_sub(1));
    }

    #[test]
    fn basic_fetch_and_existing_three_row_ops_still_work() {
        let mut bus = TestBus::default();
        bus.memory[..7].copy_from_slice(&[
            op::LDI,
            0,
            4,
            op::ADDI,
            0,
            6,
            op::HALT,
        ]);
        let mut cpu = Cpu::default();
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.reg(Reg::A), 4);
        bus.micro_addresses.clear();
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.reg(Reg::A), 10);
    }

    #[test]
    fn ld_and_st_are_causal_five_row_programs_with_shared_memory_routines() {
        let mut bus = TestBus::default();
        let program = [op::LD, 0, 0x80, 0, op::ST, 0x81, 0, 0, op::HALT];
        bus.memory[..program.len()].copy_from_slice(&program);
        bus.memory[0x80] = 0x5A;
        let mut cpu = Cpu::default();
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_five_execute_rows(&cpu, &bus, op::LD);
        assert_eq!(cpu.reg(Reg::A), 0x5A);
        bus.micro_addresses.clear();
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_five_execute_rows(&cpu, &bus, op::ST);
        assert_eq!(bus.memory[0x81], 0x5A);
    }

    #[test]
    fn branches_and_jump_are_causal_five_row_programs() {
        for (opcode, flag, taken) in [
            (op::JMP, false, true),
            (op::JZ, false, false),
            (op::JZ, true, true),
        ] {
            let mut bus = TestBus::default();
            bus.memory[..6].copy_from_slice(&[opcode, 5, 0, op::HALT, op::HALT, op::HALT]);
            let mut cpu = Cpu::default();
            cpu.flags.zero = flag;
            assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
            assert_five_execute_rows(&cpu, &bus, opcode);
            assert_eq!(cpu.pc(), if taken { 5 } else { 3 });
            assert_eq!(!bus.pc_loads.is_empty(), taken);
        }
    }

    #[test]
    fn call_and_ret_follow_five_causal_rows_and_shared_stack_routines() {
        let mut bus = TestBus::default();
        let program = [op::CALL, 5, 0, op::HALT, op::HALT, op::RET];
        bus.memory[..program.len()].copy_from_slice(&program);
        let mut cpu = Cpu::default();
        let initial = cpu.sp();
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_five_execute_rows(&cpu, &bus, op::CALL);
        assert_eq!(cpu.sp(), initial.wrapping_sub(2));
        assert_eq!(cpu.pc(), 5);
        bus.micro_addresses.clear();
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_five_execute_rows(&cpu, &bus, op::RET);
        assert_eq!(cpu.sp(), initial);
        assert_eq!(cpu.pc(), 3);
    }

    #[test]
    fn alu_five_row_family_remains_causal() {
        for (opcode, lhs, rhs, expected) in [
            (op::MOV, 0x12, 0xA5, 0xA5),
            (op::ADD, 7, 9, 16),
            (op::SUBI, 9, 4, 5),
            (op::ANDI, 12, 10, 8),
            (op::ORI, 12, 3, 15),
            (op::XORI, 12, 10, 6),
        ] {
            let mut bus = TestBus::default();
            let mut cpu = Cpu::default();
            cpu.regs[0] = lhs;
            if matches!(opcode, op::MOV | op::ADD) {
                cpu.regs[1] = rhs;
                bus.memory[..3].copy_from_slice(&[opcode, 0, 1]);
            } else {
                bus.memory[..3].copy_from_slice(&[opcode, 0, rhs]);
            }
            assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
            assert_five_execute_rows(&cpu, &bus, opcode);
            assert_eq!(cpu.reg(Reg::A), expected);
        }
    }

    #[test]
    fn compare_and_unary_five_row_family_remains_causal() {
        for (opcode, before, expected) in [(op::INC, 4, 5), (op::DEC, 4, 3)] {
            let mut bus = TestBus::default();
            bus.memory[..2].copy_from_slice(&[opcode, 0]);
            let mut cpu = Cpu::default();
            cpu.regs[0] = before;
            assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
            assert_five_execute_rows(&cpu, &bus, opcode);
            assert_eq!(cpu.reg(Reg::A), expected);
        }

        let mut bus = TestBus::default();
        bus.memory[..3].copy_from_slice(&[op::CMPI, 0, 9]);
        let mut cpu = Cpu::default();
        cpu.regs[0] = 7;
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_five_execute_rows(&cpu, &bus, op::CMPI);
        assert!(cpu.flags.less);
    }

    #[test]
    fn fetch_pc_advance_is_the_exact_ripple_incrementer() {
        let mut bus = TestBus::default();
        bus.memory[0xFF] = op::NOP;
        let mut cpu = Cpu::default();
        cpu.pc = 0xFF;
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        let increment = bus.pc_increments.last().unwrap();
        assert_eq!(increment.after, 0x100);
        assert!(increment.low_byte_carry());
    }

    #[test]
    fn undefined_opcode_faults_because_control_rom_has_no_entry() {
        let mut bus = TestBus::default();
        bus.memory[0] = 0xAA;
        let mut cpu = Cpu::default();
        assert_eq!(
            cpu.step(&mut bus),
            StepOutcome::Fault {
                pc: 0,
                opcode: 0xAA
            }
        );
    }
}
