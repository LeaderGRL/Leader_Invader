use std::fmt::Write as _;

use leader_core::{
    resolve_physical_memory_address, AluEvent, BusTransactionEvent, BusTransactionKind, MatchTrace,
    MemoryOwner, Rect, Topology,
};
use leader_svg::RenderConfig;

const VIEWPORT: Rect = Rect {
    x: 24.0,
    y: 92.0,
    w: 900.0,
    h: 548.0,
};
const MAX_RENDERED_MEMORY_EVENTS: usize = 96;
const MAX_RENDERED_BUS_EVENTS: usize = 96;
const MAX_RENDERED_ALU_EVENTS: usize = 54;

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

#[derive(Debug, Clone, Copy)]
struct SceneFocus {
    pose: Pose,
    event_time: f32,
}

#[derive(Debug)]
struct CameraPlan {
    keys: Vec<CameraKey>,
    fetch_time: f32,
    micro_time: f32,
    alu_time: f32,
    rom_time: f32,
    ram_time: f32,
    alu_late_time: f32,
    vram_time: f32,
    gpu_time: f32,
    late_memory_time: f32,
}

/// Wrap the immutable physical topology in a deterministic technical camera.
/// Camera holds are centered on the exact native trace event they display.
#[must_use]
pub fn apply(mut svg: String, topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    if !svg.contains("data-frontpage-version=\"physical-die-v2\"") {
        return svg;
    }

    let plan = camera_plan(topology, trace, config);
    if plan.keys.len() < 2 {
        return svg;
    }

    let Some(machine_start) = svg.find("<g id=\"v2-machine\"") else {
        return svg;
    };
    let Some(relative_end) = svg[machine_start..].find('>') else {
        return svg;
    };
    let machine_open_end = machine_start + relative_end + 1;

    let initial = plan.keys[0].pose;
    let values_translate = plan
        .keys
        .iter()
        .map(|key| format!("{:.5} {:.5}", key.pose.tx, key.pose.ty))
        .collect::<Vec<_>>()
        .join(";");
    let values_scale = plan
        .keys
        .iter()
        .map(|key| format!("{:.7}", key.pose.scale))
        .collect::<Vec<_>>()
        .join(";");
    let key_times = plan
        .keys
        .iter()
        .map(|key| format!("{:.7}", (key.time / config.total()).clamp(0.0, 1.0)))
        .collect::<Vec<_>>()
        .join(";");
    let key_splines = std::iter::repeat_n("0.42 0 0.18 1", plan.keys.len().saturating_sub(1))
        .collect::<Vec<_>>()
        .join(";");

    let mut rig = String::with_capacity(12_000);
    let _ = write!(
        rig,
        r##"<g id="v2-machine-viewport" clip-path="url(#v2-machine-clip)" data-camera="trace-driven" data-camera-keys="{}"><g id="v2-camera-translate" transform="translate({:.5} {:.5})"><animateTransform attributeName="transform" attributeType="XML" type="translate" values="{}" keyTimes="{}" keySplines="{}" calcMode="spline" dur="{:.3}s" repeatCount="indefinite"/><g id="v2-camera-scale" transform="scale({:.7})"><animateTransform attributeName="transform" attributeType="XML" type="scale" values="{}" keyTimes="{}" keySplines="{}" calcMode="spline" dur="{:.3}s" repeatCount="indefinite"/><g id="v2-machine" data-camera-space="topology">"##,
        plan.keys.len(),
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
        let _ = write!(
            metadata,
            " data-camera-key-count=\"{}\" data-scene-fetch=\"{:.4}\" data-scene-micro=\"{:.4}\" data-scene-alu=\"{:.4}\" data-scene-rom=\"{:.4}\" data-scene-ram=\"{:.4}\" data-scene-alu-late=\"{:.4}\" data-scene-vram=\"{:.4}\" data-scene-gpu=\"{:.4}\" data-scene-late-memory=\"{:.4}\"",
            plan.keys.len(),
            plan.fetch_time,
            plan.micro_time,
            plan.alu_time,
            plan.rom_time,
            plan.ram_time,
            plan.alu_late_time,
            plan.vram_time,
            plan.gpu_time,
            plan.late_memory_time,
        );
        for (key_index, key) in plan.keys.iter().enumerate() {
            let _ = write!(
                metadata,
                " data-camera-t{key_index}=\"{:.3}\" data-camera-s{key_index}=\"{:.5}\"",
                key.time, key.pose.scale,
            );
        }
        metadata.push_str("/>\n");
        svg.insert_str(index, &metadata);
    }

    svg
}

