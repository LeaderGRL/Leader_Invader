use leader_core::{MatchTrace, Topology};
use leader_svg::RenderConfig;

/// Legacy Observatory post-processing is intentionally disabled for the
/// physical-die front page. Native activity is rendered directly on canonical
/// physical wires by `frontpage_v2`, so adding held dashboard overlays here
/// would duplicate authority and reintroduce non-physical presentation.
#[must_use]
pub fn apply(svg: String, _topology: &Topology, _trace: &MatchTrace, _config: RenderConfig) -> String {
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::{build_topology, Machine};

    #[test]
    fn postprocessor_is_identity_for_physical_die_output() {
        let topology = build_topology();
        let trace = Machine::run_match("v2-live-identity", 5000);
        let source = String::from("<svg data-frontpage-version=\"physical-die-v2\"/>");
        assert_eq!(
            apply(source.clone(), &topology, &trace, RenderConfig::default()),
            source
        );
    }
}
