use std::fmt::Write as _;

use leader_core::{
    build_navigation, resolve_physical_memory_address, AluEvent, BusTransactionEvent,
    BusTransactionKind, MatchTrace, MemoryOwner, NavigationModel, Rect, Topology,
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
const SCENE_HOLD_HALF: f32 = 0.42;
const MIN_SCENE_HOLD_HALF: f32 = 0.12;

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

#[derive(Debug, Clone, Copy)]
struct CameraScene {
    name: &'static str,
    time: f32,
    pose: Pose,
}

#[derive(Debug)]
struct CameraPlan {
    keys: Vec<CameraKey>,
    scenes: Vec<CameraScene>,
}

/// Wraps the immutable physical topology in a deterministic technical camera.
/// Every named scene has a serialized expected pose. Browser validation checks
/// that the animated camera actually reaches that pose while the corresponding
/// native trace event is visible; activity elsewhere in the SVG is not enough.
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

    let mut rig = String::with_capacity(14_000);
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
            " data-camera-key-count=\"{}\" data-scene-count=\"{}\"",
            plan.keys.len(),
            plan.scenes.len(),
        );
        for scene in &plan.scenes {
            let _ = write!(
                metadata,
                " data-scene-{}=\"{:.4}\" data-scene-{}-tx=\"{:.5}\" data-scene-{}-ty=\"{:.5}\" data-scene-{}-scale=\"{:.7}\"",
                scene.name,
                scene.time,
                scene.name,
                scene.pose.tx,
                scene.name,
                scene.pose.ty,
                scene.name,
                scene.pose.scale,
            );
        }
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
    let navigation = build_navigation(topology);
    let fetch_pose = focus_navigation_modules(
        &navigation,
        &["pc.fetch", "decode.instruction"],
        56.0,
        0.82,
    )
    .unwrap_or(overview);
    let micro_pose = topology
        .node("microRom")
        .map(|node| fit_pose(node.bounds, 12.0, 3.8))
        .unwrap_or(fetch_pose);
    let alu_pose = topology
        .group("alu")
        .map(|group| fit_pose(group.bounds, 55.0, 0.52))
        .unwrap_or(overview);
    let gpu_pose = topology
        .group("gpu")
        .map(|group| fit_pose(group.bounds, 50.0, 0.50))
        .unwrap_or(overview);

    // Desired times are deliberately separated. The selected timestamps still
    // come from renderer-visible native events, but the presentation no longer
    // creates overlapping holds that bounce between two subsystems.
    let fetch_time = select_fetch_time(trace, config, 3.2);
    let vram = memory_focus(topology, trace, config, MemoryOwner::Vram, 5.3).unwrap_or(SceneFocus {
        pose: topology
            .group("vramsys")
            .map_or(overview, |group| fit_pose(group.bounds, 45.0, 0.58)),
        event_time: 5.3,
    });
    let micro_time = select_micro_time(trace, config, 7.2);
    let rom = memory_focus(topology, trace, config, MemoryOwner::Rom, 17.4).unwrap_or(SceneFocus {
        pose: topology
            .group("romsys")
            .map_or(overview, |group| fit_pose(group.bounds, 45.0, 0.42)),
        event_time: 17.4,
    });
    let alu_time = select_alu_time(trace, config, 19.8);
    let gpu_time = select_dma_time(trace, config, 21.8);
    let ram = memory_focus(topology, trace, config, MemoryOwner::Ram, 25.5).unwrap_or(SceneFocus {
        pose: topology
            .group("ramsys")
            .map_or(overview, |group| fit_pose(group.bounds, 45.0, 0.40)),
        event_time: 25.5,
    });
    let late_memory = memory_focus_any(topology, trace, config, 43.0).unwrap_or(SceneFocus {
        pose: ram.pose,
        event_time: 43.0,
    });

    let mut scenes = vec![
        CameraScene { name: "fetch", time: fetch_time, pose: fetch_pose },
        CameraScene { name: "vram", time: vram.event_time, pose: vram.pose },
        CameraScene { name: "micro", time: micro_time, pose: micro_pose },
        CameraScene { name: "rom", time: rom.event_time, pose: rom.pose },
        CameraScene { name: "alu", time: alu_time, pose: alu_pose },
        CameraScene { name: "gpu", time: gpu_time, pose: gpu_pose },
        CameraScene { name: "ram", time: ram.event_time, pose: ram.pose },
        CameraScene { name: "late-memory", time: late_memory.event_time, pose: late_memory.pose },
    ];
    scenes.sort_by(|left, right| left.time.total_cmp(&right.time));

    let mut keys = vec![
        CameraKey { time: 0.0, pose: overview },
        CameraKey { time: 0.9, pose: overview },
    ];
    for (index, scene) in scenes.iter().enumerate() {
        let previous_gap = index
            .checked_sub(1)
            .map_or(f32::INFINITY, |previous| scene.time - scenes[previous].time);
        let next_gap = scenes
            .get(index + 1)
            .map_or(f32::INFINITY, |next| next.time - scene.time);
        let half_hold = SCENE_HOLD_HALF
            .min(previous_gap * 0.22)
            .min(next_gap * 0.22)
            .max(MIN_SCENE_HOLD_HALF);
        keys.push(CameraKey {
            time: (scene.time - half_hold).max(1.0),
            pose: scene.pose,
        });
        keys.push(CameraKey {
            time: (scene.time + half_hold).min(config.total() - 1.0),
            pose: scene.pose,
        });
    }
    keys.push(CameraKey {
        time: (config.total() - 3.0).max(1.0),
        pose: overview,
    });
    keys.push(CameraKey { time: config.total(), pose: overview });
    keys.sort_by(|left, right| left.time.total_cmp(&right.time));
    dedupe_key_times(&mut keys);

    CameraPlan { keys, scenes }
}

