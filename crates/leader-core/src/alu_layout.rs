use crate::topology::{Link, Node, Rect, SignalKind, Topology};

/// Injects the internal wiring and function-selection gates for the visible
/// eight-slice ALU.
///
/// The historical topology already contains the full-adder gates
/// (`xorA/xorB/andA/andB/orC`) and the result mux. This pass completes both the
/// arithmetic ripple network and the logical result paths so every native ALU
/// operation has a physical route into `muxR` rather than relying on a
/// renderer-side interpretation.
pub fn inject_alu_wiring(topology: &mut Topology) {
    inject_logic_nodes(topology);

    for bit in 0..8 {
        add_link(
            topology,
            &format!("alu-a-xor-{bit}"),
            "readMuxA",
            &format!("xorA{bit}"),
            SignalKind::Data,
            "A",
        );
        add_link(
            topology,
            &format!("alu-b-xor-{bit}"),
            "readMuxB",
            &format!("xorA{bit}"),
            SignalKind::Data,
            "B/EFF",
        );
        add_link(
            topology,
            &format!("alu-a-gen-{bit}"),
            "readMuxA",
            &format!("andA{bit}"),
            SignalKind::Data,
            "A",
        );
        add_link(
            topology,
            &format!("alu-b-gen-{bit}"),
            "readMuxB",
            &format!("andA{bit}"),
            SignalKind::Data,
            "B/EFF",
        );
        add_link(
            topology,
            &format!("alu-xor-sum-{bit}"),
            &format!("xorA{bit}"),
            &format!("xorB{bit}"),
            SignalKind::Data,
            "A XOR B",
        );
        add_link(
            topology,
            &format!("alu-xor-prop-{bit}"),
            &format!("xorA{bit}"),
            &format!("andB{bit}"),
            SignalKind::Data,
            "PROP",
        );
        add_link(
            topology,
            &format!("alu-prop-carry-{bit}"),
            &format!("andB{bit}"),
            &format!("orC{bit}"),
            SignalKind::Carry,
            "CARRY",
        );

        // Logical function gates consume the architectural operands directly.
        add_link(
            topology,
            &format!("alu-a-pass-{bit}"),
            "readMuxA",
            &format!("passR{bit}"),
            SignalKind::Data,
            "A",
        );
        add_link(
            topology,
            &format!("alu-a-and-logic-{bit}"),
            "readMuxA",
            &format!("logicAnd{bit}"),
            SignalKind::Data,
            "A",
        );
        add_link(
            topology,
            &format!("alu-b-and-logic-{bit}"),
            "readMuxB",
            &format!("logicAnd{bit}"),
            SignalKind::Data,
            "B",
        );
        add_link(
            topology,
            &format!("alu-a-or-logic-{bit}"),
            "readMuxA",
            &format!("logicOr{bit}"),
            SignalKind::Data,
            "A",
        );
        add_link(
            topology,
            &format!("alu-b-or-logic-{bit}"),
            "readMuxB",
            &format!("logicOr{bit}"),
            SignalKind::Data,
            "B",
        );

        // All five physical function results terminate at the same result mux.
        for (source, suffix, label) in [
            (format!("passR{bit}"), "pass", "PASS"),
            (format!("logicAnd{bit}"), "and", "AND"),
            (format!("logicOr{bit}"), "or", "OR"),
            (format!("xorA{bit}"), "xor", "XOR"),
            (format!("xorB{bit}"), "sum", "SUM"),
        ] {
            add_link(
                topology,
                &format!("alu-{suffix}-result-{bit}"),
                &source,
                &format!("muxR{bit}"),
                SignalKind::Data,
                label,
            );
        }
        add_link(
            topology,
            &format!("alu-select-result-{bit}"),
            "aluSel",
            &format!("muxR{bit}"),
            SignalKind::Control,
            "OP",
        );
        add_link(
            topology,
            &format!("alu-result-write-{bit}"),
            &format!("muxR{bit}"),
            "writeBus",
            SignalKind::Data,
            "R",
        );

        if bit == 0 {
            add_link(
                topology,
                "alu-cin-sum-0",
                "aluSel",
                "xorB0",
                SignalKind::Control,
                "CIN",
            );
            add_link(
                topology,
                "alu-cin-prop-0",
                "aluSel",
                "andB0",
                SignalKind::Control,
                "CIN",
            );
        } else {
            add_link(
                topology,
                &format!("alu-carry-prop-{bit}"),
                &format!("orC{}", bit - 1),
                &format!("andB{bit}"),
                SignalKind::Carry,
                "CIN",
            );
        }
    }
}

fn inject_logic_nodes(topology: &mut Topology) {
    for bit in 0..8 {
        let y = 815.0 + bit as f32 * 94.0;
        for (prefix, title, kind, x) in [
            ("passR", "PASS", "BUF", 2800.0),
            ("logicAnd", "AND", "AND", 2870.0),
            ("logicOr", "OR", "OR", 2940.0),
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
                bounds: Rect::new(x, y, 62.0, 54.0),
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
                format!("passR{bit}"),
                format!("logicAnd{bit}"),
                format!("logicOr{bit}"),
            ] {
                assert!(topology.node(&node).is_some(), "{node}");
            }

            for id in [
                format!("alu-a-xor-{bit}"),
                format!("alu-b-xor-{bit}"),
                format!("alu-a-gen-{bit}"),
                format!("alu-b-gen-{bit}"),
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
