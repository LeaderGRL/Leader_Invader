use crate::topology::{Link, Node, Rect, SignalKind, Topology};

/// Injects the internal wiring and function-selection gates for the visible
/// eight-slice ALU.
///
/// Arithmetic uses an explicit per-bit RHS conditioning XOR so subtraction is
/// physically represented as `B_eff = B XOR SUB`, with the same SUB control
/// supplying the bit-0 carry-in. Logical gates consume architectural B directly.
pub fn inject_alu_wiring(topology: &mut Topology) {
    inject_function_nodes(topology);

    for bit in 0..8 {
        let rhs_xor = format!("rhsXor{bit}");
        let xor_a = format!("xorA{bit}");
        let xor_b = format!("xorB{bit}");
        let and_a = format!("andA{bit}");
        let and_b = format!("andB{bit}");
        let or_c = format!("orC{bit}");
        let mux_r = format!("muxR{bit}");

        add_link(topology, &format!("alu-a-xor-{bit}"), "readMuxA", &xor_a, SignalKind::Data, "A");
        add_link(topology, &format!("alu-b-rhs-xor-{bit}"), "readMuxB", &rhs_xor, SignalKind::Data, "B");
        add_link(topology, &format!("alu-sub-rhs-xor-{bit}"), "aluSel", &rhs_xor, SignalKind::Control, "SUB");
        add_link(topology, &format!("alu-rhs-xor-sum-{bit}"), &rhs_xor, &xor_a, SignalKind::Data, "B/EFF");
        add_link(topology, &format!("alu-a-gen-{bit}"), "readMuxA", &and_a, SignalKind::Data, "A");
        add_link(topology, &format!("alu-rhs-gen-{bit}"), &rhs_xor, &and_a, SignalKind::Data, "B/EFF");
        add_link(topology, &format!("alu-xor-sum-{bit}"), &xor_a, &xor_b, SignalKind::Data, "A XOR B/EFF");
        add_link(topology, &format!("alu-xor-prop-{bit}"), &xor_a, &and_b, SignalKind::Data, "PROP");
        add_link(topology, &format!("alu-prop-carry-{bit}"), &and_b, &or_c, SignalKind::Carry, "CARRY");

        // Logical function gates consume the architectural operands directly.
        add_link(topology, &format!("alu-a-pass-{bit}"), "readMuxA", &format!("passR{bit}"), SignalKind::Data, "A");
        add_link(topology, &format!("alu-a-and-logic-{bit}"), "readMuxA", &format!("logicAnd{bit}"), SignalKind::Data, "A");
        add_link(topology, &format!("alu-b-and-logic-{bit}"), "readMuxB", &format!("logicAnd{bit}"), SignalKind::Data, "B");
        add_link(topology, &format!("alu-a-or-logic-{bit}"), "readMuxA", &format!("logicOr{bit}"), SignalKind::Data, "A");
        add_link(topology, &format!("alu-b-or-logic-{bit}"), "readMuxB", &format!("logicOr{bit}"), SignalKind::Data, "B");

        // Every native result family terminates at the same physical result mux.
        for (source, suffix, label) in [
            (format!("passR{bit}"), "pass", "PASS"),
            (format!("logicAnd{bit}"), "and", "AND"),
            (format!("logicOr{bit}"), "or", "OR"),
            (xor_a.clone(), "xor", "XOR"),
            (xor_b.clone(), "sum", "SUM"),
        ] {
            add_link(topology, &format!("alu-{suffix}-result-{bit}"), &source, &mux_r, SignalKind::Data, label);
        }
        add_link(topology, &format!("alu-select-result-{bit}"), "aluSel", &mux_r, SignalKind::Control, "OP");
        add_link(topology, &format!("alu-result-write-{bit}"), &mux_r, "writeBus", SignalKind::Data, "R");

        if bit == 0 {
            add_link(topology, "alu-cin-sum-0", "aluSel", "xorB0", SignalKind::Control, "CIN/SUB");
            add_link(topology, "alu-cin-prop-0", "aluSel", "andB0", SignalKind::Control, "CIN/SUB");
        } else {
            let previous_carry = format!("orC{}", bit - 1);
            add_link(topology, &format!("alu-carry-prop-{bit}"), &previous_carry, &and_b, SignalKind::Carry, "CIN");
        }
    }
}

