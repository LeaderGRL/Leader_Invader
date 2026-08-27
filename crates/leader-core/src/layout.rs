use crate::topology::{Link, Node, Rect, SignalKind, Topology};

const INNER_PAD: f32 = 28.0;

pub fn apply_visual_layout(topology: &mut Topology) {
    pack_program_counter(topology);
    pack_decode(topology);
    align_register_file(topology);
    inject_decoder_lines(topology);
    inject_control_lines(topology);
}

fn pack_program_counter(topology: &mut Topology) {
    for bit in 0..16 {
        let col = (bit % 8) as f32;
        let row = (bit / 8) as f32;
        if let Some(node) = topology
            .nodes
            .iter_mut()
            .find(|node| node.id == format!("pcBit{bit}"))
        {
            node.bounds.x = 570.0 + col * 66.0;
            node.bounds.y = 150.0 + row * 78.0;
        }
        if let Some(node) = topology
            .nodes
            .iter_mut()
            .find(|node| node.id == format!("marBit{bit}"))
        {
            node.bounds.x = 570.0 + col * 66.0;
            node.bounds.y = 350.0 + row * 78.0;
        }
    }

    set(topology, "pcMuxLo", 570.0, 535.0);
    set(topology, "pcMuxHi", 735.0, 535.0);
    set(topology, "pcIncLo", 900.0, 535.0);
    set(topology, "pcCarry", 930.0, 650.0);
    set(topology, "pcIncHi", 735.0, 650.0);

    if let Some(group) = topology.groups.iter_mut().find(|group| group.id == "pc") {
        group.bounds = Rect::new(520.0, 80.0, 650.0, 700.0);
    }
}

fn pack_decode(topology: &mut Topology) {
    for bit in 0..8 {
        if let Some(node) = topology
            .nodes
            .iter_mut()
            .find(|node| node.id == format!("mdrBit{bit}"))
        {
            node.bounds.x += 16.0;
        }
        if let Some(node) = topology
            .nodes
            .iter_mut()
            .find(|node| node.id == format!("irBit{bit}"))
        {
            node.bounds.x += 16.0;
        }
    }
    set(topology, "microAddr", 1782.0, 220.0);
    set(topology, "microRom", 1782.0, 340.0);
}

fn align_register_file(topology: &mut Topology) {
    const BANKS: [(&str, &str); 8] = [
        ("A", "A"),
        ("B", "B"),
        ("X", "C"),
        ("Y", "D"),
        ("TMP", "X"),
        ("FLAGS", "Y"),
        ("SPLO", "T"),
        ("SPHI", "U"),
    ];

    for (old, new) in BANKS {
        for bit in 0..8 {
            let old_id = format!("reg{old}{bit}");
            let new_id = format!("reg{new}{bit}");
            if old_id == new_id {
                if let Some(node) = topology.nodes.iter_mut().find(|node| node.id == old_id) {
                    node.title = format!("{new}{bit}");
                }
                continue;
            }

            if let Some(node) = topology.nodes.iter_mut().find(|node| node.id == old_id) {
                node.id = new_id.clone();
                node.title = format!("{new}{bit}");
            }
            for link in &mut topology.links {
                if link.from == old_id {
                    link.from = new_id.clone();
                }
                if link.to == old_id {
                    link.to = new_id.clone();
                }
            }
        }
    }
}

fn inject_decoder_lines(topology: &mut Topology) {
    if topology.node("decA0").is_some() {
        return;
    }

    for bank in ['A', 'B'] {
        let base_y = if bank == 'A' { 520.0 } else { 650.0 };
        for line in 0..16 {
            let col = (line % 8) as f32;
            let row = (line / 8) as f32;
            let id = format!("dec{bank}{line}");
            topology.nodes.push(Node {
                id: id.clone(),
                title: format!("{bank}:{line:X}"),
                kind: "1-HOT".to_owned(),
                group: "decode".to_owned(),
                bounds: Rect::new(1260.0 + col * 80.0, base_y + row * 48.0, 58.0, 34.0),
            });
            topology.links.push(Link {
                id: format!("decode-{bank}-{line}"),
                from: format!("dec{bank}"),
                to: id,
                signal: SignalKind::Control,
                label: format!("D{line:X}"),
            });
        }
    }
}

