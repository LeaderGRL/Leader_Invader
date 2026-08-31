use crate::logic::{AluOp, AluTrace};

/// One authoritative one-bit value travelling over a physical ALU link.
///
/// `rank` is a dependency order, not an analog delay. Equal ranks may settle in
/// parallel. `selected` marks links that participate in the native operation;
/// unselected entries remain valid combinational candidate values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalAluLinkValue {
    pub link_id: String,
    pub bit: u8,
    pub rank: u8,
    pub stage: &'static str,
    pub value: bool,
    pub selected: bool,
}

/// Materializes a dependency-ordered propagation model from one native ALU
/// trace. Frontends may animate these ranks, but must not derive gate/link
/// values or operation selection independently.
#[must_use]
pub fn physical_alu_link_values(trace: AluTrace) -> Vec<PhysicalAluLinkValue> {
    let mut values = Vec::with_capacity(192);
    let is_sub = matches!(trace.op, AluOp::Sub | AluOp::Compare);
    let selected_family = selected_family(trace.op);
    let uses_adder = selected_family == "sum";
    let uses_xor_path = uses_adder || selected_family == "xor";
    let writes_back = trace.op != AluOp::Compare;

    for bit in 0..8_u8 {
        let shift = u32::from(bit);
        let lhs = trace.lhs & (1 << shift) != 0;
        let rhs = trace.rhs & (1 << shift) != 0;
        let rhs_effective = trace.rhs_effective & (1 << shift) != 0;
        let xor_ab = lhs ^ rhs_effective;
        let carry_in = trace.carry_in(usize::from(bit));
        let sum = xor_ab ^ carry_in;
        let generate = lhs & rhs_effective;
        let propagate = xor_ab & carry_in;
        let carry_out = trace.carry_out(usize::from(bit));
        let pass = lhs;
        let logical_and = lhs & rhs;
        let logical_or = lhs | rhs;
        let result = trace.result & (1 << shift) != 0;

        push(&mut values, bit, 0, "operand_a", lhs, uses_xor_path, format!("alu-a-xor-{bit}"));
        push(&mut values, bit, 0, "operand_a", lhs, uses_adder, format!("alu-a-gen-{bit}"));
        push(&mut values, bit, 0, "operand_b", rhs, uses_xor_path, format!("alu-b-rhs-xor-{bit}"));
        push(&mut values, bit, 0, "sub_control", is_sub, uses_xor_path, format!("alu-sub-rhs-xor-{bit}"));
        push(&mut values, bit, 0, "pass_input", lhs, selected_family == "pass", format!("alu-a-pass-{bit}"));
        push(&mut values, bit, 0, "and_input_a", lhs, selected_family == "and", format!("alu-a-and-logic-{bit}"));
        push(&mut values, bit, 0, "and_input_b", rhs, selected_family == "and", format!("alu-b-and-logic-{bit}"));
        push(&mut values, bit, 0, "or_input_a", lhs, selected_family == "or", format!("alu-a-or-logic-{bit}"));
        push(&mut values, bit, 0, "or_input_b", rhs, selected_family == "or", format!("alu-b-or-logic-{bit}"));

        push(&mut values, bit, 1, "rhs_effective", rhs_effective, uses_xor_path, format!("alu-rhs-xor-sum-{bit}"));
        push(&mut values, bit, 1, "rhs_effective", rhs_effective, uses_adder, format!("alu-rhs-gen-{bit}"));
        push(&mut values, bit, 1, "pass", pass, selected_family == "pass", format!("alu-pass-result-{bit}"));
        push(&mut values, bit, 1, "logical_and", logical_and, selected_family == "and", format!("alu-and-result-{bit}"));
        push(&mut values, bit, 1, "logical_or", logical_or, selected_family == "or", format!("alu-or-result-{bit}"));

        push(&mut values, bit, 2, "xor_ab", xor_ab, uses_adder, format!("alu-xor-sum-{bit}"));
        push(&mut values, bit, 2, "xor_ab", xor_ab, uses_adder, format!("alu-xor-prop-{bit}"));
        push(&mut values, bit, 2, "logical_xor", xor_ab, selected_family == "xor", format!("alu-xor-result-{bit}"));
        push(&mut values, bit, 2, "generate", generate, uses_adder, format!("ac{bit}"));

        let carry_rank = 3 + bit.saturating_mul(2);
        if bit == 0 {
            push(&mut values, bit, carry_rank, "carry_in", carry_in, uses_adder, "alu-cin-sum-0".to_owned());
            push(&mut values, bit, carry_rank, "carry_in", carry_in, uses_adder, "alu-cin-prop-0".to_owned());
        } else {
            push(&mut values, bit, carry_rank, "carry_in", carry_in, uses_adder, format!("cc{}", bit - 1));
            push(&mut values, bit, carry_rank, "carry_in", carry_in, uses_adder, format!("alu-carry-prop-{bit}"));
        }
        push(&mut values, bit, carry_rank, "sum", sum, uses_adder, format!("alu-sum-result-{bit}"));
        push(&mut values, bit, carry_rank, "propagate", propagate, uses_adder, format!("alu-prop-carry-{bit}"));

        let carry_link = if bit < 7 { format!("cc{bit}") } else { "alu-carry-flag".to_owned() };
        push(&mut values, bit, carry_rank + 1, "carry_out", carry_out, uses_adder, carry_link);

        let result_rank = if uses_adder { carry_rank + 1 } else { 3 };
        push(
            &mut values,
            bit,
            result_rank,
            "result",
            result,
            writes_back,
            format!("alu-result-write-{bit}"),
        );
    }

    values.sort_by(|left, right| {
        left.rank
            .cmp(&right.rank)
            .then(left.bit.cmp(&right.bit))
            .then(left.link_id.cmp(&right.link_id))
    });
    values
}

