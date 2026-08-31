use crate::topology::{Link, SignalKind, Topology};

/// Injects the missing internal wires for the visible eight-slice ALU.
///
/// The base topology already contains the principal SUM→RESULT,
/// GENERATE→CARRY and CARRY→next-SUM links. This pass completes the rest of
/// the physical full-adder network so renderers can traverse the actual gate
/// graph instead of inventing presentation-only connections.
pub fn inject_alu_wiring(topology: &mut Topology) {
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
            "B",
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
            "B",
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
    fn every_slice_has_complete_visible_full_adder_wiring() {
        let mut topology = crate::topology::build_topology();
        inject_alu_wiring(&mut topology);

        for bit in 0..8 {
            for id in [
                format!("alu-a-xor-{bit}"),
                format!("alu-b-xor-{bit}"),
                format!("alu-a-gen-{bit}"),
                format!("alu-b-gen-{bit}"),
                format!("alu-xor-sum-{bit}"),
                format!("alu-xor-prop-{bit}"),
                format!("alu-prop-carry-{bit}"),
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
        let once = topology.links.len();
        inject_alu_wiring(&mut topology);
        assert_eq!(topology.links.len(), once);
    }
}
