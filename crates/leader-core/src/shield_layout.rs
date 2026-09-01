use crate::topology::{Link, Node, Rect, SignalKind, Topology};

pub const SHIELD_NODES: [&str; 8] = [
    "shieldAddr",
    "shieldMask",
    "shieldWriteEnable",
    "shieldRam0",
    "shieldRam1",
    "shieldRam2",
    "shieldRam3",
    "shieldVideoMux",
];

pub fn inject_shield_bank(topology: &mut Topology) {
    if let Some(io) = topology.groups.iter_mut().find(|group| group.id == "io") {
        io.bounds.h = io.bounds.h.max(1370.0);
    }

    let nodes = [
        ("shieldAddr", "SHIELD ADDR", "6-bit ADDR", 1010.0, 2520.0, 170.0, 72.0),
        ("shieldMask", "DAMAGE MASK", "8-bit 1-HOT", 1010.0, 2630.0, 170.0, 72.0),
        ("shieldWriteEnable", "SHIELD WRITE", "AND / WE", 1010.0, 2740.0, 170.0, 72.0),
        ("shieldRam0", "SHIELD 0", "128-bit RAM", 1240.0, 2520.0, 150.0, 92.0),
        ("shieldRam1", "SHIELD 1", "128-bit RAM", 1420.0, 2520.0, 150.0, 92.0),
        ("shieldRam2", "SHIELD 2", "128-bit RAM", 1600.0, 2520.0, 150.0, 92.0),
        ("shieldRam3", "SHIELD 3", "128-bit RAM", 1780.0, 2520.0, 130.0, 92.0),
        ("shieldVideoMux", "SHIELD VIDEO", "4→1 MUX", 1450.0, 2730.0, 220.0, 82.0),
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

    for (id, from, to, signal, label) in [
        ("m3-shield-addr-data", "dataBuf", "shieldAddr", SignalKind::Data, "X/Y ADDRESS"),
        ("m3-shield-mask-data", "dataBuf", "shieldMask", SignalKind::Data, "1-HOT MASK"),
        ("m3-shield-write-gate", "ctrlBuf", "shieldWriteEnable", SignalKind::Control, "DAMAGE WRITE"),
        ("m3-shield-mask-write", "shieldMask", "shieldWriteEnable", SignalKind::Control, "BIT CLEAR"),
    ] {
        topology.links.push(Link {
            id: id.to_owned(),
            from: from.to_owned(),
            to: to.to_owned(),
            signal,
            label: label.to_owned(),
        });
    }

    for shield in 0..4 {
        let ram = format!("shieldRam{shield}");
        topology.links.push(Link {
            id: format!("m3-shield-{shield}-select"),
            from: "shieldAddr".to_owned(),
            to: ram.clone(),
            signal: SignalKind::Control,
            label: format!("SHIELD {shield} CS"),
        });
        topology.links.push(Link {
            id: format!("m3-shield-{shield}-write"),
            from: "shieldWriteEnable".to_owned(),
            to: ram.clone(),
            signal: SignalKind::Control,
            label: "WE".to_owned(),
        });
        topology.links.push(Link {
            id: format!("m3-shield-{shield}-video"),
            from: ram,
            to: "shieldVideoMux".to_owned(),
            signal: SignalKind::Video,
            label: "BITMAP".to_owned(),
        });
    }

    topology.links.push(Link {
        id: "m3-shield-video-out".to_owned(),
        from: "shieldVideoMux".to_owned(),
        to: "spriteRom".to_owned(),
        signal: SignalKind::Video,
        label: "SHIELD PIXEL".to_owned(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{layout, topology};

    #[test]
    fn shield_datapath_fits_between_projectile_bank_and_system_bus() {
        let mut topology = topology::build_topology();
        layout::apply_visual_layout(&mut topology);
        inject_shield_bank(&mut topology);
        let io = topology.group("io").expect("I/O group").bounds;
        assert!(io.y + io.h <= 2940.0);
        assert!(io.y + io.h < 3000.0, "shield hardware must remain above bus");
        for id in SHIELD_NODES {
            let node = topology.node(id).expect("shield node");
            assert!(node.bounds.x >= io.x);
            assert!(node.bounds.y >= io.y);
            assert!(node.bounds.x + node.bounds.w <= io.x + io.w);
            assert!(node.bounds.y + node.bounds.h <= io.y + io.h);
        }
    }
}
