use crate::topology::{Link, Node, Rect, SignalKind, Topology};

pub const SHIFT_REGISTER_NODES: [&str; 5] = [
    "shiftHi",
    "shiftLo",
    "shiftOffset",
    "shiftMux",
    "shiftOut",
];

pub fn inject_shift_register(topology: &mut Topology) {
    let nodes = [
        ("shiftHi", "SHIFT HI", "8× DFF", 1000.0, 1940.0, 140.0, 76.0),
        ("shiftLo", "SHIFT LO", "8× DFF", 1170.0, 1940.0, 140.0, 76.0),
        ("shiftOffset", "SHIFT OFFSET", "3× DFF", 1340.0, 1940.0, 130.0, 76.0),
        ("shiftMux", "SHIFT WINDOW", "16→8 MUX", 1500.0, 1940.0, 160.0, 76.0),
        ("shiftOut", "SHIFT OUT", "8× LATCH", 1690.0, 1940.0, 150.0, 76.0),
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
        ("m3-shift-data-in", "dataBuf", "shiftHi", SignalKind::Data, "DATA WRITE"),
        ("m3-shift-cascade", "shiftHi", "shiftLo", SignalKind::Data, "OLD HI"),
        ("m3-shift-hi-window", "shiftHi", "shiftMux", SignalKind::Data, "BITS 15:8"),
        ("m3-shift-lo-window", "shiftLo", "shiftMux", SignalKind::Data, "BITS 7:0"),
        ("m3-shift-offset", "shiftOffset", "shiftMux", SignalKind::Control, "OFFSET[2:0]"),
        ("m3-shift-result", "shiftMux", "shiftOut", SignalKind::Data, "WINDOW[7:0]"),
        ("m3-shift-readback", "shiftOut", "dataBuf", SignalKind::Data, "DEVICE READ"),
        ("m3-shift-offset-write", "dataBuf", "shiftOffset", SignalKind::Data, "OFFSET WRITE"),
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
    fn shift_register_nodes_fit_inside_io_group() {
        let mut topology = topology::build_topology();
        layout::apply_visual_layout(&mut topology);
        inject_shift_register(&mut topology);
        let io = topology.group("io").expect("I/O group").bounds;
        for id in SHIFT_REGISTER_NODES {
            let node = topology.node(id).expect("shift node");
            assert!(node.bounds.x >= io.x);
            assert!(node.bounds.y >= io.y);
            assert!(node.bounds.x + node.bounds.w <= io.x + io.w);
            assert!(node.bounds.y + node.bounds.h <= io.y + io.h);
        }
    }
}
