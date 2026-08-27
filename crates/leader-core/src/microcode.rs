use crate::isa::op;

pub mod internal {
    pub const MAR_LOAD: u16 = 1 << 0;
    pub const MDR_LOAD: u16 = 1 << 1;
    pub const IR_LOAD: u16 = 1 << 2;
    pub const PC_INC: u16 = 1 << 3;
    pub const OPERAND_A_LOAD: u16 = 1 << 4;
    pub const OPERAND_B_LOAD: u16 = 1 << 5;
    pub const ALU_OP_LOAD: u16 = 1 << 6;
    pub const FLAGS_LOAD: u16 = 1 << 7;
    pub const ADDR_LO_LOAD: u16 = 1 << 8;
    pub const ADDR_HI_LOAD: u16 = 1 << 9;
    pub const CONDITION_LOAD: u16 = 1 << 10;
    pub const PC_SELECT: u16 = 1 << 11;
    pub const REG_SELECT: u16 = 1 << 12;
    pub const BUS_ADDRESS_ENABLE: u16 = 1 << 13;
    pub const BUS_DATA_ENABLE: u16 = 1 << 14;
    pub const ARCH_COMMIT: u16 = 1 << 15;
}

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
    internal: u16,
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

    #[must_use]
    pub const fn internal_bits(self) -> u16 { self.internal }

    #[must_use]
    pub const fn bits24(self) -> u32 { (self.bits() as u32) | ((self.internal as u32) << 8) }

    #[must_use]
    pub const fn has_internal(self, signal: u16) -> bool { self.internal & signal != 0 }

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
        Self { reg_write, alu_enable, mem_read, mem_write, pc_load, stack_enable, wait, halt, internal: 0 }
    }

    const fn with_internal(mut self, bits: u16) -> Self {
        self.internal = bits;
        self
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
    pub const EXEC_STRIDE: u8 = 5;
    pub const EXEC_ROWS: u8 = 5;
    pub const EXEC_LAST: u8 = 0xFC;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicroAddressSource { FetchStart, Sequential, Dispatch, RoutineCall, RoutineReturn }

impl MicroAddressSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FetchStart => "fetch_start",
            Self::Sequential => "sequential",
            Self::Dispatch => "dispatch",
            Self::RoutineCall => "routine_call",
            Self::RoutineReturn => "routine_return",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicroAddressTransition { pub before: u8, pub after: u8, pub source: MicroAddressSource }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicroSequencer { address: u8, return_address: u8 }

impl Default for MicroSequencer {
    fn default() -> Self { Self { address: uaddr::FETCH_T0, return_address: uaddr::FETCH_T0 } }
}

impl MicroSequencer {
    #[must_use] pub const fn address(self) -> u8 { self.address }
    #[must_use] pub const fn return_address(self) -> u8 { self.return_address }
    pub fn fetch_start(&mut self) -> MicroAddressTransition { self.load(uaddr::FETCH_T0, MicroAddressSource::FetchStart) }
    pub fn advance(&mut self) -> MicroAddressTransition { self.load(self.address.wrapping_add(1), MicroAddressSource::Sequential) }
    pub fn dispatch(&mut self, address: u8) -> MicroAddressTransition { self.load(address, MicroAddressSource::Dispatch) }
    pub fn call(&mut self, address: u8) -> MicroAddressTransition {
        self.return_address = self.address;
        self.load(address, MicroAddressSource::RoutineCall)
    }
    pub fn return_from_routine(&mut self) -> MicroAddressTransition { self.load(self.return_address, MicroAddressSource::RoutineReturn) }
    fn load(&mut self, address: u8, source: MicroAddressSource) -> MicroAddressTransition {
        let before = self.address;
        self.address = address;
        MicroAddressTransition { before, after: address, source }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecuteRowKind { Operand, Address, Condition, PcSelect, AluSelect, Propagate, Memory, Stack, Commit, Idle }

impl ExecuteRowKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Operand => "operand",
            Self::Address => "address",
            Self::Condition => "condition",
            Self::PcSelect => "pc_select",
            Self::AluSelect => "alu_select",
            Self::Propagate => "propagate",
            Self::Memory => "memory",
            Self::Stack => "stack",
            Self::Commit => "commit",
            Self::Idle => "idle",
        }
    }
}