fn focus_navigation_modules(
    navigation: &NavigationModel,
    module_ids: &[&str],
    padding: f32,
    max_scale: f32,
) -> Option<Pose> {
    let mut bounds = None;
    for module_id in module_ids {
        let view = navigation.view_for_module(module_id)?;
        bounds = Some(match bounds {
            None => view.bounds,
            Some(current) => union_rect(current, view.bounds),
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

fn select_alu_time(trace: &MatchTrace, config: RenderConfig, desired: f32) -> f32 {
    rendered_alu_events(trace)
        .into_iter()
        .min_by(|left, right| {
            let left_time = trace_moment(left.frame, left.ordinal, trace, config);
            let right_time = trace_moment(right.frame, right.ordinal, trace, config);
            (left_time - desired)
                .abs()
                .total_cmp(&(right_time - desired).abs())
                .then_with(|| alu_interest(**right).cmp(&alu_interest(**left)))
        })
        .map_or(desired, |event| trace_moment(event.frame, event.ordinal, trace, config) + 0.14)
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

fn dedupe_key_times(keys: &mut [CameraKey]) {
    let mut previous = -1.0_f32;
    for key in keys {
        if key.time <= previous {
            key.time = previous + 0.001;
        }
        previous = key.time;
    }
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
    fn scene_contract_serializes_time_and_exact_expected_pose() {
        let topology = build_topology();
        let trace = Machine::run_match("trace-camera-contract", 5000);
        let source = "<svg data-frontpage-version=\"physical-die-v2\"><g id=\"v2-machine\"></g>\n<g id=\"v2-crt\"></g></svg>".to_string();
        let output = apply(source, &topology, &trace, crate::frontpage::render_config());
        assert!(output.contains("data-scene-micro=\""));
        assert!(output.contains("data-scene-micro-tx=\""));
        assert!(output.contains("data-scene-micro-ty=\""));
        assert!(output.contains("data-scene-micro-scale=\""));
        assert!(output.contains("data-scene-late-memory=\""));
    }

    #[test]
    fn camera_visits_microcode_once_and_scene_centers_do_not_overlap() {
        let topology = build_topology();
        let trace = Machine::run_match("trace-camera-sequence", 5000);
        let plan = camera_plan(&topology, &trace, crate::frontpage::render_config());
        assert_eq!(plan.scenes.iter().filter(|scene| scene.name == "micro").count(), 1);
        assert_eq!(plan.scenes.iter().filter(|scene| scene.name == "alu").count(), 1);
        for pair in plan.scenes.windows(2) {
            assert!(pair[1].time - pair[0].time > 1.0, "{} and {} overlap", pair[0].name, pair[1].name);
        }
    }

    #[test]
    fn fetch_pose_excludes_the_dedicated_microcode_closeup() {
        let topology = build_topology();
        let trace = Machine::run_match("trace-camera-fetch", 5000);
        let plan = camera_plan(&topology, &trace, crate::frontpage::render_config());
        let fetch = plan.scenes.iter().find(|scene| scene.name == "fetch").unwrap();
        let micro = plan.scenes.iter().find(|scene| scene.name == "micro").unwrap();
        assert!(micro.pose.scale > fetch.pose.scale * 2.0);
        assert!((micro.pose.tx - fetch.pose.tx).abs() > 20.0 || (micro.pose.ty - fetch.pose.ty).abs() > 20.0);
    }

    #[test]
    fn memory_camera_uses_the_same_sampled_events_as_the_renderer() {
        let trace = Machine::run_match("trace-camera-memory-sampling", 5000);
        let sampled = rendered_memory_events(&trace);
        assert!(!sampled.is_empty());
        assert!(sampled.len() <= MAX_RENDERED_MEMORY_EVENTS + 1);
        assert!(sampled.iter().all(|event| event.address.is_some() && event.data.is_some()));
    }
}