fn camera_plan(topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> CameraPlan {
    let overview = fit_pose(Rect::new(0.0, 0.0, topology.width, topology.height), 8.0, 0.0);
    let cpu_pose = focus_groups(topology, &["pc", "decode"], 70.0, 0.64).unwrap_or(overview);
    let micro_pose = topology
        .node("microRom")
        .map(|node| fit_pose(node.bounds, 12.0, 3.8))
        .unwrap_or(cpu_pose);
    let alu_pose = topology
        .group("alu")
        .map(|group| fit_pose(group.bounds, 55.0, 0.52))
        .unwrap_or(overview);
    let gpu_pose = topology
        .group("gpu")
        .map(|group| fit_pose(group.bounds, 50.0, 0.50))
        .unwrap_or(overview);

    let fetch_time = select_fetch_time(trace, config, 3.0);
    let micro_time = select_micro_time(trace, config, 7.0);
    let alu_time = select_alu_time(trace, config, 11.5, false);
    let alu_late_time = select_alu_time(trace, config, 29.0, true);
    let rom = memory_focus(topology, trace, config, MemoryOwner::Rom, 16.0).unwrap_or(SceneFocus {
        pose: topology
            .group("romsys")
            .map_or(overview, |group| fit_pose(group.bounds, 45.0, 0.42)),
        event_time: 16.0,
    });
    let ram = memory_focus(topology, trace, config, MemoryOwner::Ram, 22.0).unwrap_or(SceneFocus {
        pose: topology
            .group("ramsys")
            .map_or(overview, |group| fit_pose(group.bounds, 45.0, 0.40)),
        event_time: 22.0,
    });
    let vram = memory_focus(topology, trace, config, MemoryOwner::Vram, 37.0).unwrap_or(SceneFocus {
        pose: topology
            .group("vramsys")
            .map_or(overview, |group| fit_pose(group.bounds, 45.0, 0.58)),
        event_time: 37.0,
    });
    let late_memory = memory_focus_any(topology, trace, config, 50.0).unwrap_or(SceneFocus {
        pose: ram.pose,
        event_time: 50.0,
    });
    let gpu_time = select_dma_time(trace, config, 44.0);

    let scenes = [
        (fetch_time, cpu_pose, 0.8_f32, 1.15_f32),
        (micro_time, micro_pose, 0.9, 1.25),
        (alu_time, alu_pose, 0.8, 1.35),
        (rom.event_time, rom.pose, 0.8, 1.35),
        (ram.event_time, ram.pose, 0.8, 1.55),
        (alu_late_time, alu_pose, 0.8, 1.35),
        (vram.event_time, vram.pose, 0.8, 1.55),
        (gpu_time, gpu_pose, 0.8, 1.45),
        (late_memory.event_time, late_memory.pose, 0.8, 1.55),
    ];

    let mut keys = vec![
        CameraKey {
            time: 0.0,
            pose: overview,
        },
        CameraKey {
            time: 0.9,
            pose: overview,
        },
    ];
    for (event_time, pose, lead, hold) in scenes {
        let center = event_time.clamp(2.0, config.total() - 2.0);
        keys.push(CameraKey {
            time: (center - lead).max(1.0),
            pose,
        });
        keys.push(CameraKey {
            time: (center + hold).min(config.total() - 1.0),
            pose,
        });
    }
    keys.push(CameraKey {
        time: (config.total() - 3.6).max(1.0),
        pose: overview,
    });
    keys.push(CameraKey {
        time: config.total(),
        pose: overview,
    });
    keys.sort_by(|left, right| left.time.total_cmp(&right.time));
    keys.dedup_by(|left, right| (left.time - right.time).abs() < 0.015);

    CameraPlan {
        keys,
        fetch_time,
        micro_time,
        alu_time,
        rom_time: rom.event_time,
        ram_time: ram.event_time,
        alu_late_time,
        vram_time: vram.event_time,
        gpu_time,
        late_memory_time: late_memory.event_time,
    }
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
) -> Option<SceneFocus> {
    let event = rendered_memory_events(trace)
        .into_iter()
        .filter_map(|event| {
            let address = event.address?;
            let physical = resolve_physical_memory_address(address)?;
            if physical.owner != owner {
                return None;
            }
            let time = trace_moment(event.frame, event.ordinal, trace, config) + 0.17;
            Some((physical, time, (time - desired_time).abs()))
        })
        .min_by(|left, right| left.2.total_cmp(&right.2))?;
    let prefix = owner_prefix(owner)?;
    let node = topology.node(&format!("{prefix}{}", event.0.page))?;
    Some(SceneFocus {
        pose: memory_bank_pose(topology, owner, event.0.page)
            .unwrap_or_else(|| fit_pose(node.bounds, 10.0, 3.0)),
        event_time: event.1,
    })
}

fn memory_focus_any(
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
    desired_time: f32,
) -> Option<SceneFocus> {
    let event = rendered_memory_events(trace)
        .into_iter()
        .filter_map(|event| {
            let address = event.address?;
            let physical = resolve_physical_memory_address(address)?;
            let time = trace_moment(event.frame, event.ordinal, trace, config) + 0.17;
            Some((physical, time, (time - desired_time).abs()))
        })
        .min_by(|left, right| left.2.total_cmp(&right.2))?;
    let prefix = owner_prefix(event.0.owner)?;
    let node = topology.node(&format!("{prefix}{}", event.0.page))?;
    Some(SceneFocus {
        pose: memory_bank_pose(topology, event.0.owner, event.0.page)
            .unwrap_or_else(|| fit_pose(node.bounds, 10.0, 3.0)),
        event_time: event.1,
    })
}

fn memory_bank_pose(topology: &Topology, owner: MemoryOwner, page: usize) -> Option<Pose> {
    let (prefix, columns, page_count) = match owner {
        MemoryOwner::Rom => ("romPage", 8_usize, 32_usize),
        MemoryOwner::Ram => ("ramPage", 12, 96),
        MemoryOwner::Vram => ("vramPage", 4, 8),
        MemoryOwner::Mmio | MemoryOwner::Unmapped => return None,
    };
    let row = page / columns;
    let col = page % columns;
    let span = if owner == MemoryOwner::Vram { 2 } else { 3 };
    let start_col = col
        .saturating_sub(span / 2)
        .min(columns.saturating_sub(span));
    let mut bounds = topology
        .node(&format!("{prefix}{}", row * columns + start_col))?
        .bounds;
    for offset in 1..span {
        let candidate = row * columns + start_col + offset;
        if candidate >= page_count {
            break;
        }
        bounds = union_rect(
            bounds,
            topology.node(&format!("{prefix}{candidate}"))?.bounds,
        );
    }
    Some(fit_pose(bounds, 24.0, 2.55))
}

fn owner_prefix(owner: MemoryOwner) -> Option<&'static str> {
    match owner {
        MemoryOwner::Rom => Some("romPage"),
        MemoryOwner::Ram => Some("ramPage"),
        MemoryOwner::Vram => Some("vramPage"),
        MemoryOwner::Mmio | MemoryOwner::Unmapped => None,
    }
}