const NONE: ControlWord = ControlWord::new(false, false, false, false, false, false, false, false);
const REG_ALU: ControlWord = ControlWord::new(true, true, false, false, false, false, false, false);
const REG_WRITE: ControlWord = ControlWord::new(true, false, false, false, false, false, false, false);
const ALU_ONLY: ControlWord = ControlWord::new(false, true, false, false, false, false, false, false);
const LOAD: ControlWord = ControlWord::new(true, false, true, false, false, false, false, false);
const STORE: ControlWord = ControlWord::new(false, false, false, true, false, false, false, false);
const PC_LOAD: ControlWord = ControlWord::new(false, false, false, false, true, false, false, false);
const STACK_ONLY: ControlWord = ControlWord::new(false, false, false, false, false, true, false, false);
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

#[must_use] pub const fn control_word(opcode: u8) -> ControlWord { match decode(opcode) { Some(i) => i.control, None => NONE } }

#[must_use]
pub const fn opcode_slot(opcode: u8) -> Option<u8> {
    Some(match opcode {
        op::NOP => 0, op::LDI => 1, op::LD => 2, op::ST => 3, op::MOV => 4,
        op::ADD => 5, op::ADDI => 6, op::SUBI => 7, op::ANDI => 8, op::ORI => 9,
        op::XORI => 10, op::INC => 11, op::DEC => 12, op::CMP => 13, op::CMPI => 14,
        op::JMP => 15, op::JZ => 16, op::JNZ => 17, op::JLT => 18, op::JGE => 19,
        op::JC => 20, op::CALL => 21, op::RET => 22, op::WAIT_VBLANK => 23, op::HALT => 24,
        _ => return None,
    })
}

#[must_use] pub const fn execute_address(opcode: u8) -> Option<u8> { match opcode_slot(opcode) { Some(s) => Some(uaddr::EXEC_BASE + s * uaddr::EXEC_STRIDE), None => None } }
#[must_use] pub const fn execute_step_address(opcode: u8, step: u8) -> Option<u8> { if step >= uaddr::EXEC_ROWS { return None; } match execute_address(opcode) { Some(b) => Some(b + step), None => None } }
#[must_use] pub const fn is_five_row_alu(opcode: u8) -> bool { matches!(opcode, op::MOV | op::ADD | op::SUBI | op::ANDI | op::ORI | op::XORI | op::INC | op::DEC | op::CMP | op::CMPI) }
#[must_use] pub const fn is_five_row_memory(opcode: u8) -> bool { matches!(opcode, op::LD | op::ST) }
#[must_use] pub const fn is_five_row_branch(opcode: u8) -> bool { matches!(opcode, op::JMP | op::JZ | op::JNZ | op::JLT | op::JGE | op::JC) }
#[must_use] pub const fn is_five_row_stack(opcode: u8) -> bool { matches!(opcode, op::CALL | op::RET) }

#[must_use]
pub const fn execute_control_step(opcode: u8) -> u8 {
    if is_five_row_alu(opcode) || is_five_row_memory(opcode) || is_five_row_branch(opcode) || is_five_row_stack(opcode) { 4 }
    else { match opcode { op::LDI | op::ADDI => 2, _ => 0 } }
}

