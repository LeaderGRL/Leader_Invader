use crate::trace::PhaseKind;

pub mod op {
    pub const NOP:u8=0x00;pub const LDI:u8=0x10;pub const LD:u8=0x11;pub const ST:u8=0x12;pub const MOV:u8=0x13;
    pub const ADD:u8=0x20;pub const ADDI:u8=0x21;pub const SUBI:u8=0x22;pub const ANDI:u8=0x23;pub const ORI:u8=0x24;pub const XORI:u8=0x25;pub const INC:u8=0x26;pub const DEC:u8=0x27;pub const CMP:u8=0x28;pub const CMPI:u8=0x29;
    pub const JMP:u8=0x30;pub const JZ:u8=0x31;pub const JNZ:u8=0x32;pub const JLT:u8=0x33;pub const JGE:u8=0x34;pub const JC:u8=0x35;pub const CALL:u8=0x36;pub const RET:u8=0x37;
    pub const WAIT_VBLANK:u8=0xFE;pub const HALT:u8=0xFF;
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]#[repr(u8)]
pub enum Reg{A=0,B=1,C=2,D=3,X=4,Y=5,T=6,U=7}
impl Reg{#[must_use]pub const fn code(self)->u8{self as u8}fn index(v:u8)->Option<usize>{(v<8).then_some(v as usize)}}

#[derive(Debug,Clone,Copy,Default,PartialEq,Eq)]pub struct Flags{pub zero:bool,pub carry:bool,pub less:bool}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]pub enum StepOutcome{Continue,WaitVBlank,Halted,Fault{pc:u16,opcode:u8}}

pub trait Bus{
 fn fetch8(&mut self,pc:u16)->u8;fn read8(&mut self,pc:u16,address:u16)->u8;fn write8(&mut self,pc:u16,address:u16,value:u8);
 fn trace_decode(&mut self,pc:u16,opcode:u8,mnemonic:&'static str);fn trace_alu(&mut self,pc:u16,value:u8,control:&'static str);fn trace_control(&mut self,pc:u16,control:&'static str);
}

#[derive(Debug,Clone,PartialEq,Eq)]pub struct Cpu{regs:[u8;8],pc:u16,sp:u16,flags:Flags,halted:bool}
impl Default for Cpu{fn default()->Self{Self{regs:[0;8],pc:0,sp:0x7fff,flags:Flags::default(),halted:false}}}
impl Cpu{
 #[must_use]pub const fn pc(&self)->u16{self.pc}#[must_use]pub const fn sp(&self)->u16{self.sp}#[must_use]pub const fn flags(&self)->Flags{self.flags}#[must_use]pub fn reg(&self,r:Reg)->u8{self.regs[r as usize]}
 pub fn step<B:Bus>(&mut self,b:&mut B)->StepOutcome{
  if self.halted{return StepOutcome::Halted}let pc=self.pc;let opv=self.next8(b);b.trace_decode(pc,opv,mnemonic(opv));
  match opv{
   op::NOP=>StepOutcome::Continue,
   op::LDI=>{let Some(r)=self.reg_index(b,pc,opv)else{return self.fault(pc,opv)};let v=self.next8(b);self.regs[r]=v;self.zero(v);b.trace_alu(pc,v,"LDI");StepOutcome::Continue},
   op::LD=>{let Some(r)=self.reg_index(b,pc,opv)else{return self.fault(pc,opv)};let a=self.next16(b);let v=b.read8(pc,a);self.regs[r]=v;self.zero(v);StepOutcome::Continue},
   op::ST=>{let a=self.next16(b);let Some(r)=self.reg_index(b,pc,opv)else{return self.fault(pc,opv)};b.write8(pc,a,self.regs[r]);StepOutcome::Continue},
   op::MOV=>{let Some(d)=self.reg_index(b,pc,opv)else{return self.fault(pc,opv)};let Some(s)=self.reg_index(b,pc,opv)else{return self.fault(pc,opv)};self.regs[d]=self.regs[s];self.zero(self.regs[d]);b.trace_alu(pc,self.regs[d],"MOV");StepOutcome::Continue},
   op::ADD=>{let Some(d)=self.reg_index(b,pc,opv)else{return self.fault(pc,opv)};let Some(s)=self.reg_index(b,pc,opv)else{return self.fault(pc,opv)};let(v,c)=self.regs[d].overflowing_add(self.regs[s]);self.regs[d]=v;self.flags=Flags{zero:v==0,carry:c,less:false};b.trace_alu(pc,v,"ADD");StepOutcome::Continue},
   op::ADDI=>self.imm_alu(b,pc,opv,"ADDI",|a,v|a.overflowing_add(v)),
   op::SUBI=>self.imm_alu(b,pc,opv,"SUBI",|a,v|{let(x,borrow)=a.overflowing_sub(v);(x,!borrow)}),
   op::ANDI=>self.logic(b,pc,opv,"ANDI",|a,v|a&v),op::ORI=>self.logic(b,pc,opv,"ORI",|a,v|a|v),op::XORI=>self.logic(b,pc,opv,"XORI",|a,v|a^v),
   op::INC=>self.one(b,pc,opv,"INC",true),op::DEC=>self.one(b,pc,opv,"DEC",false),
   op::CMP=>{let Some(a)=self.reg_index(b,pc,opv)else{return self.fault(pc,opv)};let Some(c)=self.reg_index(b,pc,opv)else{return self.fault(pc,opv)};self.compare(self.regs[a],self.regs[c]);b.trace_alu(pc,self.regs[a].wrapping_sub(self.regs[c]),"CMP");StepOutcome::Continue},
   op::CMPI=>{let Some(r)=self.reg_index(b,pc,opv)else{return self.fault(pc,opv)};let v=self.next8(b);self.compare(self.regs[r],v);b.trace_alu(pc,self.regs[r].wrapping_sub(v),"CMPI");StepOutcome::Continue},
   op::JMP=>{self.pc=self.next16(b);b.trace_control(pc,"JMP");StepOutcome::Continue},
   op::JZ=>self.branch(b,pc,self.flags.zero,"JZ"),op::JNZ=>self.branch(b,pc,!self.flags.zero,"JNZ"),op::JLT=>self.branch(b,pc,self.flags.less,"JLT"),op::JGE=>self.branch(b,pc,!self.flags.less,"JGE"),op::JC=>self.branch(b,pc,self.flags.carry,"JC"),
   op::CALL=>{let target=self.next16(b);let ret=self.pc;self.push(b,pc,(ret>>8)as u8);self.push(b,pc,ret as u8);self.pc=target;b.trace_control(pc,"CALL");StepOutcome::Continue},
   op::RET=>{let lo=self.pop(b,pc);let hi=self.pop(b,pc);self.pc=u16::from_le_bytes([lo,hi]);b.trace_control(pc,"RET");StepOutcome::Continue},
   op::WAIT_VBLANK=>{b.trace_control(pc,"WAIT_VBLANK");StepOutcome::WaitVBlank},op::HALT=>{self.halted=true;b.trace_control(pc,"HALT");StepOutcome::Halted},_=>self.fault(pc,opv)
  }
 }
 fn next8<B:Bus>(&mut self,b:&mut B)->u8{let v=b.fetch8(self.pc);self.pc=self.pc.wrapping_add(1);v}fn next16<B:Bus>(&mut self,b:&mut B)->u16{let lo=self.next8(b);let hi=self.next8(b);u16::from_le_bytes([lo,hi])}
 fn reg_index<B:Bus>(&mut self,b:&mut B,_pc:u16,_op:u8)->Option<usize>{Reg::index(self.next8(b))}
 fn imm_alu<B:Bus,F:FnOnce(u8,u8)->(u8,bool)>(&mut self,b:&mut B,pc:u16,opv:u8,name:&'static str,f:F)->StepOutcome{let Some(r)=self.reg_index(b,pc,opv)else{return self.fault(pc,opv)};let imm=self.next8(b);let(v,c)=f(self.regs[r],imm);self.regs[r]=v;self.flags=Flags{zero:v==0,carry:c,less:!c};b.trace_alu(pc,v,name);StepOutcome::Continue}
 fn logic<B:Bus,F:FnOnce(u8,u8)->u8>(&mut self,b:&mut B,pc:u16,opv:u8,name:&'static str,f:F)->StepOutcome{let Some(r)=self.reg_index(b,pc,opv)else{return self.fault(pc,opv)};let imm=self.next8(b);let v=f(self.regs[r],imm);self.regs[r]=v;self.flags=Flags{zero:v==0,carry:false,less:false};b.trace_alu(pc,v,name);StepOutcome::Continue}
 fn one<B:Bus>(&mut self,b:&mut B,pc:u16,opv:u8,name:&'static str,inc:bool)->StepOutcome{let Some(r)=self.reg_index(b,pc,opv)else{return self.fault(pc,opv)};let(v,c)=if inc{self.regs[r].overflowing_add(1)}else{let(x,borrow)=self.regs[r].overflowing_sub(1);(x,!borrow)};self.regs[r]=v;self.flags=Flags{zero:v==0,carry:c,less:!c};b.trace_alu(pc,v,name);StepOutcome::Continue}
 fn compare(&mut self,a:u8,b:u8){self.flags=Flags{zero:a==b,carry:a>=b,less:a<b}}fn branch<B:Bus>(&mut self,b:&mut B,pc:u16,yes:bool,name:&'static str)->StepOutcome{let target=self.next16(b);if yes{self.pc=target}b.trace_control(pc,name);StepOutcome::Continue}
 fn push<B:Bus>(&mut self,b:&mut B,pc:u16,v:u8){self.sp=self.sp.wrapping_sub(1);b.write8(pc,self.sp,v)}fn pop<B:Bus>(&mut self,b:&mut B,pc:u16)->u8{let v=b.read8(pc,self.sp);self.sp=self.sp.wrapping_add(1);v}
 fn zero(&mut self,v:u8){self.flags.zero=v==0;self.flags.less=false}fn fault(&mut self,pc:u16,opcode:u8)->StepOutcome{self.halted=true;StepOutcome::Fault{pc,opcode}}
}

#[must_use]pub const fn mnemonic(v:u8)->&'static str{match v{op::NOP=>"NOP",op::LDI=>"LDI",op::LD=>"LD",op::ST=>"ST",op::MOV=>"MOV",op::ADD=>"ADD",op::ADDI=>"ADDI",op::SUBI=>"SUBI",op::ANDI=>"ANDI",op::ORI=>"ORI",op::XORI=>"XORI",op::INC=>"INC",op::DEC=>"DEC",op::CMP=>"CMP",op::CMPI=>"CMPI",op::JMP=>"JMP",op::JZ=>"JZ",op::JNZ=>"JNZ",op::JLT=>"JLT",op::JGE=>"JGE",op::JC=>"JC",op::CALL=>"CALL",op::RET=>"RET",op::WAIT_VBLANK=>"WAIT_VBLANK",op::HALT=>"HALT",_=>"FAULT"}}
#[must_use]pub const fn phase_for_opcode(v:u8)->PhaseKind{match v{op::LD=>PhaseKind::MemoryRead,op::ST=>PhaseKind::MemoryWrite,op::ADD|op::ADDI|op::SUBI|op::ANDI|op::ORI|op::XORI|op::INC|op::DEC|op::CMP|op::CMPI=>PhaseKind::Alu,_=>PhaseKind::Decode}}

#[cfg(test)]mod tests{use super::*;struct T{m:Vec<u8>}impl Default for T{fn default()->Self{Self{m:vec![0;256]}}}impl Bus for T{fn fetch8(&mut self,p:u16)->u8{self.m[p as usize]}fn read8(&mut self,_:u16,a:u16)->u8{self.m[a as usize]}fn write8(&mut self,_:u16,a:u16,v:u8){self.m[a as usize]=v}fn trace_decode(&mut self,_:u16,_:u8,_:&'static str){}fn trace_alu(&mut self,_:u16,_:u8,_:&'static str){}fn trace_control(&mut self,_:u16,_:&'static str){}}
 #[test]fn load_add_store(){let mut b=T::default();let p=[op::LDI,Reg::A.code(),4,op::ADDI,Reg::A.code(),6,op::ST,0x80,0,Reg::A.code(),op::HALT];b.m[..p.len()].copy_from_slice(&p);let mut c=Cpu::default();for _ in 0..4{c.step(&mut b);}assert_eq!(b.m[0x80],10)} }