fn select_fetch_time(trace: &MatchTrace, config: RenderConfig, desired: f32) -> f32 {
    rendered_bus_events(trace)
        .into_iter()
        .filter(|event| event.kind == BusTransactionKind::Fetch)
        .map(|event| trace_moment(event.frame, event.ordinal, trace, config) + 0.18)
        .min_by(|left, right| (left - desired).abs().total_cmp(&(right - desired).abs()))
        .unwrap_or(desired)
}

fn select_dma_time(trace: &MatchTrace, config: RenderConfig, desired: f32) -> f32 {
    rendered_bus_events(trace)
        .into_iter()
        .filter(|event| event.kind == BusTransactionKind::Dma)
        .map(|event| trace_moment(event.frame, event.ordinal, trace, config) + 0.22)
        .min_by(|left, right| (left - desired).abs().total_cmp(&(right - desired).abs()))
        .unwrap_or(desired)
}

fn select_micro_time(trace: &MatchTrace, config: RenderConfig, desired: f32) -> f32 {
    sample_slice(&trace.micro_addresses, 80)
        .into_iter()
        .filter(|event| event.control_bits != 0)
        .map(|event| trace_moment(event.frame, event.ordinal, trace, config) + 0.12)
        .min_by(|left, right| (left - desired).abs().total_cmp(&(right - desired).abs()))
        .unwrap_or(desired)
}

