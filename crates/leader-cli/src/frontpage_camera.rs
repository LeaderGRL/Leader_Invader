use std::fmt::Write as _;

use leader_core::{
    resolve_physical_memory_address, MatchTrace, MemoryOwner, Rect, Topology,
};
use leader_svg::RenderConfig;

// Keep a dedicated right-hand sidebar for the CRT. The hardware camera never
// renders underneath the display, eliminating panel/topology overlap while
// preserving the full 1200x675 GitHub canvas.
const VIEWPORT: Rect = Rect {
    x: 24.0,
    y: 92.0,
    w: 900.0,
    h: 548.0,
};

#[derive(Debug, Clone, Copy)]
struct Pose {
    tx: f32,
    ty: f32,
    scale: f32,
}

#[derive(Debug, Clone, Copy)]
struct CameraKey {
    time: f32,
    pose: Pose,
}

/// Wraps the raw topology group in a clipped, trace-driven camera rig.
///
/// The camera never changes topology or signal timing. It only changes how much
/// of the already-present physical machine is visible, so low-level detail can
/// be inspected without reintroducing particles or decorative motion.
#[must_use]
pub fn apply(mut svg: String, topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    if !svg.contains("data-frontpage-version=\"physical-die-v2\"") {
        return svg;
    }

    let keys = camera_keys(topology, trace, config);
    if keys.len() < 2 {
        return svg;
    }

    let Some(machine_start) = svg.find("<g id=\"v2-machine\"") else {
        return svg;
    };
    let Some(relative_end) = svg[machine_start..].find('>') else {
        return svg;
    };
    let machine_open_end = machine_start + relative_end + 1;

    let mut rig = String::with_capacity(12_000);
    let initial = keys[0].pose;
    let values_translate = keys
        .iter()
        .map(|key| format!("{:.5} {:.5}", key.pose.tx, key.pose.ty))
        .collect::<Vec<_>>()
        .join(";");
    let values_scale = keys
        .iter()
        .map(|key| format!("{:.7}", key.pose.scale))
        .collect::<Vec<_>>()
        .join(";");
    let key_times = keys
        .iter()
        .map(|key| format!("{:.7}", (key.time / config.total()).clamp(0.0, 1.0)))
        .collect::<Vec<_>>()
        .join(";");
    let key_splines = std::iter::repeat_n("0.42 0 0.18 1", keys.len().saturating_sub(1))
        .collect::<Vec<_>>()
        .join(";");

    let _ = write!(
        rig,
        r##"<g id="v2-machine-viewport" clip-path="url(#v2-machine-clip)" data-camera="trace-driven" data-camera-keys="{}"><g id="v2-camera-translate" transform="translate({:.5} {:.5})"><animateTransform attributeName="transform" attributeType="XML" type="translate" values="{}" keyTimes="{}" keySplines="{}" calcMode="spline" dur="{:.3}s" repeatCount="indefinite"/><g id="v2-camera-scale" transform="scale({:.7})"><animateTransform attributeName="transform" attributeType="XML" type="scale" values="{}" keyTimes="{}" keySplines="{}" calcMode="spline" dur="{:.3}s" repeatCount="indefinite"/><g id="v2-machine" data-camera-space="topology">"##,
        keys.len(),
        initial.tx,
        initial.ty,
        values_translate,
        key_times,
        key_splines,
        config.total(),
        initial.scale,
        values_scale,
        key_times,
        key_splines,
        config.total(),
    );

    svg.replace_range(machine_start..machine_open_end, &rig);

    const CRT_MARKER: &str = "</g>\n<g id=\"v2-crt\">";
    if let Some(machine_end) = svg.find(CRT_MARKER) {
        svg.replace_range(
            machine_end..machine_end + CRT_MARKER.len(),
            "</g></g></g></g>\n<g id=\"v2-crt\">",
        );
    } else {
        return svg;
    }

    svg = svg.replace(
        "PHYSICAL TRACE COMPUTER · RUST · NO CAMERA · NO PARTICLES",
        "PHYSICAL TRACE COMPUTER · RUST · TRACE-DRIVEN CAMERA · NO PARTICLES",
    );
    svg = svg.replace(
        "A dense fixed hardware die.",
        "A dense hardware die explored by a deterministic trace-driven camera.",
    );

    if let Some(index) = svg.rfind("</svg>") {
        let mut metadata = String::from("<g id=\"v2-camera-contract\" display=\"none\"");
        let _ = write!(metadata, " data-camera-key-count=\"{}\"", keys.len());
        for (index, key) in keys.iter().enumerate() {
            let _ = write!(
                metadata,
                " data-camera-t{index}=\"{:.3}\" data-camera-s{index}=\"{:.5}\"",
                key.time, key.pose.scale,
            );
        }
        metadata.push_str("/>\n");
        svg.insert_str(index, &metadata);
    }

    svg
}

