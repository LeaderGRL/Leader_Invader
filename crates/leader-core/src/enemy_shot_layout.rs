use crate::topology::{Link, Node, Rect, SignalKind, Topology};

pub const ENEMY_SHOT_NODES: [&str; 11] = [
    "enemyShotAlloc",
    "enemyShotCooldown",
    "enemyShot0X",
    "enemyShot0Y",
    "enemyShot0Active",
    "enemyShot1X",
    "enemyShot1Y",
    "enemyShot1Active",
    "enemyShot2X",
    "enemyShot2Y",
    "enemyShot2Active",
];

pub fn inject_enemy_shot_bank(topology: &mut Topology) {
    if let Some(io) = topology.groups.iter_mut().find(|group| group.id == "io") {
        io.bounds.h = io.bounds.h.max(1000.0);
    }

    let nodes = [
        ("enemyShotAlloc", "SHOT ALLOC", "2-bit RR PTR", 1010.0, 2160.0, 170.0, 78.0),
        ("enemyShotCooldown", "SHOT COOLDOWN", "8-bit CNT", 1010.0, 2290.0, 170.0, 78.0),
        ("enemyShot0X", "SHOT 0 X", "8× DFF", 1240.0, 2160.0, 150.0, 66.0),
        ("enemyShot0Y", "SHOT 0 Y", "8× DFF", 1240.0, 2260.0, 150.0, 66.0),
        ("enemyShot0Active", "SHOT 0 ACTIVE", "DFF", 1240.0, 2360.0, 150.0, 66.0),
        ("enemyShot1X", "SHOT 1 X", "8× DFF", 1450.0, 2160.0, 150.0, 66.0),
        ("enemyShot1Y", "SHOT 1 Y", "8× DFF", 1450.0, 2260.0, 150.0, 66.0),
        ("enemyShot1Active", "SHOT 1 ACTIVE", "DFF", 1450.0, 2360.0, 150.0, 66.0),
        ("enemyShot2X", "SHOT 2 X", "8× DFF", 1660.0, 2160.0, 150.0, 66.0),
        ("enemyShot2Y", "SHOT 2 Y", "8× DFF", 1660.0, 2260.0, 150.0, 66.0),
        ("enemyShot2Active", "SHOT 2 ACTIVE", "DFF", 1660.0, 2360.0, 150.0, 66.0),
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

    topology.links.push(Link {
        id: "m3-shot-frame-clock".to_owned(),
        from: "timer".to_owned(),
        to: "enemyShotCooldown".to_owned(),
        signal: SignalKind::Clock,
        label: "FRAME CLOCK".to_owned(),
    });
    topology.links.push(Link {
        id: "m3-shot-allocator-clock".to_owned(),
        from: "enemyShotCooldown".to_owned(),
        to: "enemyShotAlloc".to_owned(),
        signal: SignalKind::Control,
        label: "SPAWN ENABLE".to_owned(),
    });

    for slot in 0..3 {
        let x = format!("enemyShot{slot}X");
        let y = format!("enemyShot{slot}Y");
        let active = format!("enemyShot{slot}Active");
        topology.links.push(Link {
            id: format!("m3-shot-{slot}-select"),
            from: "enemyShotAlloc".to_owned(),
            to: active.clone(),
            signal: SignalKind::Control,
            label: format!("SLOT {slot} SELECT"),
        });
        topology.links.push(Link {
            id: format!("m3-shot-{slot}-x-write"),
            from: "dataBuf".to_owned(),
            to: x.clone(),
            signal: SignalKind::Data,
            label: "X WRITE".to_owned(),
        });
        topology.links.push(Link {
            id: format!("m3-shot-{slot}-y-write"),
            from: "dataBuf".to_owned(),
            to: y.clone(),
            signal: SignalKind::Data,
            label: "Y WRITE".to_owned(),
        });
        topology.links.push(Link {
            id: format!("m3-shot-{slot}-active-write"),
            from: "ctrlBuf".to_owned(),
            to: active.clone(),
            signal: SignalKind::Control,
            label: "ARM / CLEAR".to_owned(),
        });
        topology.links.push(Link {
            id: format!("m3-shot-{slot}-x-video"),
            from: x,
            to: "spriteRom".to_owned(),
            signal: SignalKind::Video,
            label: "PROJECTILE X".to_owned(),
        });
        topology.links.push(Link {
            id: format!("m3-shot-{slot}-y-video"),
            from: y,
            to: "spriteRom".to_owned(),
            signal: SignalKind::Video,
            label: "PROJECTILE Y".to_owned(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{layout, topology};

    #[test]
    fn enemy_shot_bank_nodes_fit_inside_extended_io_group() {
        let mut topology = topology::build_topology();
        layout::apply_visual_layout(&mut topology);
        inject_enemy_shot_bank(&mut topology);
        let io = topology.group("io").expect("I/O group").bounds;
        assert!(io.y + io.h < 3000.0, "I/O must stay above system bus");
        for id in ENEMY_SHOT_NODES {
            let node = topology.node(id).expect("enemy-shot node");
            assert!(node.bounds.x >= io.x);
            assert!(node.bounds.y >= io.y);
            assert!(node.bounds.x + node.bounds.w <= io.x + io.w);
            assert!(node.bounds.y + node.bounds.h <= io.y + io.h);
        }
    }
}