fn inject_control_lines(topology: &mut Topology) {
    if topology.node("ctrlRegWrite").is_some() {
        return;
    }

    const LINES: [(&str, &str); 8] = [
        ("ctrlRegWrite", "REGW"),
        ("ctrlAlu", "ALU"),
        ("ctrlMemRead", "MEMR"),
        ("ctrlMemWrite", "MEMW"),
        ("ctrlPcLoad", "PCLD"),
        ("ctrlStack", "STACK"),
        ("ctrlWait", "WAIT"),
        ("ctrlHalt", "HALT"),
    ];

    for (index, (id, title)) in LINES.iter().enumerate() {
        topology.nodes.push(Node {
            id: (*id).to_owned(),
            title: (*title).to_owned(),
            kind: "CTRL".to_owned(),
            group: "decode".to_owned(),
            bounds: Rect::new(1260.0 + index as f32 * 80.0, 775.0, 58.0, 34.0),
        });
        topology.links.push(Link {
            id: format!("micro-{id}"),
            from: "microRom".to_owned(),
            to: (*id).to_owned(),
            signal: SignalKind::Control,
            label: (*title).to_owned(),
        });
    }
}

fn set(topology: &mut Topology, id: &str, x: f32, y: f32) {
    if let Some(node) = topology.nodes.iter_mut().find(|node| node.id == id) {
        node.bounds.x = x;
        node.bounds.y = y;
    }
}

#[must_use]
pub fn layout_violations(topology: &Topology) -> Vec<String> {
    let mut errors = Vec::new();
    for node in &topology.nodes {
        let Some(group) = topology.group(&node.group) else {
            errors.push(format!("{} references missing group {}", node.id, node.group));
            continue;
        };
        let inner = Rect::new(
            group.bounds.x + INNER_PAD,
            group.bounds.y + INNER_PAD,
            group.bounds.w - INNER_PAD * 2.0,
            group.bounds.h - INNER_PAD * 2.0,
        );
        if !contains(inner, node.bounds) {
            errors.push(format!(
                "{} escapes {}: node=({:.0},{:.0},{:.0},{:.0}) group=({:.0},{:.0},{:.0},{:.0})",
                node.id,
                node.group,
                node.bounds.x,
                node.bounds.y,
                node.bounds.w,
                node.bounds.h,
                group.bounds.x,
                group.bounds.y,
                group.bounds.w,
                group.bounds.h
            ));
        }
    }
    errors
}

fn contains(outer: Rect, inner: Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.w <= outer.x + outer.w
        && inner.y + inner.h <= outer.y + outer.h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_node_stays_inside_its_visual_subsystem() {
        let mut topology = crate::topology::build_topology();
        apply_visual_layout(&mut topology);
        let violations = layout_violations(&topology);
        assert!(violations.is_empty(), "{}", violations.join("\n"));
    }

    #[test]
    fn register_banks_match_real_isa() {
        let mut topology = crate::topology::build_topology();
        apply_visual_layout(&mut topology);
        for name in ["A", "B", "C", "D", "X", "Y", "T", "U"] {
            for bit in 0..8 {
                assert!(topology.node(&format!("reg{name}{bit}")).is_some());
            }
        }
    }

    #[test]
    fn decoder_exposes_all_one_hot_lines() {
        let mut topology = crate::topology::build_topology();
        apply_visual_layout(&mut topology);
        for bank in ['A', 'B'] {
            for line in 0..16 {
                assert!(topology.node(&format!("dec{bank}{line}")).is_some());
            }
        }
    }

    #[test]
    fn control_rom_exposes_all_control_lines() {
        let mut topology = crate::topology::build_topology();
        apply_visual_layout(&mut topology);
        for id in [
            "ctrlRegWrite",
            "ctrlAlu",
            "ctrlMemRead",
            "ctrlMemWrite",
            "ctrlPcLoad",
            "ctrlStack",
            "ctrlWait",
            "ctrlHalt",
        ] {
            assert!(topology.node(id).is_some(), "missing {id}");
        }
    }
}