fn camera_keys(topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> Vec<CameraKey> {
    let overview = fit_pose(
        Rect::new(0.0, 0.0, topology.width, topology.height),
        8.0,
        0.0,
    );
    let cpu = focus_groups(topology, &["pc", "decode"], 70.0, 0.64).unwrap_or(overview);
    let micro = topology
        .node("microRom")
        .map(|node| fit_pose(node.bounds, 12.0, 3.8))
        .unwrap_or(cpu);
    let alu = topology
        .group("alu")
        .map(|group| fit_pose(group.bounds, 55.0, 0.52))
        .unwrap_or(overview);
    let rom = memory_focus(topology, trace, config, MemoryOwner::Rom, 15.0)
        .or_else(|| topology.group("romsys").map(|group| fit_pose(group.bounds, 45.0, 0.42)))
        .unwrap_or(overview);
    let ram = memory_focus(topology, trace, config, MemoryOwner::Ram, 22.0)
        .or_else(|| topology.group("ramsys").map(|group| fit_pose(group.bounds, 45.0, 0.40)))
        .unwrap_or(overview);
    let vram = memory_focus(topology, trace, config, MemoryOwner::Vram, 37.0)
        .or_else(|| topology.group("vramsys").map(|group| fit_pose(group.bounds, 45.0, 0.58)))
        .unwrap_or(overview);
    let late_memory = memory_focus_any(topology, trace, config, 50.0).unwrap_or(ram);
    let gpu = topology
        .group("gpu")
        .map(|group| fit_pose(group.bounds, 50.0, 0.50))
        .unwrap_or(overview);

    let total = config.total();
    let mut keys = vec![
        CameraKey { time: 0.0, pose: overview },
        CameraKey { time: 0.9, pose: overview },
        CameraKey { time: 1.8, pose: cpu },
        CameraKey { time: 4.0, pose: cpu },
        CameraKey { time: 5.0, pose: micro },
        CameraKey { time: 7.2, pose: micro },
        CameraKey { time: 8.3, pose: alu },
        CameraKey { time: 12.0, pose: alu },
        CameraKey { time: 13.1, pose: rom },
        CameraKey { time: 17.0, pose: rom },
        CameraKey { time: 18.1, pose: ram },
        CameraKey { time: 24.0, pose: ram },
        CameraKey { time: 25.1, pose: alu },
        CameraKey { time: 31.0, pose: alu },
        CameraKey { time: 32.1, pose: vram },
        CameraKey { time: 39.0, pose: vram },
        CameraKey { time: 40.1, pose: gpu },
        CameraKey { time: 47.0, pose: gpu },
        CameraKey { time: 48.1, pose: late_memory },
        CameraKey { time: 54.0, pose: late_memory },
        CameraKey { time: 55.2, pose: overview },
        CameraKey { time: total, pose: overview },
    ];

    for key in &mut keys {
        key.time = key.time.min(total);
    }
    keys.sort_by(|a, b| a.time.total_cmp(&b.time));
    keys.dedup_by(|a, b| (a.time - b.time).abs() < 0.000_1);
    keys
}

fn focus_groups(topology: &Topology, ids: &[&str], padding: f32, max_scale: f32) -> Option<Pose> {
    let mut bounds = None;
    for id in ids {
        let group = topology.group(id)?;
        bounds = Some(match bounds {
            None => group.bounds,
            Some(current) => union_rect(current, group.bounds),
        });
    }
    bounds.map(|bounds| fit_pose(bounds, padding, max_scale))
}

fn memory_focus(
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
    owner: MemoryOwner,
    desired_time: f32,
) -> Option<Pose> {
    let event = trace
        .bus_transactions
        .iter()
        .filter_map(|event| {
            let address = event.address?;
            let physical = resolve_physical_memory_address(address)?;
            if physical.owner != owner {
                return None;
            }
            let time = trace_moment(event.frame, event.ordinal, trace, config);
            Some((event, physical, (time - desired_time).abs()))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))?;

    let prefix = match owner {
        MemoryOwner::Rom => "romPage",
        MemoryOwner::Ram => "ramPage",
        MemoryOwner::Vram => "vramPage",
        MemoryOwner::Mmio | MemoryOwner::Unmapped => return None,
    };
    let node = topology.node(&format!("{prefix}{}", event.1.page))?;
    Some(fit_pose(node.bounds, 10.0, 3.0))
}

fn memory_focus_any(
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
    desired_time: f32,
) -> Option<Pose> {
    let event = trace
        .bus_transactions
        .iter()
        .filter_map(|event| {
            let address = event.address?;
            let physical = resolve_physical_memory_address(address)?;
            let time = trace_moment(event.frame, event.ordinal, trace, config);
            Some((physical, (time - desired_time).abs()))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))?;

    let prefix = match event.0.owner {
        MemoryOwner::Rom => "romPage",
        MemoryOwner::Ram => "ramPage",
        MemoryOwner::Vram => "vramPage",
        MemoryOwner::Mmio | MemoryOwner::Unmapped => return None,
    };
    let node = topology.node(&format!("{prefix}{}", event.0.page))?;
    Some(fit_pose(node.bounds, 10.0, 3.0))
}

fn fit_pose(bounds: Rect, padding: f32, max_scale: f32) -> Pose {
    let focus = bounds.padded(padding);
    let scale = (VIEWPORT.w / focus.w.max(1.0))
        .min(VIEWPORT.h / focus.h.max(1.0));
    let scale = if max_scale > 0.0 { scale.min(max_scale) } else { scale };
    let rendered_w = focus.w * scale;
    let rendered_h = focus.h * scale;
    Pose {
        scale,
        tx: VIEWPORT.x + (VIEWPORT.w - rendered_w) * 0.5 - focus.x * scale,
        ty: VIEWPORT.y + (VIEWPORT.h - rendered_h) * 0.5 - focus.y * scale,
    }
}

fn union_rect(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.min(b.x);
    let y0 = a.y.min(b.y);
    let x1 = (a.x + a.w).max(b.x + b.w);
    let y1 = (a.y + a.h).max(b.y + b.h);
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

fn trace_moment(frame: u32, ordinal: u16, trace: &MatchTrace, config: RenderConfig) -> f32 {
    if trace.total_frames == 0 {
        return config.game_start();
    }
    config.game_start()
        + frame as f32 / trace.total_frames as f32 * config.game_seconds
        + f32::from(ordinal.min(63)) * 0.0015
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::{build_topology, Machine};

    #[test]
    fn camera_wraps_machine_without_moving_the_physical_topology() {
        let topology = build_topology();
        let trace = Machine::run_match("trace-camera", 5000);
        let source = "<svg data-frontpage-version=\"physical-die-v2\"><g id=\"v2-machine\" clip-path=\"url(#v2-machine-clip)\" transform=\"translate(1 2) scale(.1)\"><g id=\"v2-logic-nodes\"></g></g>\n<g id=\"v2-crt\"></g><text>PHYSICAL TRACE COMPUTER · RUST · NO CAMERA · NO PARTICLES</text></svg>".to_string();
        let output = apply(source, &topology, &trace, crate::frontpage::render_config());
        assert!(output.contains("id=\"v2-machine-viewport\""));
        assert!(output.contains("id=\"v2-camera-translate\""));
        assert!(output.contains("id=\"v2-camera-scale\""));
        assert!(output.contains("TRACE-DRIVEN CAMERA · NO PARTICLES"));
        assert!(!output.contains("id=\"v2-machine\" clip-path="));
    }

    #[test]
    fn camera_starts_and_ends_on_the_full_die() {
        let topology = build_topology();
        let trace = Machine::run_match("trace-camera-overview", 5000);
        let config = crate::frontpage::render_config();
        let keys = camera_keys(&topology, &trace, config);
        assert!(keys.len() >= 20);
        assert!((keys.first().unwrap().pose.scale - keys.last().unwrap().pose.scale).abs() < 0.000_1);
        assert_eq!(keys.first().unwrap().time, 0.0);
        assert!((keys.last().unwrap().time - config.total()).abs() < 0.001);
        assert!(keys.iter().any(|key| key.pose.scale >= 2.8));
    }
}
