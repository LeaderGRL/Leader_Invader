use leader_core::{MatchTrace, Topology};
use leader_svg::RenderConfig;

mod physical_die {
    include!("frontpage_v2.rs");
}

mod bitfabric {
    include!("frontpage_bitfabric.rs");
}

mod layers {
    include!("frontpage_layers.rs");
}

mod quality {
    include!("frontpage_quality.rs");
}

mod crt_contract {
    include!("frontpage_crt_contract.rs");
}

mod camera {
    include!("frontpage_camera.rs");
}

mod final_crt {
    include!("frontpage_final_crt.rs");
}

/// Canonical timing for the GitHub front-page artifact.
///
/// The full machine is visible from t=0. Native propagation starts after a
/// short power-on interval, then a deterministic technical camera exposes the
/// active low-level subsystem at a readable scale. The outro resolves onto the
/// exact final native VRAM checkpoint and holds it until the SVG loop resets.
#[must_use]
pub const fn render_config() -> RenderConfig {
    RenderConfig {
        width: 1200,
        height: 675,
        assembly_seconds: 1.5,
        boot_seconds: 1.0,
        game_seconds: 52.0,
        outro_seconds: 4.5,
    }
}

/// Render the complete physical machine, enrich it with true memory/microcode
/// bit-cell fabrics, layer electrical propagation beneath component bodies,
/// enforce scale-independent readability and CRT continuity, then apply a
/// trace-driven camera. The terminal shot is sourced from the same native VRAM
/// checkpoint already rendered by the physical CRT; it never reconstructs the
/// game from semantic state.
#[must_use]
pub fn render(topology: &Topology, trace: &MatchTrace, _legacy_config: RenderConfig) -> String {
    let config = render_config();
    let svg = physical_die::render(topology, trace, config);
    let svg = bitfabric::apply(svg, topology, trace);
    let svg = layers::apply(svg);
    let svg = quality::apply(svg, topology);
    let svg = crt_contract::apply(svg, config);
    let svg = camera::apply(svg, topology, trace, config);
    final_crt::apply(svg, trace, config)
}
