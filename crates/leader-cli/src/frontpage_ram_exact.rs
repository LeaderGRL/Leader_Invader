use leader_core::{MatchTrace, Topology};
use leader_svg::RenderConfig;

/// Exact RAM byte/page addressing now belongs directly to the physical-die
/// renderer through `resolve_physical_memory_byte`. Keep this historical stage
/// as an identity function so the production pipeline has a single visual
/// authority.
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
        let trace = Machine::run_match("v2-ram-identity", 5000);
        let source = String::from("<svg data-frontpage-version=\"physical-die-v2\"/>");
        assert_eq!(
            apply(source.clone(), &topology, &trace, RenderConfig::default()),
            source
        );
    }
}