#[must_use]
pub const fn execute_row_kind(opcode: u8, step: u8) -> Option<ExecuteRowKind> {
    if step >= uaddr::EXEC_ROWS || opcode_slot(opcode).is_none() { return None; }
    if is_five_row_alu(opcode) { return Some(match step { 0 | 1 => ExecuteRowKind::Operand, 2 => ExecuteRowKind::AluSelect, 3 => ExecuteRowKind::Propagate, 4 => ExecuteRowKind::Commit, _ => ExecuteRowKind::Idle }); }
    if opcode == op::LD { return Some(match step { 0 => ExecuteRowKind::Operand, 1 | 2 => ExecuteRowKind::Address, 3 => ExecuteRowKind::Memory, 4 => ExecuteRowKind::Commit, _ => ExecuteRowKind::Idle }); }
    if opcode == op::ST { return Some(match step { 0 | 1 => ExecuteRowKind::Address, 2 => ExecuteRowKind::Operand, 3 => ExecuteRowKind::Memory, 4 => ExecuteRowKind::Commit, _ => ExecuteRowKind::Idle }); }
    if is_five_row_branch(opcode) { return Some(match step { 0 | 1 => ExecuteRowKind::Address, 2 => ExecuteRowKind::Condition, 3 => ExecuteRowKind::PcSelect, 4 => ExecuteRowKind::Commit, _ => ExecuteRowKind::Idle }); }
    if opcode == op::CALL { return Some(match step { 0 | 1 => ExecuteRowKind::Address, 2 | 3 => ExecuteRowKind::Stack, 4 => ExecuteRowKind::Commit, _ => ExecuteRowKind::Idle }); }
    if opcode == op::RET { return Some(match step { 0 | 1 => ExecuteRowKind::Stack, 2 | 3 => ExecuteRowKind::PcSelect, 4 => ExecuteRowKind::Commit, _ => ExecuteRowKind::Idle }); }
    match opcode {
        op::LDI | op::ADDI => Some(match step { 0 | 1 => ExecuteRowKind::Operand, 2 => ExecuteRowKind::Commit, _ => ExecuteRowKind::Idle }),
        _ => Some(if step == 0 { ExecuteRowKind::Commit } else { ExecuteRowKind::Idle }),
    }
}

const fn execute_internal(opcode: u8, step: u8) -> u16 {
    use internal::*;
    if is_five_row_alu(opcode) {
        return match step {
            0 => OPERAND_A_LOAD | REG_SELECT,
            1 => OPERAND_B_LOAD | REG_SELECT,
            2 => ALU_OP_LOAD,
            3 => FLAGS_LOAD,
            4 => ARCH_COMMIT,
            _ => 0,
        };
    }
    if matches!(opcode, op::LD | op::ST) {
        return match step {
            0 => if opcode == op::LD { OPERAND_A_LOAD | REG_SELECT } else { ADDR_LO_LOAD },
            1 => if opcode == op::LD { ADDR_LO_LOAD } else { ADDR_HI_LOAD },
            2 => if opcode == op::LD { ADDR_HI_LOAD } else { OPERAND_A_LOAD | REG_SELECT },
            3 => BUS_ADDRESS_ENABLE | BUS_DATA_ENABLE,
            4 => ARCH_COMMIT,
            _ => 0,
        };
    }
    if is_five_row_branch(opcode) {
        return match step { 0 => ADDR_LO_LOAD, 1 => ADDR_HI_LOAD, 2 => CONDITION_LOAD, 3 => PC_SELECT, 4 => ARCH_COMMIT, _ => 0 };
    }
    if opcode == op::CALL {
        return match step { 0 => ADDR_LO_LOAD, 1 => ADDR_HI_LOAD, 2 | 3 => BUS_ADDRESS_ENABLE | BUS_DATA_ENABLE, 4 => PC_SELECT | ARCH_COMMIT, _ => 0 };
    }
    if opcode == op::RET {
        return match step { 0 | 1 => BUS_ADDRESS_ENABLE | BUS_DATA_ENABLE, 2 | 3 => PC_SELECT, 4 => ARCH_COMMIT, _ => 0 };
    }
    match opcode {
        op::LDI | op::ADDI => match step { 0 => OPERAND_A_LOAD | REG_SELECT, 1 => OPERAND_B_LOAD, 2 => ARCH_COMMIT, _ => 0 },
        _ => if step == 0 { ARCH_COMMIT } else { 0 },
    }
}

