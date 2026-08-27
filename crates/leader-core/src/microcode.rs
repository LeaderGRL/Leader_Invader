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

/// Semantic operation selected by the control ROM. The CPU dispatches on this
/// value, never on the raw opcode, so opcode decoding has one authoritative table.
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
    Some(MicroInstruction {
        opcode,
        mnemonic,
        operation,
        control,
    })
}

#[must_use]
pub const fn control_word(opcode: u8) -> ControlWord {
    match decode(opcode) {
        Some(instruction) => instruction.control,
        None => NONE,
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
        assert!(!instruction.control.mem_write);
        assert_eq!(instruction.control.bits() & 0b11, 0b11);
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
        assert!(!decode(op::WAIT_VBLANK).expect("WAIT").control.halt);
        assert!(decode(op::HALT).expect("HALT").control.halt);
    }

    #[test]
    fn every_defined_opcode_has_one_authoritative_microinstruction() {
        for opcode in [
            op::NOP,
            op::LDI,
            op::LD,
            op::ST,
            op::MOV,
            op::ADD,
            op::ADDI,
            op::SUBI,
            op::ANDI,
            op::ORI,
            op::XORI,
            op::INC,
            op::DEC,
            op::CMP,
            op::CMPI,
            op::JMP,
            op::JZ,
            op::JNZ,
            op::JLT,
            op::JGE,
            op::JC,
            op::CALL,
            op::RET,
            op::WAIT_VBLANK,
            op::HALT,
        ] {
            let instruction = decode(opcode).expect("defined opcode must decode");
            assert_eq!(instruction.opcode, opcode);
            assert!(!instruction.mnemonic.is_empty());
        }
        assert!(decode(0xAA).is_none());
    }
}
