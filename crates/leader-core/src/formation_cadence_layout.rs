use crate::topology::{Link, Node, Rect, SignalKind, Topology};

pub const FORMATION_CADENCE_NODES: [&str; 4] = [
    "formationAlive",
    "formationDivider",
    "formationCounter",
    "formationTick",
];

pub fn inject_formation_cadence(topology: &mut Topology) {
    let nodes = [
        ("formationAlive", "ALIENS ALIVE", "6-bit POPCOUNT", 1000.0, 2030.0, 170.0, 60.0),
        ("formationDivider", "STEP DIVISOR", "3/2/1 SELECT", 1200.0, 2030.0, 170.0, 60.0),
        ("formationCounter", "STEP COUNTER", "2-bit CNT", 1400.0, 2030.0, 170.0, 60.0),
        ("formationTick", "FLEET TICK", "CMP + PULSE", 1600.0, 2030.0, 170.0, 60.0),
    ];
    for (id, title, kind, x, y, w, h) in nodes {
        topology.nodes.push(Node {
            id: id.to_owned(),
            title: title.to_owned(),
            kind: kind.to_owned(),
            group: "io".to_owned(),
            bounds: Rect::new(x, y, w, h),
        });
    }

    let links = [
        ("m3-cadence-alien-state", "dataBuf", "formationAlive", SignalKind::Data, "ALIEN ROW MASKS"),
        ("m3-cadence-speed-select", "formationAlive", "formationDivider", SignalKind::Control, "32→3 / 24→2 / 12→1"),
        ("m3-cadence-divisor", "formationDivider", "formationCounter", SignalKind::Control, "DIVISOR"),
        ("m3-cadence-clock", "timer", "formationCounter", SignalKind::Clock, "FRAME CLOCK"),
        ("m3-cadence-pulse", "formationCounter", "formationTick", SignalKind::Control, "COUNT == DIV"),
        ("m3-cadence-bus-gate", "formationTick", "ctrlBuf", SignalKind::Control, "FLEET MOVE ENABLE"),
    ];
    for (id, from, to, signal, label) in links {
        topology.links.push(Link {
            id: id.to_owned(),
            from: from.to_owned(),
            to: to.to_owned(),
            signal,
            label: label.to_owned(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{layout, topology};

    #[test]
    fn cadence_nodes_fit_inside_io_group() {
        let mut topology = topology::build_topology();
        layout::apply_visual_layout(&mut topology);
        inject_formation_cadence(&mut topology);
        let io = topology.group("io").expect("I/O group").bounds;
        for id in FORMATION_CADENCE_NODES {
            let node = topology.node(id).expect("cadence node");
            assert!(node.bounds.x >= io.x);
            assert!(node.bounds.y >= io.y);
            assert!(node.bounds.x + node.bounds.w <= io.x + io.w);
            assert!(node.bounds.y + node.bounds.h <= io.y + io.h);
        }
    }
}
