use crate::logic::{
    logic_trace, ripple_add, ripple_increment16, ripple_sub, AluOp, AluTrace, PcIncrementTrace,
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

/// Physical source selected by the program-counter input mux.
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

    /// Called for every sequential byte fetch. The CPU state is already computed
    /// by this exact ripple trace; implementations may record it for diagnostics.
    fn trace_pc_increment(&mut self, _trace: PcIncrementTrace) {}

    /// Called only when the PC input mux selects a non-sequential source.
    fn trace_pc_load(
        &mut self,
        _before: u16,
        _after: u16,
        _source: PcSource,
        _control: &'static str,
    ) {
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cpu {
    regs: [u8; 8],
    pc: u16,
    sp: u16,
    flags: Flags,
    halted: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self {
            regs: [0; 8],
            pc: 0,
            sp: 0x7FFF,
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

        let pc = self.pc;
        let opcode = self.next8(bus);
        bus.trace_decode(pc, opcode, mnemonic(opcode));

        match opcode {
            op::NOP => StepOutcome::Continue,
            op::LDI => {
                let Some(reg) = self.next_reg(bus) else {
                    return self.fault(pc, opcode);
                };
                let value = self.next8(bus);
                self.write_reg(bus, pc, reg, value, "LDI");
                self.flags = Flags {
                    zero: value == 0,
                    carry: false,
                    less: false,
                };
                bus.trace_alu_exact(pc, logic_trace(AluOp::Pass, value, 0, value), "LDI");
                StepOutcome::Continue
            }
            op::LD => {
                let Some(reg) = self.next_reg(bus) else {
                    return self.fault(pc, opcode);
                };
                let address = self.next16(bus);
                let value = bus.read8(pc, address);
                self.write_reg(bus, pc, reg, value, "LD");
                self.flags.zero = value == 0;
                self.flags.less = false;
                StepOutcome::Continue
            }
            op::ST => {
                let address = self.next16(bus);
                let Some(reg) = self.next_reg(bus) else {
                    return self.fault(pc, opcode);
                };
                bus.write8(pc, address, self.regs[reg as usize]);
                StepOutcome::Continue
            }
            op::MOV => {
                let Some(dst) = self.next_reg(bus) else {
                    return self.fault(pc, opcode);
                };
                let Some(src) = self.next_reg(bus) else {
                    return self.fault(pc, opcode);
                };
                let value = self.regs[src as usize];
                self.write_reg(bus, pc, dst, value, "MOV");
                self.flags.zero = value == 0;
                self.flags.less = false;
                bus.trace_alu_exact(pc, logic_trace(AluOp::Pass, value, 0, value), "MOV");
                StepOutcome::Continue
            }
            op::ADD => {
                let Some(dst) = self.next_reg(bus) else {
                    return self.fault(pc, opcode);
                };
                let Some(src) = self.next_reg(bus) else {
                    return self.fault(pc, opcode);
                };
                let trace = ripple_add(
                    self.regs[dst as usize],
                    self.regs[src as usize],
                    false,
                    AluOp::Add,
                );
                self.commit_arithmetic(bus, pc, dst, trace, "ADD");
                StepOutcome::Continue
            }
            op::ADDI => self.immediate_arithmetic(bus, pc, opcode, AluOp::Add, "ADDI"),
            op::SUBI => self.immediate_arithmetic(bus, pc, opcode, AluOp::Sub, "SUBI"),
            op::ANDI => self.immediate_logic(bus, pc, opcode, AluOp::And, "ANDI"),
            op::ORI => self.immediate_logic(bus, pc, opcode, AluOp::Or, "ORI"),
            op::XORI => self.immediate_logic(bus, pc, opcode, AluOp::Xor, "XORI"),
            op::INC => self.unary_arithmetic(bus, pc, opcode, true),
            op::DEC => self.unary_arithmetic(bus, pc, opcode, false),
            op::CMP => {
                let Some(lhs_reg) = self.next_reg(bus) else {
                    return self.fault(pc, opcode);
                };
                let Some(rhs_reg) = self.next_reg(bus) else {
                    return self.fault(pc, opcode);
                };
                let trace = ripple_sub(
                    self.regs[lhs_reg as usize],
                    self.regs[rhs_reg as usize],
                    AluOp::Compare,
                );
                self.commit_compare(bus, pc, trace, "CMP");
                StepOutcome::Continue
            }
            op::CMPI => {
                let Some(reg) = self.next_reg(bus) else {
                    return self.fault(pc, opcode);
                };
                let rhs = self.next8(bus);
                let trace = ripple_sub(self.regs[reg as usize], rhs, AluOp::Compare);
                self.commit_compare(bus, pc, trace, "CMPI");
                StepOutcome::Continue
            }
            op::JMP => {
                let target = self.next16(bus);
                self.load_pc(bus, target, PcSource::Jump, "JMP");
                bus.trace_control(pc, "JMP");
                StepOutcome::Continue
            }
            op::JZ => self.branch(bus, pc, self.flags.zero, "JZ"),
            op::JNZ => self.branch(bus, pc, !self.flags.zero, "JNZ"),
            op::JLT => self.branch(bus, pc, self.flags.less, "JLT"),
            op::JGE => self.branch(bus, pc, !self.flags.less, "JGE"),
            op::JC => self.branch(bus, pc, self.flags.carry, "JC"),
            op::CALL => {
                let target = self.next16(bus);
                let ret = self.pc;
                self.push(bus, pc, (ret >> 8) as u8);
                self.push(bus, pc, ret as u8);
                self.load_pc(bus, target, PcSource::Call, "CALL");
                bus.trace_control(pc, "CALL");
                StepOutcome::Continue
            }
            op::RET => {
                let lo = self.pop(bus, pc);
                let hi = self.pop(bus, pc);
                let target = u16::from_le_bytes([lo, hi]);
                self.load_pc(bus, target, PcSource::Return, "RET");
                bus.trace_control(pc, "RET");
                StepOutcome::Continue
            }
            op::WAIT_VBLANK => {
                bus.trace_control(pc, "WAIT_VBLANK");
                StepOutcome::WaitVBlank
            }
            op::HALT => {
                self.halted = true;
                bus.trace_control(pc, "HALT");
                StepOutcome::Halted
            }
            _ => self.fault(pc, opcode),
        }
    }

    /// Fetches one byte and advances the semantic PC through the same sixteen-bit
    /// ripple incrementer represented by INC LO / CARRY / INC HI in the SVG.
    fn next8<B: Bus>(&mut self, bus: &mut B) -> u8 {
        let before = self.pc;
        let value = bus.fetch8(before);
        let increment = ripple_increment16(before);
        self.pc = increment.after;
        bus.trace_pc_increment(increment);
        value
    }

    fn next16<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let lo = self.next8(bus);
        let hi = self.next8(bus);
        u16::from_le_bytes([lo, hi])
    }

    fn next_reg<B: Bus>(&mut self, bus: &mut B) -> Option<Reg> {
        Reg::from_code(self.next8(bus))
    }

    fn load_pc<B: Bus>(
        &mut self,
        bus: &mut B,
        target: u16,
        source: PcSource,
        control: &'static str,
    ) {
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
        let slot = &mut self.regs[reg as usize];
        let before = *slot;
        *slot = value;
        bus.trace_register_write(pc, reg, before, value, control);
    }

    fn immediate_arithmetic<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        opcode: u8,
        op: AluOp,
        control: &'static str,
    ) -> StepOutcome {
        let Some(reg) = self.next_reg(bus) else {
            return self.fault(pc, opcode);
        };
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

    fn immediate_logic<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        opcode: u8,
        op: AluOp,
        control: &'static str,
    ) -> StepOutcome {
        let Some(reg) = self.next_reg(bus) else {
            return self.fault(pc, opcode);
        };
        let rhs = self.next8(bus);
        let lhs = self.regs[reg as usize];
        let result = match op {
            AluOp::And => lhs & rhs,
            AluOp::Or => lhs | rhs,
            AluOp::Xor => lhs ^ rhs,
            _ => unreachable!("immediate logic only supports boolean operations"),
        };
        let trace = logic_trace(op, lhs, rhs, result);
        self.write_reg(bus, pc, reg, result, control);
        self.flags = Flags {
            zero: result == 0,
            carry: false,
            less: false,
        };
        bus.trace_alu_exact(pc, trace, control);
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
        let lhs = self.regs[reg as usize];
        let (trace, control) = if increment {
            (ripple_add(lhs, 1, false, AluOp::Add), "INC")
        } else {
            (ripple_sub(lhs, 1, AluOp::Sub), "DEC")
        };
        self.commit_arithmetic(bus, pc, reg, trace, control);
        StepOutcome::Continue
    }

    fn commit_arithmetic<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        reg: Reg,
        trace: AluTrace,
        control: &'static str,
    ) {
        self.write_reg(bus, pc, reg, trace.result, control);
        self.flags = Flags {
            zero: trace.result == 0,
            carry: trace.final_carry(),
            less: !trace.final_carry() && matches!(trace.op, AluOp::Sub),
        };
        bus.trace_alu_exact(pc, trace, control);
    }

    fn commit_compare<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        trace: AluTrace,
        control: &'static str,
    ) {
        self.flags = Flags {
            zero: trace.result == 0,
            carry: trace.final_carry(),
            less: !trace.final_carry(),
        };
        bus.trace_alu_exact(pc, trace, control);
    }

    fn branch<B: Bus>(
        &mut self,
        bus: &mut B,
        pc: u16,
        condition: bool,
        control: &'static str,
    ) -> StepOutcome {
        let target = self.next16(bus);
        if condition {
            self.load_pc(bus, target, PcSource::Branch, control);
        }
        bus.trace_control(pc, control);
        StepOutcome::Continue
    }

    fn push<B: Bus>(&mut self, bus: &mut B, pc: u16, value: u8) {
        self.sp = self.sp.wrapping_sub(1);
        bus.write8(pc, self.sp, value);
    }

    fn pop<B: Bus>(&mut self, bus: &mut B, pc: u16) -> u8 {
        let value = bus.read8(pc, self.sp);
        self.sp = self.sp.wrapping_add(1);
        value
    }

    fn fault(&mut self, pc: u16, opcode: u8) -> StepOutcome {
        self.halted = true;
        StepOutcome::Fault { pc, opcode }
    }
}

