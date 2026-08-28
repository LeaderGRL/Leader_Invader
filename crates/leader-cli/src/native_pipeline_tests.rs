use leader_core::{build_topology, Machine};
use leader_svg::{render, RenderConfig};

use crate::{
    alu_overlay, bus_overlay, control_state_overlay, control_word_overlay, decoder_overlay,
    director, enemy_shot_overlay, flags_overlay, formation_cadence_overlay, microcode_overlay,
    microcycle_overlay, pc_overlay, register_overlay, render_native_base, shield_overlay,
    shift_register_overlay, stack_overlay, timing_overlay,
};

fn apply_native_pipeline(
    svg: String,
    topology: &leader_core::Topology,
    trace: &leader_core::MatchTrace,
    config: RenderConfig,
) -> String {
    let svg = director::apply_camera(svg, topology, trace, config);
    let svg = pc_overlay::apply(svg, topology, trace, config);
    let svg = decoder_overlay::apply(svg, topology, trace, config);
    let svg = microcode_overlay::apply(svg, topology, trace, config);
    let svg = control_word_overlay::apply(svg, topology, trace, config);
    let svg = control_state_overlay::apply(svg, topology, trace, config);
    let svg = microcycle_overlay::apply(svg, topology, trace, config);
    let svg = alu_overlay::apply(svg, topology, trace, config);
    let svg = flags_overlay::apply(svg, topology, trace, config);
    let svg = register_overlay::apply(svg, topology, trace, config);
    let svg = bus_overlay::apply(svg, topology, trace, config);
    let svg = stack_overlay::apply(svg, topology, trace, config);
    let svg = formation_cadence_overlay::apply(svg, topology, trace, config);
    let svg = shift_register_overlay::apply(svg, topology, trace, config);
    let svg = enemy_shot_overlay::apply(svg, topology, trace, config);
    let svg = shield_overlay::apply(svg, topology, trace, config);
    timing_overlay::apply(svg, topology, trace, config)
}

#[test]
fn production_base_suppresses_only_legacy_semantic_activity() {
    let topology = build_topology();
    let trace = Machine::run_match("f3-native-base", 120);
    let config = RenderConfig::default();

    let with_legacy_activity = render(&topology, &trace, config);
    let production_base = render_native_base(&topology, &trace, config);

    let mut explicit_native_base = trace.clone();
    explicit_native_base.micro_samples.clear();
    let expected = render(&topology, &explicit_native_base, config);

    assert_eq!(production_base, expected);
    assert_ne!(production_base, with_legacy_activity);
    assert!(production_base.contains("GAME CLEAR") || production_base.contains("TRACE LIMIT"));
}

#[test]
fn complete_native_overlay_pipeline_is_native_only_without_materialization() {
    let topology = build_topology();
    let trace = Machine::run_match("f3-native-pipeline", 5000);
    let config = RenderConfig::default();

    assert!(!trace.micro_cycles.is_empty());
    assert!(!trace.micro_addresses.is_empty());
    assert!(!trace.bus_transactions.is_empty());
    assert!(!trace.alu_events.is_empty());
    assert!(!trace.flag_events.is_empty());
    assert!(!trace.control_latch_events.is_empty());
    assert!(!trace.formation_cadence_events.is_empty());
    assert!(!trace.register_writes.is_empty());
    assert!(!trace.pc_events.is_empty());
    assert!(!trace.sp_events.is_empty());
    assert!(!trace.shift_register_events.is_empty());
    assert!(trace
        .frames
        .iter()
        .any(|frame| frame.enemy_shots.iter().flatten().count() >= 2));
    assert!(trace.bus_transactions.iter().any(|event| matches!(
        event.control,
        "SHIELD_DAMAGE_PLAYER" | "SHIELD_DAMAGE_ENEMY"
    )));

    let base = render_native_base(&topology, &trace, config);
    let baseline = apply_native_pipeline(base.clone(), &topology, &trace, config);

    let mut native_only = trace.clone();
    native_only.micro_samples.clear();
    let without_semantic_samples = apply_native_pipeline(base, &topology, &native_only, config);

    assert_eq!(without_semantic_samples, baseline);
    assert!(baseline.contains("id=\"m3-shift-register\""));
    assert!(baseline.contains("data-shift-result=\"A0\""));
    assert!(baseline.contains("id=\"m3-formation-cadence\""));
    assert!(baseline.contains("data-cadence-divisor=\"3\""));
    assert!(baseline.contains("data-cadence-tick=\"1\""));
    assert!(baseline.contains("data-cadence-tick=\"0\""));
    assert!(baseline.contains("id=\"m3-enemy-shot-bank\""));
    assert!(baseline.contains("data-enemy-shot-slot=\"0\""));
    assert!(baseline.contains("data-enemy-shot-slot=\"1\""));
    assert!(baseline.contains("data-enemy-shot-slot=\"2\""));
    assert!(baseline.contains("data-enemy-shot-active-count=\"2\"")
        || baseline.contains("data-enemy-shot-active-count=\"3\""));
    assert!(baseline.contains("id=\"m3-shield-bank\""));
    assert!(baseline.contains("data-shield-mask=\""));
    assert!(baseline.contains("data-shield-before=\""));
    assert!(baseline.contains("data-shield-after=\""));
}
