use leader_core::{MatchTrace, Topology};
use leader_svg::RenderConfig;

mod physical_die {
    include!("frontpage_v2.rs");
}

mod bitfabric {
    include!("frontpage_bitfabric.rs");
}

/// Canonical timing for the GitHub front-page artifact.
///
/// The full machine is visible from t=0. Native propagation starts after a
/// short power-on interval rather than after a cinematic assembly sequence.
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

/// The GitHub front page is not a cinematic assembly sequence. The entire
/// physical machine exists from t=0, then native activity starts almost
/// immediately so a repository visitor sees causal hardware propagation within
/// the first few seconds.
#[must_use]
pub fn render(topology: &Topology, trace: &MatchTrace, _legacy_config: RenderConfig) -> String {
    let svg = physical_die::render(topology, trace, render_config());
    bitfabric::apply(svg, topology, trace)
}