fn inject_function_nodes(topology: &mut Topology) {
    for bit in 0..8 {
        let y = 815.0 + bit as f32 * 94.0;
        for (prefix, title, kind, x, width) in [
            ("rhsXor", "B XOR SUB", "XOR", 1988.0, 54.0),
            ("passR", "PASS", "BUF", 2800.0, 62.0),
            ("logicAnd", "AND", "AND", 2870.0, 62.0),
            ("logicOr", "OR", "OR", 2940.0, 62.0),
        ] {
            let id = format!("{prefix}{bit}");
            if topology.node(&id).is_some() {
                continue;
            }
            topology.nodes.push(Node {
                id,
                title: format!("{title} {bit}"),
                kind: kind.to_owned(),
                group: "alu".to_owned(),
                bounds: Rect::new(x, y, width, 54.0),
            });
        }
    }
}

fn add_link(
    topology: &mut Topology,
    id: &str,
    from: &str,
    to: &str,
    signal: SignalKind,
    label: &str,
) {
    if topology.links.iter().any(|link| link.id == id) {
        return;
    }
    debug_assert!(topology.node(from).is_some(), "missing ALU source node {from}");
    debug_assert!(topology.node(to).is_some(), "missing ALU target node {to}");
    topology.links.push(Link {
        id: id.to_owned(),
        from: from.to_owned(),
        to: to.to_owned(),
        signal,
        label: label.to_owned(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_slice_has_complete_visible_alu_wiring() {
        let mut topology = crate::topology::build_topology();
        inject_alu_wiring(&mut topology);

        for bit in 0..8 {
            for node in [
                format!("rhsXor{bit}"),
                format!("passR{bit}"),
                format!("logicAnd{bit}"),
                format!("logicOr{bit}"),
            ] {
                assert!(topology.node(&node).is_some(), "{node}");
            }

            for id in [
                format!("alu-a-xor-{bit}"),
                format!("alu-b-rhs-xor-{bit}"),
                format!("alu-sub-rhs-xor-{bit}"),
                format!("alu-rhs-xor-sum-{bit}"),
                format!("alu-a-gen-{bit}"),
                format!("alu-rhs-gen-{bit}"),
                format!("alu-xor-sum-{bit}"),
                format!("alu-xor-prop-{bit}"),
                format!("alu-prop-carry-{bit}"),
                format!("alu-a-pass-{bit}"),
                format!("alu-a-and-logic-{bit}"),
                format!("alu-b-and-logic-{bit}"),
                format!("alu-a-or-logic-{bit}"),
                format!("alu-b-or-logic-{bit}"),
                format!("alu-pass-result-{bit}"),
                format!("alu-and-result-{bit}"),
                format!("alu-or-result-{bit}"),
                format!("alu-xor-result-{bit}"),
                format!("alu-sum-result-{bit}"),
                format!("alu-select-result-{bit}"),
                format!("alu-result-write-{bit}"),
            ] {
                assert!(topology.links.iter().any(|link| link.id == id), "{id}");
            }
        }

        assert!(topology.links.iter().any(|link| link.id == "alu-cin-sum-0"));
        assert!(topology.links.iter().any(|link| link.id == "alu-cin-prop-0"));
        for bit in 1..8 {
            let id = format!("alu-carry-prop-{bit}");
            assert!(topology.links.iter().any(|link| link.id == id), "{id}");
        }
    }

    #[test]
    fn subtraction_rhs_conditioning_is_explicit() {
        let mut topology = crate::topology::build_topology();
        inject_alu_wiring(&mut topology);
        for bit in 0..8 {
            let rhs = format!("rhsXor{bit}");
            assert!(topology.links.iter().any(|link| link.from == "readMuxB" && link.to == rhs));
            assert!(topology.links.iter().any(|link| link.from == "aluSel" && link.to == rhs));
            assert!(topology.links.iter().any(|link| link.from == rhs && link.to == format!("xorA{bit}")));
            assert!(topology.links.iter().any(|link| link.from == rhs && link.to == format!("andA{bit}")));
        }
    }

    #[test]
    fn injected_wiring_is_idempotent() {
        let mut topology = crate::topology::build_topology();
        inject_alu_wiring(&mut topology);
        let nodes_once = topology.nodes.len();
        let links_once = topology.links.len();
        inject_alu_wiring(&mut topology);
        assert_eq!(topology.nodes.len(), nodes_once);
        assert_eq!(topology.links.len(), links_once);
    }
}