#[must_use]
pub const fn mnemonic(value: u8) -> &'static str {
    match value {
        op::NOP => "NOP",
        op::LDI => "LDI",
        op::LD => "LD",
        op::ST => "ST",
        op::MOV => "MOV",
        op::ADD => "ADD",
        op::ADDI => "ADDI",
        op::SUBI => "SUBI",
        op::ANDI => "ANDI",
        op::ORI => "ORI",
        op::XORI => "XORI",
        op::INC => "INC",
        op::DEC => "DEC",
        op::CMP => "CMP",
        op::CMPI => "CMPI",
        op::JMP => "JMP",
        op::JZ => "JZ",
        op::JNZ => "JNZ",
        op::JLT => "JLT",
        op::JGE => "JGE",
        op::JC => "JC",
        op::CALL => "CALL",
        op::RET => "RET",
        op::WAIT_VBLANK => "WAIT_VBLANK",
        op::HALT => "HALT",
        _ => "FAULT",
    }
}

#[must_use]
pub const fn phase_for_opcode(value: u8) -> PhaseKind {
    match value {
        op::LD => PhaseKind::MemoryRead,
        op::ST => PhaseKind::MemoryWrite,
        op::ADD
        | op::ADDI
        | op::SUBI
        | op::ANDI
        | op::ORI
        | op::XORI
        | op::INC
        | op::DEC
        | op::CMP
        | op::CMPI => PhaseKind::Alu,
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
    }

    impl Default for TestBus {
        fn default() -> Self {
            Self {
                memory: vec![0; 65_536],
                exact_alu: Vec::new(),
                writes: Vec::new(),
                pc_increments: Vec::new(),
                pc_loads: Vec::new(),
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
    }

    #[test]
    fn load_add_store_uses_exact_ripple_path() {
        let mut bus = TestBus::default();
        let program = [
            op::LDI,
            Reg::A.code(),
            4,
            op::ADDI,
            Reg::A.code(),
            6,
            op::ST,
            0x80,
            0,
            Reg::A.code(),
            op::HALT,
        ];
        bus.memory[..program.len()].copy_from_slice(&program);
        let mut cpu = Cpu::default();
        for _ in 0..4 {
            cpu.step(&mut bus);
        }
        assert_eq!(bus.memory[0x80], 10);
        assert_eq!(bus.exact_alu[1].result, 10);
        assert_eq!(bus.exact_alu[1].op, AluOp::Add);
        assert!(bus.writes.contains(&(Reg::A, 4, 10)));
        assert!(!bus.pc_increments.is_empty());
    }

    #[test]
    fn fetch_pc_advance_is_the_exact_ripple_incrementer() {
        let mut bus = TestBus::default();
        bus.memory[0x00FF] = op::NOP;
        let mut cpu = Cpu::default();
        cpu.pc = 0x00FF;
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        let increment = bus.pc_increments.last().copied().expect("pc increment");
        assert_eq!(increment.before, 0x00FF);
        assert_eq!(increment.after, 0x0100);
        assert!(increment.low_byte_carry());
        assert_eq!(cpu.pc(), increment.after);
    }

    #[test]
    fn jump_and_branch_select_nonsequential_pc_mux_sources() {
        let mut bus = TestBus::default();
        let program = [
            op::JMP,
            0x04,
            0x00,
            op::HALT,
            op::JZ,
            0x09,
            0x00,
            op::HALT,
            op::HALT,
            op::HALT,
        ];
        bus.memory[..program.len()].copy_from_slice(&program);
        let mut cpu = Cpu::default();
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.pc(), 4);
        assert_eq!(bus.pc_loads[0].2, PcSource::Jump);

        cpu.flags.zero = true;
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.pc(), 9);
        assert_eq!(bus.pc_loads[1].2, PcSource::Branch);
    }
}