fn select_alu_time(trace: &MatchTrace, config: RenderConfig, desired: f32, late: bool) -> f32 {
    let events = rendered_alu_events(trace);
    let mut candidates = events
        .into_iter()
        .filter(|event| !late || trace_moment(event.frame, event.ordinal, trace, config) >= 20.0)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        alu_interest(**right)
            .cmp(&alu_interest(**left))
            .then_with(|| {
                let left_time = trace_moment(left.frame, left.ordinal, trace, config);
                let right_time = trace_moment(right.frame, right.ordinal, trace, config);
                (left_time - desired)
                    .abs()
                    .total_cmp(&(right_time - desired).abs())
            })
    });
    candidates.first().map_or(desired, |event| {
        trace_moment(event.frame, event.ordinal, trace, config) + 0.14
    })
}

fn alu_interest(event: AluEvent) -> u32 {
    event.trace.carry_chain.count_ones() * 4
        + event.trace.lhs.count_ones()
        + event.trace.rhs_effective.count_ones()
        + event.trace.result.count_ones()
}

fn rendered_memory_events(trace: &MatchTrace) -> Vec<&BusTransactionEvent> {
    let candidates = trace
        .bus_transactions
        .iter()
        .filter(|event| event.address.is_some() && event.data.is_some())
        .collect::<Vec<_>>();
    sample_refs(&candidates, MAX_RENDERED_MEMORY_EVENTS)
}

fn rendered_bus_events(trace: &MatchTrace) -> Vec<&BusTransactionEvent> {
    let candidates = trace
        .bus_transactions
        .iter()
        .filter(|event| event.address.is_some())
        .collect::<Vec<_>>();
    sample_refs(&candidates, MAX_RENDERED_BUS_EVENTS)
}

fn rendered_alu_events(trace: &MatchTrace) -> Vec<&AluEvent> {
    sample_slice(&trace.alu_events, MAX_RENDERED_ALU_EVENTS)
}

fn sample_refs<'a, T>(values: &[&'a T], limit: usize) -> Vec<&'a T> {
    if values.len() <= limit || limit == 0 {
        return values.to_vec();
    }
    let stride = values.len().div_ceil(limit);
    values.iter().step_by(stride).copied().collect()
}

fn sample_slice<T>(values: &[T], limit: usize) -> Vec<&T> {
    if values.len() <= limit || limit == 0 {
        return values.iter().collect();
    }
    let stride = values.len().div_ceil(limit);
    values.iter().step_by(stride).collect()
}

fn fit_pose(bounds: Rect, padding: f32, max_scale: f32) -> Pose {
    let focus = bounds.padded(padding);
    let scale = (VIEWPORT.w / focus.w.max(1.0)).min(VIEWPORT.h / focus.h.max(1.0));
    let scale = if max_scale > 0.0 {
        scale.min(max_scale)
    } else {
        scale
    };
    let rendered_w = focus.w * scale;
    let rendered_h = focus.h * scale;
    Pose {
        scale,
        tx: VIEWPORT.x + (VIEWPORT.w - rendered_w) * 0.5 - focus.x * scale,
        ty: VIEWPORT.y + (VIEWPORT.h - rendered_h) * 0.5 - focus.y * scale,
    }
}

fn union_rect(left: Rect, right: Rect) -> Rect {
    let x0 = left.x.min(right.x);
    let y0 = left.y.min(right.y);
    let x1 = (left.x + left.w).max(right.x + right.w);
    let y1 = (left.y + left.h).max(right.y + right.h);
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
    fn plan_exposes_native_scene_timestamps_and_deep_detail() {
        let topology = build_topology();
        let trace = Machine::run_match("trace-camera-overview", 5000);
        let config = crate::frontpage::render_config();
        let plan = camera_plan(&topology, &trace, config);
        assert!(plan.keys.len() >= 12);
        assert_eq!(plan.keys.first().unwrap().time, 0.0);
        assert!((plan.keys.last().unwrap().time - config.total()).abs() < 0.001);
        assert!(plan.keys.iter().any(|key| key.pose.scale >= 2.0));
        assert!(plan.ram_time > 0.0);
        assert!(plan.vram_time > 0.0);
        assert!(plan.gpu_time > 0.0);
    }

    #[test]
    fn memory_camera_uses_the_same_sampled_events_as_the_renderer() {
        let trace = Machine::run_match("trace-camera-memory-sampling", 5000);
        let sampled = rendered_memory_events(&trace);
        assert!(!sampled.is_empty());
        assert!(sampled.len() <= MAX_RENDERED_MEMORY_EVENTS + 1);
        assert!(sampled
            .iter()
            .all(|event| event.address.is_some() && event.data.is_some()));
    }
}
