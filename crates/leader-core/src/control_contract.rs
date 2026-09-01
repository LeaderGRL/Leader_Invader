use crate::{control_word_at, Topology, INTERNAL_CONTROL_NODES};

pub const EXTERNAL_CONTROL_NODES: [(&str, &str); 8] = [
    ("ctrlRegWrite", "REGW"),
    ("ctrlAlu", "ALU"),
    ("ctrlMemRead", "MEMR"),
    ("ctrlMemWrite", "MEMW"),
    ("ctrlPcLoad", "PCLD"),
    ("ctrlStack", "STACK"),
    ("ctrlWait", "WAIT"),
    ("ctrlHalt", "HALT"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalControlLine {
    pub bit: u8,
    pub node_id: &'static str,
    pub label: &'static str,
}

#[must_use]
pub fn physical_control_lines() -> [PhysicalControlLine; 24] {
    let mut lines = [PhysicalControlLine {
        bit: 0,
        node_id: "",
        label: "",
    }; 24];

    let mut index = 0;
    while index < EXTERNAL_CONTROL_NODES.len() {
        let (node_id, label) = EXTERNAL_CONTROL_NODES[index];
        lines[index] = PhysicalControlLine {
            bit: index as u8,
            node_id,
            label,
        };
        index += 1;
    }

    let mut internal_index = 0;
    while internal_index < INTERNAL_CONTROL_NODES.len() {
        let (node_id, _, label) = INTERNAL_CONTROL_NODES[internal_index];
        let bit = internal_index + 8;
        lines[bit] = PhysicalControlLine {
            bit: bit as u8,
            node_id,
            label,
        };
        internal_index += 1;
    }

    lines
}

#[must_use]
pub fn control_topology_violations(topology: &Topology) -> Vec<String> {
    let lines = physical_control_lines();
    let mut errors = Vec::new();

    for line in lines {
        if topology.node(line.node_id).is_none() {
            errors.push(format!("control bit {} missing node {}", line.bit, line.node_id));
            continue;
        }
        if !topology.links.iter().any(|link| {
            link.from == "microRom" && link.to == line.node_id && link.label == line.label
        }) {
            errors.push(format!(
                "control bit {} node {} is not wired from microRom as {}",
                line.bit, line.node_id, line.label
            ));
        }
    }

    for left in 0..lines.len() {
        for right in (left + 1)..lines.len() {
            if lines[left].node_id == lines[right].node_id {
                errors.push(format!(
                    "control bits {} and {} share node {}",
                    lines[left].bit, lines[right].bit, lines[left].node_id
                ));
            }
            if lines[left].label == lines[right].label {
                errors.push(format!(
                    "control bits {} and {} share label {}",
                    lines[left].bit, lines[right].bit, lines[left].label
                ));
            }
        }
    }

    errors
}

#[must_use]
pub fn physically_used_control_mask() -> u32 {
    let mut mask = 0_u32;
    for opcode in 0_u16..=255 {
        for address in 0_u16..=255 {
            mask |= control_word_at(address as u8, opcode as u8).bits24();
        }
    }
    mask & 0x00ff_ffff
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_topology;

    #[test]
    fn topology_exposes_exactly_one_node_for_every_physical_control_bit() {
        let topology = build_topology();
        let violations = control_topology_violations(&topology);
        assert!(violations.is_empty(), "{}", violations.join("\n"));

        let lines = physical_control_lines();
        assert_eq!(lines.len(), 24);
        for (expected_bit, line) in lines.iter().enumerate() {
            assert_eq!(usize::from(line.bit), expected_bit);
        }
    }

    #[test]
    fn every_physical_control_bit_is_exercised_by_the_control_rom() {
        assert_eq!(physically_used_control_mask(), 0x00ff_ffff);
    }
}
