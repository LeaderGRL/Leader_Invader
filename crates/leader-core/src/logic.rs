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

/// Exact logic-level state produced by one 8-bit ALU operation.
///
/// `carry_chain` stores carry-in for bit 0 in bit 0, then carry-out from
/// slice N in bit N+1. Therefore bit 8 is the final carry-out.
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

/// Computes a byte using eight explicit full-adder slices.
///
/// This function is intentionally the semantic arithmetic implementation used by
/// the CPU, not merely a visualization helper. F3 SVG carry activity is generated
/// from this exact chain.
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
}
