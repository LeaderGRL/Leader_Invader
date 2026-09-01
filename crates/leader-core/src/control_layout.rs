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

pub const CONTROL_STATE_NODES: [(&str, &str, &str); 5] = [
    ("addrLoLatch", "ADDR LO", "8-bit LATCH"),
    ("addrHiLatch", "ADDR HI", "8-bit LATCH"),
    ("conditionLatch", "COND", "1-bit LATCH"),
    ("pcSelectLatch", "PC SEL", "16-bit MUX SEL"),
    ("regSelectLatch", "REG SEL", "3-bit LATCH"),
];

const CONTROL_CONSUMERS: [(&str, &str, &str); 20] = [
    ("ctrlMarLoad", "marBit0", "MAR_LOAD"),
    ("ctrlMdrLoad", "mdrBit0", "MDR_LOAD"),
    ("ctrlIrLoad", "irBit0", "IR_LOAD"),
    ("ctrlPcInc", "pcIncLo", "PC_INC"),
    ("ctrlOperandA", "readMuxA", "OPERAND_A_LOAD"),
    ("ctrlOperandB", "readMuxB", "OPERAND_B_LOAD"),
    ("ctrlAluOpLoad", "aluSel", "ALU_OP_LOAD"),
    ("ctrlFlagsLoad", "flagZ", "FLAGS_LOAD"),
    ("ctrlFlagsLoad", "flagC", "FLAGS_LOAD"),
    ("ctrlFlagsLoad", "flagN", "FLAGS_LOAD"),
    ("ctrlAddrLo", "addrLoLatch", "ADDR_LO_LOAD"),
    ("ctrlAddrHi", "addrHiLatch", "ADDR_HI_LOAD"),
    ("ctrlCondition", "conditionLatch", "CONDITION_LOAD"),
    ("ctrlPcSelect", "pcSelectLatch", "PC_SELECT"),
    ("pcSelectLatch", "pcMuxLo", "PC_SELECT"),
    ("pcSelectLatch", "pcMuxHi", "PC_SELECT"),
    ("ctrlRegSelect", "regSelectLatch", "REG_SELECT"),
    ("ctrlBusAddress", "addrBuf", "BUS_ADDRESS_ENABLE"),
    ("ctrlBusData", "dataBuf", "BUS_DATA_ENABLE"),
    ("ctrlArchCommit", "writeBus", "ARCH_COMMIT"),
];

const CALL_RETURN_PATH: [(&str, &str, &str, SignalKind); 7] = [
    ("pcMuxLo", "returnDataMux", "RETURN LO", SignalKind::Data),
    ("pcMuxHi", "returnDataMux", "RETURN HI", SignalKind::Data),
    ("ctrlStack", "returnDataMux", "STACK", SignalKind::Control),
    ("returnDataMux", "dataBuf", "RETURN BYTE", SignalKind::Data),
    ("dataBuf", "addrLoLatch", "RET LO", SignalKind::Data),
    ("dataBuf", "addrHiLatch", "RET HI", SignalKind::Data),
    ("returnDataMux", "stackRam", "CALL BYTE", SignalKind::Data),
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

    for (index, (id, title, kind)) in CONTROL_STATE_NODES.iter().enumerate() {
        topology.nodes.push(Node {
            id: (*id).to_owned(),
            title: (*title).to_owned(),
            kind: (*kind).to_owned(),
            group: "decode".to_owned(),
            bounds: Rect::new(1260.0 + index as f32 * 132.0, 816.0, 116.0, 32.0),
        });
    }

    topology.nodes.push(Node {
        id: "returnDataMux".to_owned(),
        title: "RETURN BYTE".to_owned(),
        kind: "PC HI/LO MUX".to_owned(),
        group: "bus".to_owned(),
        bounds: Rect::new(4930.0, 3380.0, 160.0, 70.0),
    });

    for (index, (from, to, label)) in CONTROL_CONSUMERS.iter().enumerate() {
        topology.links.push(Link {
            id: format!("control-consumer-{index}"),
            from: (*from).to_owned(),
            to: (*to).to_owned(),
            signal: SignalKind::Control,
            label: (*label).to_owned(),
        });
    }

    for (index, (from, to, label, signal)) in CALL_RETURN_PATH.iter().enumerate() {
        topology.links.push(Link {
            id: format!("call-return-path-{index}"),
            from: (*from).to_owned(),
            to: (*to).to_owned(),
            signal: *signal,
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

        for (id, _, _) in CONTROL_STATE_NODES {
            let node = topology.node(id).unwrap_or_else(|| panic!("missing {id}"));
            assert!(node.bounds.x >= decode.x);
            assert!(node.bounds.y >= decode.y);
            assert!(node.bounds.x + node.bounds.w <= decode.x + decode.w);
            assert!(node.bounds.y + node.bounds.h <= decode.y + decode.h);
        }
    }

    #[test]
    fn internal_control_outputs_are_wired_to_real_consumers() {
        let mut topology = crate::topology::build_topology();
        crate::layout::apply_visual_layout(&mut topology);
        inject_internal_control_lines(&mut topology);

        for (from, to, label) in CONTROL_CONSUMERS {
            assert!(topology.node(from).is_some(), "missing source {from}");
            assert!(topology.node(to).is_some(), "missing consumer {to}");
            assert!(topology.links.iter().any(|link| {
                link.from == from && link.to == to && link.label == label
            }), "missing control wire {from} -> {to} ({label})");
        }
    }

    #[test]
    fn call_return_path_is_visible_and_closed() {
        let mut topology = crate::topology::build_topology();
        crate::layout::apply_visual_layout(&mut topology);
        inject_internal_control_lines(&mut topology);
        assert!(topology.node("returnDataMux").is_some());
        for (from, to, label, signal) in CALL_RETURN_PATH {
            assert!(topology.node(from).is_some(), "missing source {from}");
            assert!(topology.node(to).is_some(), "missing target {to}");
            assert!(topology.links.iter().any(|link| {
                link.from == from && link.to == to && link.label == label && link.signal == signal
            }), "missing CALL/RET wire {from} -> {to} ({label})");
        }
    }
}
