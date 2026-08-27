use crate::topology::{Rect, Topology};

const INNER_PAD: f32 = 28.0;

/// Applies presentation-only geometry corrections while preserving stable node IDs,
/// links and subsystem ownership. The logical topology is intentionally independent
/// from the visual packing so F3 can evolve without turning coordinates into API.
pub fn apply_visual_layout(topology: &mut Topology) {
    pack_program_counter(topology);
    pack_decode(topology);
}

fn pack_program_counter(topology: &mut Topology) {
    // The original prototype placed MAR beside PC, which pushed MAR8..15 into the
    // decode block. Pack PC and MAR as two clean 8×2 banks inside the PC subsystem.
    for bit in 0..16 {
        let col = (bit % 8) as f32;
        let row = (bit / 8) as f32;
        if let Some(node) = topology.nodes.iter_mut().find(|node| node.id == format!("pcBit{bit}")) {
            node.bounds.x = 570.0 + col * 66.0;
            node.bounds.y = 150.0 + row * 78.0;
        }
        if let Some(node) = topology.nodes.iter_mut().find(|node| node.id == format!("marBit{bit}")) {
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
    // Keep the first MDR/IR cells clear of the left dashed border and pull the
    // microcode pair back from the right edge. The bank spacing itself stays intact.
    for bit in 0..8 {
        if let Some(node) = topology.nodes.iter_mut().find(|node| node.id == format!("mdrBit{bit}")) {
            node.bounds.x += 16.0;
        }
        if let Some(node) = topology.nodes.iter_mut().find(|node| node.id == format!("irBit{bit}")) {
            node.bounds.x += 16.0;
        }
    }
    set(topology, "microAddr", 1782.0, 220.0);
    set(topology, "microRom", 1782.0, 340.0);
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
}
