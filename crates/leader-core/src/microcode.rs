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
}

#[must_use]
pub const fn control_word(opcode: u8) -> ControlWord {
    match opcode {
        op::LDI | op::MOV => ControlWord {
            reg_write: true,
            alu_enable: true,
            ..ControlWord::default_const()
        },
        op::LD => ControlWord {
            reg_write: true,
            mem_read: true,
            ..ControlWord::default_const()
        },
        op::ST => ControlWord {
            mem_write: true,
            ..ControlWord::default_const()
        },
        op::ADD | op::ADDI | op::SUBI | op::ANDI | op::ORI | op::XORI | op::INC | op::DEC => {
            ControlWord {
                reg_write: true,
                alu_enable: true,
                ..ControlWord::default_const()
            }
        }
        op::CMP | op::CMPI => ControlWord {
            alu_enable: true,
            ..ControlWord::default_const()
        },
        op::JMP | op::JZ | op::JNZ | op::JLT | op::JGE | op::JC => ControlWord {
            pc_load: true,
            ..ControlWord::default_const()
        },
        op::CALL | op::RET => ControlWord {
            pc_load: true,
            stack_enable: true,
            mem_read: opcode == op::RET,
            mem_write: opcode == op::CALL,
            ..ControlWord::default_const()
        },
        op::WAIT_VBLANK => ControlWord {
            wait: true,
            ..ControlWord::default_const()
        },
        op::HALT => ControlWord {
            halt: true,
            ..ControlWord::default_const()
        },
        _ => ControlWord::default_const(),
    }
}

impl ControlWord {
    const fn default_const() -> Self {
        Self {
            reg_write: false,
            alu_enable: false,
            mem_read: false,
            mem_write: false,
            pc_load: false,
            stack_enable: false,
            wait: false,
            halt: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_control_word_enables_alu_and_register_write() {
        let word = control_word(op::ADDI);
        assert!(word.alu_enable);
        assert!(word.reg_write);
        assert!(!word.mem_write);
        assert_eq!(word.bits() & 0b11, 0b11);
    }

    #[test]
    fn call_and_return_drive_stack_and_pc_mux() {
        let call = control_word(op::CALL);
        let ret = control_word(op::RET);
        assert!(call.pc_load && call.stack_enable && call.mem_write);
        assert!(ret.pc_load && ret.stack_enable && ret.mem_read);
    }

    #[test]
    fn wait_and_halt_have_distinct_control_lines() {
        assert!(control_word(op::WAIT_VBLANK).wait);
        assert!(!control_word(op::WAIT_VBLANK).halt);
        assert!(control_word(op::HALT).halt);
    }
}
