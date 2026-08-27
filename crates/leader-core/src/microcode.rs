use crate::isa::op;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ControlWord {
    pub reg_write: bool,
    pub alu_enable: bool,
    pub mem_read: bool,
    pub mem_write: bool,
    pub pc_load: bool,
    pub stack_enable: bool,
    pub wait: bool,
    pub halt: bool,
}

impl ControlWord {
    #[must_use]
    pub const fn bits(self) -> u8 {
        (self.reg_write as u8)
            | ((self.alu_enable as u8) << 1)
            | ((self.mem_read as u8) << 2)
            | ((self.mem_write as u8) << 3)
            | ((self.pc_load as u8) << 4)
            | ((self.stack_enable as u8) << 5)
            | ((self.wait as u8) << 6)
            | ((self.halt as u8) << 7)
    }

    const fn new(
        reg_write: bool,
        alu_enable: bool,
        mem_read: bool,
        mem_write: bool,
        pc_load: bool,
        stack_enable: bool,
        wait: bool,
        halt: bool,
    ) -> Self {
        Self {
            reg_write,
            alu_enable,
            mem_read,
            mem_write,
            pc_load,
            stack_enable,
            wait,
            halt,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicroOp {
    Nop,
    LoadImmediate,
    LoadMemory,
    StoreMemory,
    Move,
    Add,
    AddImmediate,
    SubImmediate,
    AndImmediate,
    OrImmediate,
    XorImmediate,
    Increment,
    Decrement,
    Compare,
    CompareImmediate,
    Jump,
    JumpZero,
    JumpNotZero,
    JumpLess,
    JumpGreaterEqual,
    JumpCarry,
    Call,
    Return,
    WaitVBlank,
    Halt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicroInstruction {
    pub opcode: u8,
    pub mnemonic: &'static str,
    pub operation: MicroOp,
    pub control: ControlWord,
}

/// Physical address space of the 256x24 control ROM.
///
/// 00-02: common opcode fetch
/// 10-12: common operand/immediate fetch
/// 20-22: common memory read
/// 30-32: common memory write
/// 80-98: per-instruction execute entries
pub mod uaddr {
    pub const FETCH_T0: u8 = 0x00;
    pub const FETCH_T1: u8 = 0x01;
    pub const FETCH_T2: u8 = 0x02;

    pub const OPERAND_T0: u8 = 0x10;
    pub const OPERAND_T1: u8 = 0x11;
    pub const OPERAND_T2: u8 = 0x12;

    pub const READ_T0: u8 = 0x20;
    pub const READ_T1: u8 = 0x21;
    pub const READ_T2: u8 = 0x22;

    pub const WRITE_T0: u8 = 0x30;
    pub const WRITE_T1: u8 = 0x31;
    pub const WRITE_T2: u8 = 0x32;

    pub const EXEC_BASE: u8 = 0x80;
}

const NONE: ControlWord = ControlWord::new(false, false, false, false, false, false, false, false);
const REG_ALU: ControlWord = ControlWord::new(true, true, false, false, false, false, false, false);
const ALU_ONLY: ControlWord = ControlWord::new(false, true, false, false, false, false, false, false);
const LOAD: ControlWord = ControlWord::new(true, false, true, false, false, false, false, false);
const STORE: ControlWord = ControlWord::new(false, false, false, true, false, false, false, false);
const PC_LOAD: ControlWord = ControlWord::new(false, false, false, false, true, false, false, false);
const CALL: ControlWord = ControlWord::new(false, false, false, true, true, true, false, false);
const RET: ControlWord = ControlWord::new(false, false, true, false, true, true, false, false);
const WAIT: ControlWord = ControlWord::new(false, false, false, false, false, false, true, false);
const HALT: ControlWord = ControlWord::new(false, false, false, false, false, false, false, true);

#[must_use]
pub const fn decode(opcode: u8) -> Option<MicroInstruction> {
    let (mnemonic, operation, control) = match opcode {
        op::NOP => ("NOP", MicroOp::Nop, NONE),
        op::LDI => ("LDI", MicroOp::LoadImmediate, REG_ALU),
        op::LD => ("LD", MicroOp::LoadMemory, LOAD),
        op::ST => ("ST", MicroOp::StoreMemory, STORE),
        op::MOV => ("MOV", MicroOp::Move, REG_ALU),
        op::ADD => ("ADD", MicroOp::Add, REG_ALU),
        op::ADDI => ("ADDI", MicroOp::AddImmediate, REG_ALU),
        op::SUBI => ("SUBI", MicroOp::SubImmediate, REG_ALU),
        op::ANDI => ("ANDI", MicroOp::AndImmediate, REG_ALU),
        op::ORI => ("ORI", MicroOp::OrImmediate, REG_ALU),
        op::XORI => ("XORI", MicroOp::XorImmediate, REG_ALU),
        op::INC => ("INC", MicroOp::Increment, REG_ALU),
        op::DEC => ("DEC", MicroOp::Decrement, REG_ALU),
        op::CMP => ("CMP", MicroOp::Compare, ALU_ONLY),
        op::CMPI => ("CMPI", MicroOp::CompareImmediate, ALU_ONLY),
        op::JMP => ("JMP", MicroOp::Jump, PC_LOAD),
        op::JZ => ("JZ", MicroOp::JumpZero, PC_LOAD),
        op::JNZ => ("JNZ", MicroOp::JumpNotZero, PC_LOAD),
        op::JLT => ("JLT", MicroOp::JumpLess, PC_LOAD),
        op::JGE => ("JGE", MicroOp::JumpGreaterEqual, PC_LOAD),
        op::JC => ("JC", MicroOp::JumpCarry, PC_LOAD),
        op::CALL => ("CALL", MicroOp::Call, CALL),
        op::RET => ("RET", MicroOp::Return, RET),
        op::WAIT_VBLANK => ("WAIT_VBLANK", MicroOp::WaitVBlank, WAIT),
        op::HALT => ("HALT", MicroOp::Halt, HALT),
        _ => return None,
    };
    Some(MicroInstruction { opcode, mnemonic, operation, control })
}

#[must_use]
pub const fn control_word(opcode: u8) -> ControlWord {
    match decode(opcode) {
        Some(instruction) => instruction.control,
        None => NONE,
    }
}

/// Stable execute address in the physical control ROM. The mapping is dense so
/// all currently defined instructions fit inside one 256-entry ROM while common
/// fetch/read/write microprograms retain fixed low addresses.
#[must_use]
pub const fn execute_address(opcode: u8) -> Option<u8> {
    let slot = match opcode {
        op::NOP => 0,
        op::LDI => 1,
        op::LD => 2,
        op::ST => 3,
        op::MOV => 4,
        op::ADD => 5,
        op::ADDI => 6,
        op::SUBI => 7,
        op::ANDI => 8,
        op::ORI => 9,
        op::XORI => 10,
        op::INC => 11,
        op::DEC => 12,
        op::CMP => 13,
        op::CMPI => 14,
        op::JMP => 15,
        op::JZ => 16,
        op::JNZ => 17,
        op::JLT => 18,
        op::JGE => 19,
        op::JC => 20,
        op::CALL => 21,
        op::RET => 22,
        op::WAIT_VBLANK => 23,
        op::HALT => 24,
        _ => return None,
    };
    Some(uaddr::EXEC_BASE + slot)
}

#[must_use]
pub fn control_word_at(address: u8, opcode: u8) -> ControlWord {
    match address {
        uaddr::FETCH_T0 | uaddr::OPERAND_T0 => NONE,
        uaddr::FETCH_T1 | uaddr::OPERAND_T1 | uaddr::READ_T1 => {
            ControlWord::new(false, false, true, false, false, false, false, false)
        }
        uaddr::FETCH_T2 | uaddr::OPERAND_T2 | uaddr::READ_T0 | uaddr::READ_T2 | uaddr::WRITE_T0 => NONE,
        uaddr::WRITE_T1 | uaddr::WRITE_T2 => {
            ControlWord::new(false, false, false, true, false, false, false, false)
        }
        value if execute_address(opcode) == Some(value) => control_word(opcode),
        _ => NONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_control_word_enables_alu_and_register_write() {
        let instruction = decode(op::ADDI).expect("ADDI microcode");
        assert_eq!(instruction.operation, MicroOp::AddImmediate);
        assert!(instruction.control.alu_enable);
        assert!(instruction.control.reg_write);
        assert_eq!(instruction.control.bits() & 0b11, 0b11);
    }

    #[test]
    fn every_opcode_has_unique_execute_microaddress() {
        let opcodes = [
            op::NOP, op::LDI, op::LD, op::ST, op::MOV, op::ADD, op::ADDI, op::SUBI,
            op::ANDI, op::ORI, op::XORI, op::INC, op::DEC, op::CMP, op::CMPI, op::JMP,
            op::JZ, op::JNZ, op::JLT, op::JGE, op::JC, op::CALL, op::RET, op::WAIT_VBLANK,
            op::HALT,
        ];
        let mut seen = [false; 256];
        for opcode in opcodes {
            let address = execute_address(opcode).expect("execute address");
            assert!(address >= uaddr::EXEC_BASE);
            assert!(!seen[address as usize], "duplicate µADDR {address:02X}");
            seen[address as usize] = true;
        }
    }

    #[test]
    fn physical_rom_contains_fetch_and_execute_words() {
        assert!(control_word_at(uaddr::FETCH_T1, op::ADDI).mem_read);
        let exec = execute_address(op::ADDI).expect("ADDI µADDR");
        let word = control_word_at(exec, op::ADDI);
        assert!(word.alu_enable && word.reg_write);
    }

    #[test]
    fn call_and_return_drive_stack_and_pc_mux() {
        let call = decode(op::CALL).expect("CALL microcode").control;
        let ret = decode(op::RET).expect("RET microcode").control;
        assert!(call.pc_load && call.stack_enable && call.mem_write);
        assert!(ret.pc_load && ret.stack_enable && ret.mem_read);
    }

    #[test]
    fn wait_and_halt_have_distinct_control_lines() {
        assert!(decode(op::WAIT_VBLANK).expect("WAIT").control.wait);
        assert!(decode(op::HALT).expect("HALT").control.halt);
    }
}
