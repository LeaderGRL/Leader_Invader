/// Applies final SVG presentation invariants that are easier to express once
/// the complete fixed-stage document has been serialized.
#[must_use]
pub fn apply(mut svg: String) -> String {
    // Open routed paths must never inherit the signal swatch fill. Otherwise
    // SVG implicitly closes them and paints large polygons between modules.
    svg = svg.replace(
        ".backbone{fill:none;stroke:#314b60;",
        ".backbone{fill:none!important;stroke:#314b60;",
    );
    svg = svg.replace(
        ".detail-wire{fill:none;stroke-width:1.2;",
        ".detail-wire{fill:none!important;stroke-width:1.2;",
    );

    // The detail and CRT content are already mathematically fit to their
    // panels. A clip path attached to a transformed child is evaluated in that
    // child's user space by SVG, which can move the clip away from the panel.
    // Keep clipping neutral here instead of hiding correctly fitted content.
    svg = svg.replace(
        "<clipPath id=\"clip-detail\"><rect x=\"596\" y=\"162\" width=\"568\" height=\"248\" rx=\"10\"/></clipPath>",
        "<clipPath id=\"clip-detail\"><rect x=\"-10000\" y=\"-10000\" width=\"30000\" height=\"30000\"/></clipPath>",
    );
    svg = svg.replace(
        "<clipPath id=\"clip-crt\"><rect x=\"606\" y=\"474\" width=\"244\" height=\"126\" rx=\"11\"/></clipPath>",
        "<clipPath id=\"clip-crt\"><rect x=\"-1000\" y=\"-1000\" width=\"3000\" height=\"3000\"/></clipPath>",
    );

    svg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_signal_paths_are_forced_to_stay_unfilled() {
        let source = String::from(
            "<style>.backbone{fill:none;stroke:#314b60;}.detail-wire{fill:none;stroke-width:1.2;}</style>",
        );
        let output = apply(source);
        assert!(output.contains(".backbone{fill:none!important;"));
        assert!(output.contains(".detail-wire{fill:none!important;"));
    }

    #[test]
    fn transformed_content_is_not_hidden_by_root_space_clips() {
        let source = String::from(
            "<clipPath id=\"clip-detail\"><rect x=\"596\" y=\"162\" width=\"568\" height=\"248\" rx=\"10\"/></clipPath><clipPath id=\"clip-crt\"><rect x=\"606\" y=\"474\" width=\"244\" height=\"126\" rx=\"11\"/></clipPath>",
        );
        let output = apply(source);
        assert!(output.contains("x=\"-10000\""));
        assert!(output.contains("x=\"-1000\""));
    }
}
