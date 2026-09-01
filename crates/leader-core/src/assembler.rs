use std::collections::HashMap;

use crate::isa::{op, Reg};

#[derive(Debug, Clone)]
enum FixupKind {
    Address16,
}

#[derive(Debug, Clone)]
struct Fixup {
    offset: usize,
    label: String,
    kind: FixupKind,
}

#[derive(Debug, Clone, Default)]
pub struct Assembler {
    bytes: Vec<u8>,
    labels: HashMap<String, u16>,
    fixups: Vec<Fixup>,
}

impl Assembler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn position(&self) -> u16 {
        self.bytes.len() as u16
    }

    pub fn label(&mut self, name: &str) {
        let old = self.labels.insert(name.to_owned(), self.position());
        assert!(old.is_none(), "duplicate assembler label: {name}");
    }

    pub fn nop(&mut self) { self.byte(op::NOP); }
    pub fn halt(&mut self) { self.byte(op::HALT); }
    pub fn ret(&mut self) { self.byte(op::RET); }
    pub fn wait_vblank(&mut self) { self.byte(op::WAIT_VBLANK); }

    pub fn ldi(&mut self, reg: Reg, value: u8) {
        self.bytes.extend([op::LDI, reg.code(), value]);
    }

    pub fn ld(&mut self, reg: Reg, address: u16) {
        self.bytes.extend([op::LD, reg.code()]);
        self.word(address);
    }

    pub fn st(&mut self, address: u16, reg: Reg) {
        self.byte(op::ST);
        self.word(address);
        self.byte(reg.code());
    }

    pub fn mov(&mut self, dst: Reg, src: Reg) {
        self.bytes.extend([op::MOV, dst.code(), src.code()]);
    }

    pub fn add(&mut self, dst: Reg, src: Reg) {
        self.bytes.extend([op::ADD, dst.code(), src.code()]);
    }

    pub fn addi(&mut self, reg: Reg, value: u8) {
        self.bytes.extend([op::ADDI, reg.code(), value]);
    }

    pub fn subi(&mut self, reg: Reg, value: u8) {
        self.bytes.extend([op::SUBI, reg.code(), value]);
    }

    pub fn andi(&mut self, reg: Reg, value: u8) {
        self.bytes.extend([op::ANDI, reg.code(), value]);
    }

    pub fn ori(&mut self, reg: Reg, value: u8) {
        self.bytes.extend([op::ORI, reg.code(), value]);
    }

    pub fn xori(&mut self, reg: Reg, value: u8) {
        self.bytes.extend([op::XORI, reg.code(), value]);
    }

    pub fn inc(&mut self, reg: Reg) { self.bytes.extend([op::INC, reg.code()]); }
    pub fn dec(&mut self, reg: Reg) { self.bytes.extend([op::DEC, reg.code()]); }
    pub fn cmp(&mut self, lhs: Reg, rhs: Reg) { self.bytes.extend([op::CMP, lhs.code(), rhs.code()]); }
    pub fn cmpi(&mut self, reg: Reg, value: u8) { self.bytes.extend([op::CMPI, reg.code(), value]); }

    pub fn jmp(&mut self, label: &str) { self.branch(op::JMP, label); }
    pub fn jz(&mut self, label: &str) { self.branch(op::JZ, label); }
    pub fn jnz(&mut self, label: &str) { self.branch(op::JNZ, label); }
    pub fn jlt(&mut self, label: &str) { self.branch(op::JLT, label); }
    pub fn jge(&mut self, label: &str) { self.branch(op::JGE, label); }
    pub fn jc(&mut self, label: &str) { self.branch(op::JC, label); }
    pub fn call(&mut self, label: &str) { self.branch(op::CALL, label); }

    pub fn finish(mut self) -> Vec<u8> {
        for fixup in self.fixups {
            let address = *self.labels.get(&fixup.label).unwrap_or_else(|| {
                panic!("unknown assembler label: {}", fixup.label)
            });
            match fixup.kind {
                FixupKind::Address16 => {
                    let [lo, hi] = address.to_le_bytes();
                    self.bytes[fixup.offset] = lo;
                    self.bytes[fixup.offset + 1] = hi;
                }
            }
        }
        self.bytes
    }

    fn branch(&mut self, opcode: u8, label: &str) {
        self.byte(opcode);
        let offset = self.bytes.len();
        self.word(0);
        self.fixups.push(Fixup {
            offset,
            label: label.to_owned(),
            kind: FixupKind::Address16,
        });
    }

    fn byte(&mut self, value: u8) { self.bytes.push(value); }
    fn word(&mut self, value: u16) { self.bytes.extend(value.to_le_bytes()); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_forward_labels() {
        let mut asm = Assembler::new();
        asm.jmp("later");
        asm.nop();
        asm.label("later");
        asm.halt();
        let rom = asm.finish();
        assert_eq!(rom[0], op::JMP);
        assert_eq!(u16::from_le_bytes([rom[1], rom[2]]), 4);
        assert_eq!(rom[4], op::HALT);
    }
}
