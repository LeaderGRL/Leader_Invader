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
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    fn index(code: u8) -> Option<usize> {
        (code < 8).then_some(code as usize)
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

pub trait Bus {
    fn fetch8(&mut self, pc: u16) -> u8;
    fn read8(&mut self, pc: u16, address: u16) -> u8;
    fn write8(&mut self, pc: u16, address: u16, value: u8);
    fn trace_decode(&mut self, pc: u16, opcode: u8, mnemonic: &'static str);
    fn trace_alu(&mut self, pc: u16, value: u8, control: &'static str);
    fn trace_control(&mut self, pc: u16, control: &'static str);
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

        let start_pc = self.pc;
        let opcode = self.next8(bus);
        bus.trace_decode(start_pc, opcode, mnemonic(opcode));

        match opcode {
            op::NOP => StepOutcome::Continue,
            op::LDI => {
                let Some(reg) = Reg::index(self.next8(bus)) else {
                    return self.fault(start_pc, opcode);
                };
                let value = self.next8(bus);
                self.regs[reg] = value;
                self.set_zero(value);
                bus.trace_alu(start_pc, value, "LDI");
                StepOutcome::Continue
            }
            op::LD => {
                let Some(reg) = Reg::index(self.next8(bus)) else {
                    return self.fault(start_pc, opcode);
                };
                let address = self.next16(bus);
                let value = bus.read8(start_pc, address);
                self.regs[reg] = value;
                self.set_zero(value);
                StepOutcome::Continue
            }
            op::ST => {
                let address = self.next16(bus);
                let Some(reg) = Reg::index(self.next8(bus)) else {
                    return self.fault(start_pc, opcode);
                };
                bus.write8(start_pc, address, self.regs[reg]);
                StepOutcome::Continue
            }
            op::MOV => {
                let Some(dst) = Reg::index(self.next8(bus)) else {
                    return self.fault(start_pc, opcode);
                };
                let Some(src) = Reg::index(self.next8(bus)) else {
                    return self.fault(start_pc, opcode);
                };
                self.regs[dst] = self.regs[src];
                self.set_zero(self.regs[dst]);
                bus.trace_alu(start_pc, self.regs[dst], "MOV");
                StepOutcome::Continue
            }
            op::ADD => {
                let Some(dst) = Reg::index(self.next8(bus)) else {
                    return self.fault(start_pc, opcode);
                };
                let Some(src) = Reg::index(self.next8(bus)) else {
                    return self.fault(start_pc, opcode);
                };
                let (value, carry) = self.regs[dst].overflowing_add(self.regs[src]);
                self.regs[dst] = value;
                self.flags.carry = carry;
                self.flags.less = false;
                self.flags.zero = value == 0;
                bus.trace_alu(start_pc, value, "ADD");
                StepOutcome::Continue
            }
            op::ADDI => {
                let Some(reg) = Reg::index(self.next8(bus)) else {
                    return self.fault(start_pc, opcode);
                };
                let imm = self.next8(bus);
                let (value, carry) = self.regs[reg].overflowing_add(imm);
                self.regs[reg] = value;
                self.flags.carry = carry;
                self.flags.less = false;
                self.flags.zero = value == 0;
                bus.trace_alu(start_pc, value, "ADDI");
                StepOutcome::Continue
            }
            op::SUBI => {
                let Some(reg) = Reg::index(self.next8(bus)) else {
                    return self.fault(start_pc, opcode);
                };
                let imm = self.next8(bus);
                let (value, borrow) = self.regs[reg].overflowing_sub(imm);
                self.regs[reg] = value;
                self.flags.carry = !borrow;
                self.flags.less = borrow;
                self.flags.zero = value == 0;
                bus.trace_alu(start_pc, value, "SUBI");
                StepOutcome::Continue
            }
            op::ANDI => self.logic_imm(bus, start_pc, opcode, "ANDI", |a, b| a & b),
            op::ORI => self.logic_imm(bus, start_pc, opcode, "ORI", |a, b| a | b),
            op::XORI => self.logic_imm(bus, start_pc, opcode, "XORI", |a, b| a ^ b),
            op::INC => {
                let Some(reg) = Reg::index(self.next8(bus)) else {
                    return self.fault(start_pc, opcode);
                };
                let (value, carry) = self.regs[reg].overflowing_add(1);
                self.regs[reg] = value;
                self.flags.carry = carry;
                self.flags.less = false;
                self.flags.zero = value == 0;
                bus.trace_alu(start_pc, value, "INC");
                StepOutcome::Continue
            }
            op::DEC => {
                let Some(reg) = Reg::index(self.next8(bus)) else {
                    return self.fault(start_pc, opcode);
                };
                let (value, borrow) = self.regs[reg].overflowing_sub(1);
                self.regs[reg] = value;
                self.flags.carry = !borrow;
                self.flags.less = borrow;
                self.flags.zero = value == 0;
                bus.trace_alu(start_pc, value, "DEC");
                StepOutcome::Continue
            }
            op::CMP => {
                let Some(lhs) = Reg::index(self.next8(bus)) else {
                    return self.fault(start_pc, opcode);
                };
                let Some(rhs) = Reg::index(self.next8(bus)) else {
                    return self.fault(start_pc, opcode);
                };
                self.compare(self.regs[lhs], self.regs[rhs]);
                bus.trace_alu(start_pc, self.regs[lhs].wrapping_sub(self.regs[rhs]), "CMP");
                StepOutcome::Continue
            }
            op::CMPI => {
                let Some(reg) = Reg::index(self.next8(bus)) else {
                    return self.fault(start_pc, opcode);
                };
                let imm = self.next8(bus);
                self.compare(self.regs[reg], imm);
                bus.trace_alu(start_pc, self.regs[reg].wrapping_sub(imm), "CMPI");
                StepOutcome::Continue
            }
            op::JMP => {
                let target = self.next16(bus);
                self.pc = target;
                bus.trace_control(start_pc, "JMP");
                StepOutcome::Continue
            }
            op::JZ => self.branch(bus, start_pc, self.flags.zero, "JZ"),
            op::JNZ => self.branch(bus, start_pc, !self.flags.zero, "JNZ"),
            op::JLT => self.branch(bus, start_pc, self.flags.less, "JLT"),
            op::JGE => self.branch(bus, start_pc, !self.flags.less, "JGE"),
            op::JC => self.branch(bus, start_pc, self.flags.carry, "JC"),
            op::CALL => {
                let target = self.next16(bus);
                let ret = self.pc;
                self.push8(bus, start_pc, (ret >> 8) as u8);
                self.push8(bus, start_pc, ret as u8);
                self.pc = target;
                bus.trace_control(start_pc, "CALL");
                StepOutcome::Continue
            }
            op::RET => {
                let lo = self.pop8(bus, start_pc);
                let hi = self.pop8(bus, start_pc);
                self.pc = u16::from_le_bytes([lo, hi]);
                bus.trace_control(start_pc, "RET");
                StepOutcome::Continue
            }
            op::WAIT_VBLANK => {
                bus.trace_control(start_pc, "WAIT_VBLANK");
                StepOutcome::WaitVBlank
            }
            op::HALT => {
                self.halted = true;
                bus.trace_control(start_pc, "HALT");
                StepOutcome::Halted
            }
            _ => self.fault(start_pc, opcode),
        }
    }

    fn next8<B: Bus>(&mut self, bus: &mut B) -> u8 {
        let value = bus.fetch8(self.pc);
        self.pc = self.pc.wrapping_add(1);
        value
    }

    fn next16<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let lo = self.next8(bus);
        let hi = self.next8(bus);
        u16::from_le_bytes([lo, hi])
    }

    fn logic_imm<B: Bus, F>(
        &mut self,
        bus: &mut B,
        start_pc: u16,
        opcode: u8,
        control: &'static str,
        operation: F,
    ) -> StepOutcome
    where
        F: FnOnce(u8, u8) -> u8,
    {
        let Some(reg) = Reg::index(self.next8(bus)) else {
            return self.fault(start_pc, opcode);
        };
        let imm = self.next8(bus);
        let value = operation(self.regs[reg], imm);
        self.regs[reg] = value;
        self.flags.zero = value == 0;
        self.flags.carry = false;
        self.flags.less = false;
        bus.trace_alu(start_pc, value, control);
        StepOutcome::Continue
    }

    fn compare(&mut self, lhs: u8, rhs: u8) {
        self.flags.zero = lhs == rhs;
        self.flags.less = lhs < rhs;
        self.flags.carry = lhs >= rhs;
    }

    fn branch<B: Bus>(
        &mut self,
        bus: &mut B,
        start_pc: u16,
        condition: bool,
        control: &'static str,
    ) -> StepOutcome {
        let target = self.next16(bus);
        if condition {
            self.pc = target;
        }
        bus.trace_control(start_pc, control);
        StepOutcome::Continue
    }

    fn push8<B: Bus>(&mut self, bus: &mut B, pc: u16, value: u8) {
        self.sp = self.sp.wrapping_sub(1);
        bus.write8(pc, self.sp, value);
    }

    fn pop8<B: Bus>(&mut self, bus: &mut B, pc: u16) -> u8 {
        let value = bus.read8(pc, self.sp);
        self.sp = self.sp.wrapping_add(1);
        value
    }

    fn set_zero(&mut self, value: u8) {
        self.flags.zero = value == 0;
        self.flags.less = false;
    }

    fn fault(&mut self, pc: u16, opcode: u8) -> StepOutcome {
        self.halted = true;
        StepOutcome::Fault { pc, opcode }
    }
}

#[must_use]
pub const fn mnemonic(opcode: u8) -> &'static str {
    match opcode {
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
pub const fn phase_for_opcode(opcode: u8) -> PhaseKind {
    match opcode {
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

    #[derive(Default)]
    struct TestBus {
        mem: [u8; 256],
    }

    impl Bus for TestBus {
        fn fetch8(&mut self, pc: u16) -> u8 {
            self.mem[pc as usize]
        }
        fn read8(&mut self, _pc: u16, address: u16) -> u8 {
            self.mem[address as usize]
        }
        fn write8(&mut self, _pc: u16, address: u16, value: u8) {
            self.mem[address as usize] = value;
        }
        fn trace_decode(&mut self, _pc: u16, _opcode: u8, _mnemonic: &'static str) {}
        fn trace_alu(&mut self, _pc: u16, _value: u8, _control: &'static str) {}
        fn trace_control(&mut self, _pc: u16, _control: &'static str) {}
    }

    #[test]
    fn executes_real_load_add_store_sequence() {
        let mut bus = TestBus::default();
        bus.mem[..13].copy_from_slice(&[
            op::LDI,
            Reg::A.code(),
            4,
            op::ADDI,
            Reg::A.code(),
            6,
            op::ST,
            0x80,
            0x00,
            Reg::A.code(),
            op::HALT,
            op::NOP,
            op::NOP,
        ]);
        let mut cpu = Cpu::default();
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.step(&mut bus), StepOutcome::Continue);
        assert_eq!(cpu.step(&mut bus), StepOutcome::Halted);
        assert_eq!(bus.mem[0x80], 10);
    }
}
