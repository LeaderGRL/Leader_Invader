use leader_core::{build_topology, Machine};
use leader_svg::{render, RenderConfig};

use crate::{decoder_overlay, director, microcode_overlay, pc_overlay, stack_overlay, timing_overlay};

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
    let svg = stack_overlay::apply(svg, topology, trace, config);
    timing_overlay::apply(svg, topology, trace, config)
}

#[test]
fn complete_f3_overlay_pipeline_is_native_only() {
    let topology = build_topology();
    let trace = Machine::run_match("f3-native-pipeline", 5000);
    let config = RenderConfig::default();

    // The base artifact is intentionally rendered once from the complete trace.
    // This test isolates the F3 overlay stack from leader-svg's legacy coarse
    // activity layer and verifies that every high-fidelity overlay is native-only.
    let base = render(&topology, &trace, config);
    let baseline = apply_f3_pipeline(base.clone(), &topology, &trace, config);

    let mut native_only = trace.clone();
    native_only.micro_samples.clear();
    let without_semantic_samples = apply_f3_pipeline(base, &topology, &native_only, config);

    assert_eq!(without_semantic_samples, baseline);
}
