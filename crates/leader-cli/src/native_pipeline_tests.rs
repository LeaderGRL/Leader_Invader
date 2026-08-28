use leader_core::{build_topology, Machine};
use leader_svg::{render, RenderConfig};

use crate::{
    control_state_overlay, control_word_overlay, decoder_overlay, director, microcode_overlay,
    pc_overlay, render_native_base, stack_overlay, timing_overlay,
};

fn apply_f3_pipeline(
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
    let svg = stack_overlay::apply(svg, topology, trace, config);
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
fn complete_f3_overlay_pipeline_is_native_only() {
    let topology = build_topology();
    let trace = Machine::run_match("f3-native-pipeline", 120);
    let config = RenderConfig::default();

    let base = render_native_base(&topology, &trace, config);
    let baseline = apply_f3_pipeline(base.clone(), &topology, &trace, config);

    let mut native_only = trace.clone();
    native_only.micro_samples.clear();
    let without_semantic_samples = apply_f3_pipeline(base, &topology, &native_only, config);

    assert_eq!(without_semantic_samples, baseline);
}
