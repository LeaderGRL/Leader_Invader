#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AluOp {
    Pass,
    Add,
    Sub,
    And,
    Or,
    Xor,
    Compare,
}

impl AluOp {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Add => "add",
            Self::Sub => "sub",
            Self::And => "and",
            Self::Or => "or",
            Self::Xor => "xor",
            Self::Compare => "compare",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AluTrace {
    pub op: AluOp,
    pub lhs: u8,
    pub rhs: u8,
    pub rhs_effective: u8,
    pub result: u8,
    pub carry_chain: u16,
}

impl AluTrace {
    #[must_use]
    pub const fn carry_in(self, bit: usize) -> bool {
        self.carry_chain & (1_u16 << bit) != 0
    }

    #[must_use]
    pub const fn carry_out(self, bit: usize) -> bool {
        self.carry_chain & (1_u16 << (bit + 1)) != 0
    }

    #[must_use]
    pub const fn final_carry(self) -> bool {
        self.carry_chain & (1_u16 << 8) != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcIncrementTrace {
    pub before: u16,
    pub after: u16,
    pub carry_chain: u32,
}

impl PcIncrementTrace {
    #[must_use]
    pub const fn carry_in(self, bit: usize) -> bool {
        self.carry_chain & (1_u32 << bit) != 0
    }

    #[must_use]
    pub const fn carry_out(self, bit: usize) -> bool {
        self.carry_chain & (1_u32 << (bit + 1)) != 0
    }

    #[must_use]
    pub const fn low_byte_carry(self) -> bool {
        self.carry_chain & (1_u32 << 8) != 0
    }

    #[must_use]
    pub const fn overflow(self) -> bool {
        self.carry_chain & (1_u32 << 16) != 0
    }
}

/// Exact state of a 16-bit decrement-by-one ripple network.
/// `borrow_chain` bit 0 is the injected decrement request; bit N+1 is the
/// borrow leaving slice N.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decrement16Trace {
    pub before: u16,
    pub after: u16,
    pub borrow_chain: u32,
}

impl Decrement16Trace {
    #[must_use]
    pub const fn borrow_in(self, bit: usize) -> bool {
        self.borrow_chain & (1_u32 << bit) != 0
    }

    #[must_use]
    pub const fn borrow_out(self, bit: usize) -> bool {
        self.borrow_chain & (1_u32 << (bit + 1)) != 0
    }

    #[must_use]
    pub const fn low_byte_borrow(self) -> bool {
        self.borrow_chain & (1_u32 << 8) != 0
    }

    #[must_use]
    pub const fn underflow(self) -> bool {
        self.borrow_chain & (1_u32 << 16) != 0
    }
}

#[must_use]
pub fn ripple_add(lhs: u8, rhs: u8, carry_in: bool, op: AluOp) -> AluTrace {
    let mut result = 0_u8;
    let mut carry = carry_in;
    let mut chain = u16::from(carry_in);

    for bit in 0..8 {
        let a = (lhs >> bit) & 1;
        let b = (rhs >> bit) & 1;
        let c = u8::from(carry);
        let xor_ab = a ^ b;
        let sum = xor_ab ^ c;
        let generate = a & b;
        let propagate = xor_ab & c;
        carry = generate | propagate != 0;
        result |= sum << bit;
        if carry {
            chain |= 1_u16 << (bit + 1);
        }
    }

    AluTrace {
        op,
        lhs,
        rhs,
        rhs_effective: rhs,
        result,
        carry_chain: chain,
    }
}

#[must_use]
pub fn ripple_sub(lhs: u8, rhs: u8, op: AluOp) -> AluTrace {
    let effective = !rhs;
    let mut trace = ripple_add(lhs, effective, true, op);
    trace.rhs = rhs;
    trace.rhs_effective = effective;
    trace
}

#[must_use]
pub fn ripple_increment16(before: u16) -> PcIncrementTrace {
    let mut after = 0_u16;
    let mut carry = true;
    let mut chain = 1_u32;

    for bit in 0..16 {
        let input = (before >> bit) & 1 != 0;
        let output = input ^ carry;
        let carry_out = input & carry;
        if output {
            after |= 1_u16 << bit;
        }
        carry = carry_out;
        if carry {
            chain |= 1_u32 << (bit + 1);
        }
    }

    PcIncrementTrace {
        before,
        after,
        carry_chain: chain,
    }
}

#[must_use]
pub fn ripple_decrement16(before: u16) -> Decrement16Trace {
    let mut after = 0_u16;
    let mut borrow = true;
    let mut chain = 1_u32;

    for bit in 0..16 {
        let input = (before >> bit) & 1 != 0;
        let output = input ^ borrow;
        let borrow_out = !input & borrow;
        if output {
            after |= 1_u16 << bit;
        }
        borrow = borrow_out;
        if borrow {
            chain |= 1_u32 << (bit + 1);
        }
    }

    Decrement16Trace {
        before,
        after,
        borrow_chain: chain,
    }
}

#[must_use]
pub const fn logic_trace(op: AluOp, lhs: u8, rhs: u8, result: u8) -> AluTrace {
    AluTrace {
        op,
        lhs,
        rhs,
        rhs_effective: rhs,
        result,
        carry_chain: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ripple_add_is_semantically_identical_to_byte_addition() {
        for lhs in 0_u8..=255 {
            for rhs in [0_u8, 1, 3, 0x0f, 0x55, 0x80, 0xff] {
                let trace = ripple_add(lhs, rhs, false, AluOp::Add);
                let (expected, carry) = lhs.overflowing_add(rhs);
                assert_eq!(trace.result, expected);
                assert_eq!(trace.final_carry(), carry);
            }
        }
    }

    #[test]
    fn ripple_sub_is_semantically_identical_to_byte_subtraction() {
        for lhs in 0_u8..=255 {
            for rhs in [0_u8, 1, 3, 0x0f, 0x55, 0x80, 0xff] {
                let trace = ripple_sub(lhs, rhs, AluOp::Sub);
                let (expected, borrow) = lhs.overflowing_sub(rhs);
                assert_eq!(trace.result, expected);
                assert_eq!(trace.final_carry(), !borrow);
            }
        }
    }

    #[test]
    fn carry_chain_records_each_full_adder_boundary() {
        let trace = ripple_add(0b0000_1111, 1, false, AluOp::Add);
        assert_eq!(trace.result, 0b0001_0000);
        assert!(!trace.carry_in(0));
        for boundary in 1..=4 {
            assert!(trace.carry_chain & (1_u16 << boundary) != 0);
        }
        assert!(trace.carry_chain & (1_u16 << 5) == 0);
    }

    #[test]
    fn pc_incrementer_matches_wrapping_add_for_all_addresses() {
        for before in 0_u16..=u16::MAX {
            let trace = ripple_increment16(before);
            assert_eq!(trace.after, before.wrapping_add(1));
        }
    }

    #[test]
    fn pc_incrementer_exposes_low_to_high_carry() {
        let trace = ripple_increment16(0x12ff);
        assert_eq!(trace.after, 0x1300);
        assert!(trace.low_byte_carry());
        assert!(!trace.overflow());
    }

    #[test]
    fn decrementer_matches_wrapping_sub_for_all_addresses() {
        for before in 0_u16..=u16::MAX {
            let trace = ripple_decrement16(before);
            assert_eq!(trace.after, before.wrapping_sub(1));
        }
    }

    #[test]
    fn decrementer_exposes_low_to_high_borrow() {
        let trace = ripple_decrement16(0x1200);
        assert_eq!(trace.after, 0x11ff);
        assert!(trace.low_byte_borrow());
        assert!(!trace.underflow());
    }
}