#[must_use]
pub const fn execute_row_control(opcode: u8, step: u8) -> ControlWord {
    let external = if is_five_row_alu(opcode) {
        match step { 2 | 3 => ALU_ONLY, 4 => match opcode { op::CMP | op::CMPI => NONE, _ => REG_WRITE }, _ => NONE }
    } else if is_five_row_memory(opcode) {
        match (opcode, step) { (op::LD, 4) => REG_WRITE, _ => NONE }
    } else if is_five_row_branch(opcode) {
        if step == 4 { PC_LOAD } else { NONE }
    } else if opcode == op::CALL {
        match step { 2 | 3 => STACK_ONLY, 4 => PC_LOAD, _ => NONE }
    } else if opcode == op::RET {
        match step { 0 | 1 => STACK_ONLY, 4 => PC_LOAD, _ => NONE }
    } else if step == execute_control_step(opcode) { control_word(opcode) } else { NONE };
    external.with_internal(execute_internal(opcode, step))
}

#[must_use]
pub fn control_word_at(address: u8, opcode: u8) -> ControlWord {
    use internal::*;
    match address {
        uaddr::FETCH_T0 => NONE.with_internal(MAR_LOAD | BUS_ADDRESS_ENABLE),
        uaddr::FETCH_T1 => ControlWord::new(false, false, true, false, false, false, false, false).with_internal(MDR_LOAD | PC_INC | BUS_DATA_ENABLE),
        uaddr::FETCH_T2 => NONE.with_internal(IR_LOAD),
        uaddr::OPERAND_T0 => NONE.with_internal(MAR_LOAD | BUS_ADDRESS_ENABLE),
        uaddr::OPERAND_T1 => ControlWord::new(false, false, true, false, false, false, false, false).with_internal(MDR_LOAD | PC_INC | BUS_DATA_ENABLE),
        uaddr::OPERAND_T2 => NONE,
        uaddr::READ_T0 => NONE.with_internal(MAR_LOAD | BUS_ADDRESS_ENABLE),
        uaddr::READ_T1 => ControlWord::new(false, false, true, false, false, false, false, false).with_internal(MDR_LOAD | BUS_DATA_ENABLE),
        uaddr::READ_T2 => NONE,
        uaddr::WRITE_T0 => NONE.with_internal(MAR_LOAD | BUS_ADDRESS_ENABLE),
        uaddr::WRITE_T1 => ControlWord::new(false, false, false, true, false, false, false, false).with_internal(MDR_LOAD | BUS_DATA_ENABLE),
        uaddr::WRITE_T2 => ControlWord::new(false, false, false, true, false, false, false, false).with_internal(BUS_ADDRESS_ENABLE | BUS_DATA_ENABLE | ARCH_COMMIT),
        value => {
            let Some(base) = execute_address(opcode) else { return NONE; };
            if value < base || value >= base.saturating_add(uaddr::EXEC_ROWS) { return NONE; }
            execute_row_control(opcode, value - base)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPCODES: [u8; 25] = [op::NOP, op::LDI, op::LD, op::ST, op::MOV, op::ADD, op::ADDI, op::SUBI, op::ANDI, op::ORI, op::XORI, op::INC, op::DEC, op::CMP, op::CMPI, op::JMP, op::JZ, op::JNZ, op::JLT, op::JGE, op::JC, op::CALL, op::RET, op::WAIT_VBLANK, op::HALT];

    #[test]
    fn execute_blocks_fill_80_through_fc_without_overlap() {
        let mut seen = [false; 256];
        for opcode in OPCODES {
            let base = execute_address(opcode).unwrap();
            for step in 0..5 {
                let address = execute_step_address(opcode, step).unwrap();
                assert_eq!(address, base + step);
                assert!(!seen[address as usize]);
                seen[address as usize] = true;
            }
        }
        assert_eq!(execute_step_address(op::HALT, 4), Some(uaddr::EXEC_LAST));
    }

    #[test]
    fn physical_word_is_really_twenty_four_bits_wide() {
        let fetch0 = control_word_at(uaddr::FETCH_T0, op::NOP);
        let fetch1 = control_word_at(uaddr::FETCH_T1, op::NOP);
        let fetch2 = control_word_at(uaddr::FETCH_T2, op::NOP);
        assert!(fetch0.has_internal(internal::MAR_LOAD));
        assert!(fetch0.has_internal(internal::BUS_ADDRESS_ENABLE));
        assert!(fetch1.mem_read);
        assert!(fetch1.has_internal(internal::MDR_LOAD));
        assert!(fetch1.has_internal(internal::PC_INC));
        assert!(fetch2.has_internal(internal::IR_LOAD));
        assert_ne!(fetch0.bits24() >> 8, 0);
    }

    #[test]
    fn five_row_alu_exposes_internal_latch_path() {
        let base = execute_address(op::ADD).unwrap();
        assert!(control_word_at(base, op::ADD).has_internal(internal::OPERAND_A_LOAD));
        assert!(control_word_at(base + 1, op::ADD).has_internal(internal::OPERAND_B_LOAD));
        assert!(control_word_at(base + 2, op::ADD).has_internal(internal::ALU_OP_LOAD));
        assert!(control_word_at(base + 3, op::ADD).has_internal(internal::FLAGS_LOAD));
        assert!(control_word_at(base + 4, op::ADD).has_internal(internal::ARCH_COMMIT));
        assert!(control_word_at(base + 4, op::ADD).reg_write);
    }

    #[test]
    fn branch_and_stack_rows_expose_internal_selects() {
        let branch = execute_address(op::JZ).unwrap();
        assert!(control_word_at(branch, op::JZ).has_internal(internal::ADDR_LO_LOAD));
        assert!(control_word_at(branch + 1, op::JZ).has_internal(internal::ADDR_HI_LOAD));
        assert!(control_word_at(branch + 2, op::JZ).has_internal(internal::CONDITION_LOAD));
        assert!(control_word_at(branch + 3, op::JZ).has_internal(internal::PC_SELECT));
        assert!(control_word_at(branch + 4, op::JZ).pc_load);
        let call = execute_address(op::CALL).unwrap();
        assert!(control_word_at(call + 2, op::CALL).stack_enable);
        assert!(control_word_at(call + 2, op::CALL).has_internal(internal::BUS_DATA_ENABLE));
    }

    #[test]
    fn ldi_and_addi_roles_are_operand_operand_commit() {
        for opcode in [op::LDI, op::ADDI] {
            assert_eq!(execute_row_kind(opcode, 0), Some(ExecuteRowKind::Operand));
            assert_eq!(execute_row_kind(opcode, 1), Some(ExecuteRowKind::Operand));
            assert_eq!(execute_row_kind(opcode, 2), Some(ExecuteRowKind::Commit));
        }
    }

    #[test]
    fn microsequencer_has_real_increment_dispatch_and_return_paths() {
        let mut seq = MicroSequencer::default();
        assert_eq!(seq.advance().after, uaddr::FETCH_T1);
        let exec = execute_address(op::ADDI).unwrap();
        assert_eq!(seq.dispatch(exec).after, exec);
        assert_eq!(seq.call(uaddr::OPERAND_T0).after, uaddr::OPERAND_T0);
        assert_eq!(seq.advance().after, uaddr::OPERAND_T1);
        assert_eq!(seq.advance().after, uaddr::OPERAND_T2);
        assert_eq!(seq.return_from_routine().after, exec);
    }
}
