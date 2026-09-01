use crate::topology::{Link, Node, Rect, SignalKind, Topology};

pub const VIDEO_TIMING_NODES: [&str; 2] = ["vblankLatch", "vblankWaitGate"];

pub fn inject_video_timing(topology: &mut Topology) {
    let gpu = topology.group("gpu").expect("GPU group").bounds;
    let latch = Rect::new(gpu.x + 1110.0, gpu.y + 640.0, 170.0, 76.0);
    let gate = Rect::new(gpu.x + 1110.0, gpu.y + 750.0, 170.0, 76.0);

    topology.nodes.push(Node {
        id: "vblankLatch".to_owned(),
        title: "VBLANK LATCH".to_owned(),
        kind: "SR LATCH".to_owned(),
        group: "gpu".to_owned(),
        bounds: latch,
    });
    topology.nodes.push(Node {
        id: "vblankWaitGate".to_owned(),
        title: "WAIT GATE".to_owned(),
        kind: "AND".to_owned(),
        group: "gpu".to_owned(),
        bounds: gate,
    });

    let links = [
        (
            "video-vsync-latch-set",
            "vsync",
            "vblankLatch",
            SignalKind::Control,
            "VBLANK SET",
        ),
        (
            "video-reset-latch-clear",
            "reset",
            "vblankLatch",
            SignalKind::Control,
            "VBLANK CLEAR",
        ),
        (
            "video-vblank-wait-enable",
            "vblankLatch",
            "vblankWaitGate",
            SignalKind::Control,
            "VBLANK PENDING",
        ),
        (
            "video-cpu-wait-enable",
            "ctrlWait",
            "vblankWaitGate",
            SignalKind::Control,
            "WAIT µCONTROL",
        ),
        (
            "video-vblank-irq-latch",
            "vblankWaitGate",
            "irqLatch",
            SignalKind::Control,
            "VBLANK ACK / RESUME",
        ),
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
    use crate::{control_layout, layout, topology};

    #[test]
    fn video_timing_nodes_fit_inside_gpu_group_and_close_wait_path() {
        let mut topology = topology::build_topology();
        layout::apply_visual_layout(&mut topology);
        control_layout::inject_internal_control_lines(&mut topology);
        inject_video_timing(&mut topology);
        let gpu = topology.group("gpu").expect("GPU group").bounds;
        for id in VIDEO_TIMING_NODES {
            let node = topology.node(id).expect("video timing node");
            assert!(node.bounds.x >= gpu.x);
            assert!(node.bounds.y >= gpu.y);
            assert!(node.bounds.x + node.bounds.w <= gpu.x + gpu.w);
            assert!(node.bounds.y + node.bounds.h <= gpu.y + gpu.h);
        }
        for (from, to) in [
            ("vsync", "vblankLatch"),
            ("vblankLatch", "vblankWaitGate"),
            ("ctrlWait", "vblankWaitGate"),
            ("vblankWaitGate", "irqLatch"),
        ] {
            assert!(topology
                .links
                .iter()
                .any(|link| link.from == from && link.to == to));
        }
    }
}
