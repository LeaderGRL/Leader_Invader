use crate::topology::{Link, Node, Rect, SignalKind, Topology};

pub const INTERNAL_CONTROL_NODES: [(&str, &str, &str); 16] = [
    ("ctrlMarLoad", "MAR", "MAR_LOAD"),
    ("ctrlMdrLoad", "MDR", "MDR_LOAD"),
    ("ctrlIrLoad", "IR", "IR_LOAD"),
    ("ctrlPcInc", "PCI", "PC_INC"),
    ("ctrlOperandA", "OPA", "OPERAND_A_LOAD"),
    ("ctrlOperandB", "OPB", "OPERAND_B_LOAD"),
    ("ctrlAluOpLoad", "AOP", "ALU_OP_LOAD"),
    ("ctrlFlagsLoad", "FLG", "FLAGS_LOAD"),
    ("ctrlAddrLo", "ALO", "ADDR_LO_LOAD"),
    ("ctrlAddrHi", "AHI", "ADDR_HI_LOAD"),
    ("ctrlCondition", "CND", "CONDITION_LOAD"),
    ("ctrlPcSelect", "PCS", "PC_SELECT"),
    ("ctrlRegSelect", "RGS", "REG_SELECT"),
    ("ctrlBusAddress", "ABU", "BUS_ADDRESS_ENABLE"),
    ("ctrlBusData", "DBU", "BUS_DATA_ENABLE"),
    ("ctrlArchCommit", "COM", "ARCH_COMMIT"),
];

pub fn inject_internal_control_lines(topology: &mut Topology) {
    if topology.node(INTERNAL_CONTROL_NODES[0].0).is_some() {
        return;
    }

    for (index, (id, title, label)) in INTERNAL_CONTROL_NODES.iter().enumerate() {
        topology.nodes.push(Node {
            id: (*id).to_owned(),
            title: (*title).to_owned(),
            kind: "µCTRL".to_owned(),
            group: "decode".to_owned(),
            bounds: Rect::new(1250.0 + index as f32 * 40.0, 736.0, 36.0, 26.0),
        });
        topology.links.push(Link {
            id: format!("micro-internal-{index}"),
            from: "microRom".to_owned(),
            to: (*id).to_owned(),
            signal: SignalKind::Control,
            label: (*label).to_owned(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_sixteen_internal_control_outputs_exist_and_fit_decode_group() {
        let mut topology = crate::topology::build_topology();
        crate::layout::apply_visual_layout(&mut topology);
        inject_internal_control_lines(&mut topology);
        let decode = topology.group("decode").expect("decode group").bounds;

        for (id, _, label) in INTERNAL_CONTROL_NODES {
            let node = topology.node(id).unwrap_or_else(|| panic!("missing {id}"));
            assert_eq!(node.kind, "µCTRL");
            assert!(node.bounds.x >= decode.x);
            assert!(node.bounds.y >= decode.y);
            assert!(node.bounds.x + node.bounds.w <= decode.x + decode.w);
            assert!(node.bounds.y + node.bounds.h <= decode.y + decode.h);
            assert!(topology.links.iter().any(|link| {
                link.from == "microRom" && link.to == id && link.label == label
            }));
        }
    }
}