fn selected_family(op: AluOp) -> &'static str {
    match op {
        AluOp::Pass => "pass",
        AluOp::And => "and",
        AluOp::Or => "or",
        AluOp::Xor => "xor",
        AluOp::Add | AluOp::Sub | AluOp::Compare => "sum",
    }
}

fn push(
    values: &mut Vec<PhysicalAluLinkValue>,
    bit: u8,
    rank: u8,
    stage: &'static str,
    value: bool,
    selected: bool,
    link_id: String,
) {
    values.push(PhysicalAluLinkValue {
        link_id,
        bit,
        rank,
        stage,
        value,
        selected,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::{logic_trace, ripple_add, ripple_sub};

    #[test]
    fn add_propagation_orders_ripple_carry_before_later_sum() {
        let values = physical_alu_link_values(ripple_add(0x0f, 1, false, AluOp::Add));
        let carry0 = values
            .iter()
            .find(|value| value.link_id == "cc0" && value.stage == "carry_out")
            .unwrap();
        let carry_into_1 = values
            .iter()
            .find(|value| value.link_id == "cc0" && value.stage == "carry_in")
            .unwrap();
        let sum1 = values
            .iter()
            .find(|value| value.link_id == "alu-sum-result-1")
            .unwrap();
        assert!(carry0.rank < carry_into_1.rank);
        assert_eq!(carry_into_1.rank, sum1.rank);
        assert!(sum1.selected);
        assert_eq!(carry0.value, carry_into_1.value);
    }

    #[test]
    fn subtraction_exposes_native_rhs_conditioning() {
        let values = physical_alu_link_values(ripple_sub(0, 1, AluOp::Sub));
        let sub = values
            .iter()
            .find(|value| value.link_id == "alu-sub-rhs-xor-0")
            .unwrap();
        let effective = values
            .iter()
            .find(|value| value.link_id == "alu-rhs-xor-sum-0")
            .unwrap();
        assert!(sub.value && sub.selected);
        assert!(!effective.value);
    }

    #[test]
    fn logical_operation_selects_only_its_function_path() {
        let values = physical_alu_link_values(logic_trace(AluOp::Or, 0x80, 0x01, 0x81));
        assert!(values
            .iter()
            .any(|value| value.link_id == "alu-or-result-7" && value.selected && value.value));
        assert!(values
            .iter()
            .any(|value| value.link_id == "alu-and-result-7" && !value.selected));
        assert!(values
            .iter()
            .any(|value| value.link_id == "alu-xor-result-7" && !value.selected));
        assert!(values
            .iter()
            .filter(|value| value.stage == "carry_out")
            .all(|value| !value.selected));
    }

    #[test]
    fn xor_uses_rhs_conditioner_without_enabling_ripple_carry() {
        let values = physical_alu_link_values(logic_trace(AluOp::Xor, 0xaa, 0x0f, 0xa5));
        assert!(values
            .iter()
            .any(|value| value.link_id == "alu-rhs-xor-sum-0" && value.selected));
        assert!(values
            .iter()
            .filter(|value| value.stage == "carry_in")
            .all(|value| !value.selected));
    }

    #[test]
    fn compare_reaches_sum_candidate_without_writeback_selection() {
        let values = physical_alu_link_values(ripple_sub(4, 7, AluOp::Compare));
        assert!(values
            .iter()
            .filter(|value| value.stage == "sum")
            .all(|value| value.selected));
        assert!(values
            .iter()
            .filter(|value| value.stage == "result")
            .all(|value| !value.selected));
    }

    #[test]
    fn final_carry_has_a_real_architectural_link() {
        let values = physical_alu_link_values(ripple_add(0xff, 1, false, AluOp::Add));
        let carry = values
            .iter()
            .find(|value| value.link_id == "alu-carry-flag")
            .unwrap();
        assert!(carry.value && carry.selected);
        assert_eq!(carry.bit, 7);
    }

    #[test]
    fn every_reported_link_exists_in_final_topology() {
        let topology = crate::build_topology();
        let values = physical_alu_link_values(ripple_add(0x55, 0xaa, false, AluOp::Add));
        for value in values {
            assert!(
                topology.links.iter().any(|link| link.id == value.link_id),
                "{}",
                value.link_id
            );
        }
    }
}
