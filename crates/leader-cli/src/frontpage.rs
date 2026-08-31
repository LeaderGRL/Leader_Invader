use leader_core::{MatchTrace, Topology};
use leader_svg::RenderConfig;

mod physical_die {
    include!("frontpage_v2.rs");
}

/// The GitHub front page is not a cinematic assembly sequence. The entire
/// physical machine exists from t=0, then native activity starts almost
/// immediately so a repository visitor sees causal hardware propagation within
/// the first few seconds.
#[must_use]
pub fn render(topology: &Topology, trace: &MatchTrace, _legacy_config: RenderConfig) -> String {
    physical_die::render(
        topology,
        trace,
        RenderConfig {
            width: 1200,
            height: 675,
            assembly_seconds: 1.5,
            boot_seconds: 1.0,
            game_seconds: 52.0,
            outro_seconds: 4.5,
        },
    )
}
