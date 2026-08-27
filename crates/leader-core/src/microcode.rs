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
        (self.reg_write as u8) | ((self.alu_enable as u8) << 1) | ((self.mem_read as u8) << 2)
            | ((self.mem_write as u8) << 3) | ((self.pc_load as u8) << 4)
            | ((self.stack_enable as u8) << 5) | ((self.wait as u8) << 6) | ((self.halt as u8) << 7)
    }
    const fn new(reg_write: bool, alu_enable: bool, mem_read: bool, mem_write: bool, pc_load: bool, stack_enable: bool, wait: bool, halt: bool) -> Self {
        Self { reg_write, alu_enable, mem_read, mem_write, pc_load, stack_enable, wait, halt }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicroOp {
    Nop, LoadImmediate, LoadMemory, StoreMemory, Move, Add, AddImmediate, SubImmediate,
    AndImmediate, OrImmediate, XorImmediate, Increment, Decrement, Compare, CompareImmediate,
    Jump, JumpZero, JumpNotZero, JumpLess, JumpGreaterEqual, JumpCarry, Call, Return, WaitVBlank, Halt,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicroInstruction { pub opcode: u8, pub mnemonic: &'static str, pub operation: MicroOp, pub control: ControlWord }

/// Physical address space of the 256x24 control ROM.
/// 00-02 fetch, 10-12 operand, 20-22 read, 30-32 write, 80-FC execute blocks.
pub mod uaddr {
    pub const FETCH_T0: u8 = 0x00; pub const FETCH_T1: u8 = 0x01; pub const FETCH_T2: u8 = 0x02;
    pub const OPERAND_T0: u8 = 0x10; pub const OPERAND_T1: u8 = 0x11; pub const OPERAND_T2: u8 = 0x12;
    pub const READ_T0: u8 = 0x20; pub const READ_T1: u8 = 0x21; pub const READ_T2: u8 = 0x22;
    pub const WRITE_T0: u8 = 0x30; pub const WRITE_T1: u8 = 0x31; pub const WRITE_T2: u8 = 0x32;
    pub const EXEC_BASE: u8 = 0x80; pub const EXEC_STRIDE: u8 = 5; pub const EXEC_ROWS: u8 = 5; pub const EXEC_LAST: u8 = 0xFC;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicroAddressSource { FetchStart, Sequential, Dispatch, RoutineCall, RoutineReturn }
impl MicroAddressSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self { Self::FetchStart => "fetch_start", Self::Sequential => "sequential", Self::Dispatch => "dispatch", Self::RoutineCall => "routine_call", Self::RoutineReturn => "routine_return" }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicroAddressTransition { pub before: u8, pub after: u8, pub source: MicroAddressSource }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicroSequencer { address: u8, return_address: u8 }
impl Default for MicroSequencer { fn default() -> Self { Self { address: uaddr::FETCH_T0, return_address: uaddr::FETCH_T0 } } }
impl MicroSequencer {
    #[must_use] pub const fn address(self) -> u8 { self.address }
    #[must_use] pub const fn return_address(self) -> u8 { self.return_address }
    pub fn fetch_start(&mut self) -> MicroAddressTransition { self.load(uaddr::FETCH_T0, MicroAddressSource::FetchStart) }
    pub fn advance(&mut self) -> MicroAddressTransition { self.load(self.address.wrapping_add(1), MicroAddressSource::Sequential) }
    pub fn dispatch(&mut self, address: u8) -> MicroAddressTransition { self.load(address, MicroAddressSource::Dispatch) }
    pub fn call(&mut self, address: u8) -> MicroAddressTransition { self.return_address = self.address; self.load(address, MicroAddressSource::RoutineCall) }
    pub fn return_from_routine(&mut self) -> MicroAddressTransition { self.load(self.return_address, MicroAddressSource::RoutineReturn) }
    fn load(&mut self, address: u8, source: MicroAddressSource) -> MicroAddressTransition { let before = self.address; self.address = address; MicroAddressTransition { before, after: address, source } }
}

/// Semantic role of a physical execute row. Transition source remains separate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecuteRowKind { Operand, Address, Condition, PcSelect, AluSelect, Propagate, Memory, Commit, Idle }
impl ExecuteRowKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Operand => "operand", Self::Address => "address", Self::Condition => "condition", Self::PcSelect => "pc_select",
            Self::AluSelect => "alu_select", Self::Propagate => "propagate", Self::Memory => "memory", Self::Commit => "commit", Self::Idle => "idle",
        }
    }
}

const NONE: ControlWord = ControlWord::new(false,false,false,false,false,false,false,false);
const REG_ALU: ControlWord = ControlWord::new(true,true,false,false,false,false,false,false);
const REG_WRITE: ControlWord = ControlWord::new(true,false,false,false,false,false,false,false);
const ALU_ONLY: ControlWord = ControlWord::new(false,true,false,false,false,false,false,false);
const LOAD: ControlWord = ControlWord::new(true,false,true,false,false,false,false,false);
const STORE: ControlWord = ControlWord::new(false,false,false,true,false,false,false,false);
const PC_LOAD: ControlWord = ControlWord::new(false,false,false,false,true,false,false,false);
const CALL: ControlWord = ControlWord::new(false,false,false,true,true,true,false,false);
const RET: ControlWord = ControlWord::new(false,false,true,false,true,true,false,false);
const WAIT: ControlWord = ControlWord::new(false,false,false,false,false,false,true,false);
const HALT: ControlWord = ControlWord::new(false,false,false,false,false,false,false,true);

#[must_use]
pub const fn decode(opcode: u8) -> Option<MicroInstruction> {
    let (mnemonic, operation, control) = match opcode {
        op::NOP => ("NOP",MicroOp::Nop,NONE), op::LDI => ("LDI",MicroOp::LoadImmediate,REG_ALU), op::LD => ("LD",MicroOp::LoadMemory,LOAD),
        op::ST => ("ST",MicroOp::StoreMemory,STORE), op::MOV => ("MOV",MicroOp::Move,REG_ALU), op::ADD => ("ADD",MicroOp::Add,REG_ALU),
        op::ADDI => ("ADDI",MicroOp::AddImmediate,REG_ALU), op::SUBI => ("SUBI",MicroOp::SubImmediate,REG_ALU), op::ANDI => ("ANDI",MicroOp::AndImmediate,REG_ALU),
        op::ORI => ("ORI",MicroOp::OrImmediate,REG_ALU), op::XORI => ("XORI",MicroOp::XorImmediate,REG_ALU), op::INC => ("INC",MicroOp::Increment,REG_ALU),
        op::DEC => ("DEC",MicroOp::Decrement,REG_ALU), op::CMP => ("CMP",MicroOp::Compare,ALU_ONLY), op::CMPI => ("CMPI",MicroOp::CompareImmediate,ALU_ONLY),
        op::JMP => ("JMP",MicroOp::Jump,PC_LOAD), op::JZ => ("JZ",MicroOp::JumpZero,PC_LOAD), op::JNZ => ("JNZ",MicroOp::JumpNotZero,PC_LOAD),
        op::JLT => ("JLT",MicroOp::JumpLess,PC_LOAD), op::JGE => ("JGE",MicroOp::JumpGreaterEqual,PC_LOAD), op::JC => ("JC",MicroOp::JumpCarry,PC_LOAD),
        op::CALL => ("CALL",MicroOp::Call,CALL), op::RET => ("RET",MicroOp::Return,RET), op::WAIT_VBLANK => ("WAIT_VBLANK",MicroOp::WaitVBlank,WAIT), op::HALT => ("HALT",MicroOp::Halt,HALT),
        _ => return None,
    };
    Some(MicroInstruction { opcode, mnemonic, operation, control })
}
#[must_use] pub const fn control_word(opcode: u8) -> ControlWord { match decode(opcode) { Some(i) => i.control, None => NONE } }
#[must_use]
pub const fn opcode_slot(opcode: u8) -> Option<u8> {
    Some(match opcode {
        op::NOP=>0,op::LDI=>1,op::LD=>2,op::ST=>3,op::MOV=>4,op::ADD=>5,op::ADDI=>6,op::SUBI=>7,op::ANDI=>8,op::ORI=>9,
        op::XORI=>10,op::INC=>11,op::DEC=>12,op::CMP=>13,op::CMPI=>14,op::JMP=>15,op::JZ=>16,op::JNZ=>17,op::JLT=>18,op::JGE=>19,
        op::JC=>20,op::CALL=>21,op::RET=>22,op::WAIT_VBLANK=>23,op::HALT=>24,_=>return None,
    })
}
#[must_use] pub const fn execute_address(opcode: u8) -> Option<u8> { match opcode_slot(opcode) { Some(s) => Some(uaddr::EXEC_BASE + s*uaddr::EXEC_STRIDE), None=>None } }
#[must_use] pub const fn execute_step_address(opcode: u8, step: u8) -> Option<u8> { if step>=uaddr::EXEC_ROWS{return None;} match execute_address(opcode){Some(b)=>Some(b+step),None=>None} }
#[must_use] pub const fn is_five_row_alu(opcode:u8)->bool{matches!(opcode,op::MOV|op::ADD|op::SUBI|op::ANDI|op::ORI|op::XORI|op::INC|op::DEC|op::CMP|op::CMPI)}
#[must_use] pub const fn is_five_row_memory(opcode:u8)->bool{matches!(opcode,op::LD|op::ST)}
#[must_use] pub const fn is_five_row_branch(opcode:u8)->bool{matches!(opcode,op::JMP|op::JZ|op::JNZ|op::JLT|op::JGE|op::JC)}
#[must_use]
pub const fn execute_control_step(opcode:u8)->u8{if is_five_row_alu(opcode)||is_five_row_memory(opcode)||is_five_row_branch(opcode){4}else{match opcode{op::LDI|op::ADDI=>2,_=>0}}}

#[must_use]
pub const fn execute_row_kind(opcode:u8,step:u8)->Option<ExecuteRowKind>{
    if step>=uaddr::EXEC_ROWS||opcode_slot(opcode).is_none(){return None;}
    if is_five_row_alu(opcode){return Some(match step{0|1=>ExecuteRowKind::Operand,2=>ExecuteRowKind::AluSelect,3=>ExecuteRowKind::Propagate,4=>ExecuteRowKind::Commit,_=>ExecuteRowKind::Idle});}
    if opcode==op::LD{return Some(match step{0=>ExecuteRowKind::Operand,1|2=>ExecuteRowKind::Address,3=>ExecuteRowKind::Memory,4=>ExecuteRowKind::Commit,_=>ExecuteRowKind::Idle});}
    if opcode==op::ST{return Some(match step{0|1=>ExecuteRowKind::Address,2=>ExecuteRowKind::Operand,3=>ExecuteRowKind::Memory,4=>ExecuteRowKind::Commit,_=>ExecuteRowKind::Idle});}
    if is_five_row_branch(opcode){return Some(match step{0|1=>ExecuteRowKind::Address,2=>ExecuteRowKind::Condition,3=>ExecuteRowKind::PcSelect,4=>ExecuteRowKind::Commit,_=>ExecuteRowKind::Idle});}
    match opcode{op::LDI|op::ADDI=>Some(match step{0|1=>ExecuteRowKind::Operand,2=>ExecuteRowKind::Commit,_=>ExecuteRowKind::Idle}),_=>Some(if step==0{ExecuteRowKind::Commit}else{ExecuteRowKind::Idle})}
}

#[must_use]
pub const fn execute_row_control(opcode:u8,step:u8)->ControlWord{
    if is_five_row_alu(opcode){return match step{2|3=>ALU_ONLY,4=>match opcode{op::CMP|op::CMPI=>NONE,_=>REG_WRITE},_=>NONE};}
    if is_five_row_memory(opcode){return match(opcode,step){(op::LD,4)=>REG_WRITE,_=>NONE};}
    if is_five_row_branch(opcode){return if step==4{PC_LOAD}else{NONE};}
    if step==execute_control_step(opcode){control_word(opcode)}else{NONE}
}

#[must_use]
pub fn control_word_at(address:u8,opcode:u8)->ControlWord{
    match address{
        uaddr::FETCH_T0|uaddr::OPERAND_T0=>NONE,
        uaddr::FETCH_T1|uaddr::OPERAND_T1|uaddr::READ_T1=>ControlWord::new(false,false,true,false,false,false,false,false),
        uaddr::FETCH_T2|uaddr::OPERAND_T2|uaddr::READ_T0|uaddr::READ_T2|uaddr::WRITE_T0=>NONE,
        uaddr::WRITE_T1|uaddr::WRITE_T2=>ControlWord::new(false,false,false,true,false,false,false,false),
        value=>{let Some(base)=execute_address(opcode)else{return NONE;};if value<base||value>=base.saturating_add(uaddr::EXEC_ROWS){return NONE;}execute_row_control(opcode,value-base)}
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    const OPCODES:[u8;25]=[op::NOP,op::LDI,op::LD,op::ST,op::MOV,op::ADD,op::ADDI,op::SUBI,op::ANDI,op::ORI,op::XORI,op::INC,op::DEC,op::CMP,op::CMPI,op::JMP,op::JZ,op::JNZ,op::JLT,op::JGE,op::JC,op::CALL,op::RET,op::WAIT_VBLANK,op::HALT];
    const FIVE_ROW_ALU:[u8;10]=[op::MOV,op::ADD,op::SUBI,op::ANDI,op::ORI,op::XORI,op::INC,op::DEC,op::CMP,op::CMPI];
    #[test]fn arithmetic_control_word_enables_alu_and_register_write(){let i=decode(op::ADDI).unwrap();assert_eq!(i.operation,MicroOp::AddImmediate);assert!(i.control.alu_enable&&i.control.reg_write);}
    #[test]fn execute_blocks_fill_80_through_fc_without_overlap(){let mut seen=[false;256];for opcode in OPCODES{let base=execute_address(opcode).unwrap();for step in 0..uaddr::EXEC_ROWS{let a=execute_step_address(opcode,step).unwrap();assert_eq!(a,base+step);assert!(!seen[a as usize]);seen[a as usize]=true;}}assert_eq!(execute_step_address(op::HALT,4),Some(uaddr::EXEC_LAST));}
    #[test]fn ldi_and_addi_keep_three_row_programs(){for opcode in[op::LDI,op::ADDI]{let base=execute_address(opcode).unwrap();assert_eq!(execute_row_kind(opcode,0),Some(ExecuteRowKind::Operand));assert_eq!(execute_row_kind(opcode,1),Some(ExecuteRowKind::Operand));assert_eq!(execute_row_kind(opcode,2),Some(ExecuteRowKind::Commit));assert_eq!(execute_row_kind(opcode,3),Some(ExecuteRowKind::Idle));assert_eq!(control_word_at(base+2,opcode).bits(),control_word(opcode).bits());}}
    #[test]fn five_row_alu_has_operand_select_propagate_and_commit_rows(){for opcode in FIVE_ROW_ALU{let base=execute_address(opcode).unwrap();assert_eq!(execute_row_kind(opcode,0),Some(ExecuteRowKind::Operand));assert_eq!(execute_row_kind(opcode,1),Some(ExecuteRowKind::Operand));assert_eq!(execute_row_kind(opcode,2),Some(ExecuteRowKind::AluSelect));assert_eq!(execute_row_kind(opcode,3),Some(ExecuteRowKind::Propagate));assert_eq!(execute_row_kind(opcode,4),Some(ExecuteRowKind::Commit));assert!(control_word_at(base+2,opcode).alu_enable);assert!(control_word_at(base+3,opcode).alu_enable);}}
    #[test]fn ld_and_st_rows_expose_address_memory_and_commit_roles(){assert_eq!(execute_row_kind(op::LD,0),Some(ExecuteRowKind::Operand));assert_eq!(execute_row_kind(op::LD,1),Some(ExecuteRowKind::Address));assert_eq!(execute_row_kind(op::LD,3),Some(ExecuteRowKind::Memory));assert_eq!(execute_row_kind(op::ST,0),Some(ExecuteRowKind::Address));assert_eq!(execute_row_kind(op::ST,3),Some(ExecuteRowKind::Memory));assert!(control_word_at(execute_address(op::LD).unwrap()+4,op::LD).reg_write);}
    #[test]fn branches_use_target_condition_mux_and_pc_commit_rows(){for opcode in[op::JMP,op::JZ,op::JNZ,op::JLT,op::JGE,op::JC]{let base=execute_address(opcode).unwrap();assert_eq!(execute_row_kind(opcode,0),Some(ExecuteRowKind::Address));assert_eq!(execute_row_kind(opcode,1),Some(ExecuteRowKind::Address));assert_eq!(execute_row_kind(opcode,2),Some(ExecuteRowKind::Condition));assert_eq!(execute_row_kind(opcode,3),Some(ExecuteRowKind::PcSelect));assert_eq!(execute_row_kind(opcode,4),Some(ExecuteRowKind::Commit));assert!(control_word_at(base+4,opcode).pc_load);}}
    #[test]fn execute_row_roles_do_not_duplicate_micro_pc_transition_sources(){assert_eq!(execute_row_kind(op::LDI,0),Some(ExecuteRowKind::Operand));assert_eq!(execute_row_kind(0xAA,0),None);}
    #[test]fn microsequencer_has_real_increment_dispatch_and_return_paths(){let mut s=MicroSequencer::default();assert_eq!(s.advance().after,uaddr::FETCH_T1);let exec=execute_address(op::ADDI).unwrap();assert_eq!(s.dispatch(exec).after,exec);assert_eq!(s.call(uaddr::OPERAND_T0).after,uaddr::OPERAND_T0);assert_eq!(s.advance().after,uaddr::OPERAND_T1);assert_eq!(s.advance().after,uaddr::OPERAND_T2);assert_eq!(s.return_from_routine().after,exec);}
    #[test]fn physical_rom_contains_fetch_words(){assert!(control_word_at(uaddr::FETCH_T1,op::ADDI).mem_read);}
    #[test]fn call_and_return_drive_stack_and_pc_mux(){let c=decode(op::CALL).unwrap().control;let r=decode(op::RET).unwrap().control;assert!(c.pc_load&&c.stack_enable&&c.mem_write);assert!(r.pc_load&&r.stack_enable&&r.mem_read);}
    #[test]fn wait_and_halt_have_distinct_control_lines(){assert!(decode(op::WAIT_VBLANK).unwrap().control.wait);assert!(decode(op::HALT).unwrap().control.halt);}
}
