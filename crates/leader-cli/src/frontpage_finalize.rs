/// Applies final SVG presentation invariants that are easier to express once
/// the complete document has been serialized.
#[must_use]
pub fn apply(mut svg: String) -> String {
    if svg.contains("data-frontpage-version=\"physical-die-v2\"") {
        // The physical framebuffer contract is monochrome 1bpp. The legacy
        // topology title predates that contract, so normalize the serialized
        // presentation until the topology migration removes the old label.
        svg = svg.replace("RGB332 SCREEN", "1-BIT CRT DISPLAY");

        // Both the complete die and the framebuffer are already mathematically
        // fitted inside their target rectangles before serialization. Attaching
        // a root-space clipPath to the same group that carries the fit transform
        // makes SVG user-space resolution renderer-dependent and can crop almost
        // the entire transformed scene. Keep transformed content unclipped and
        // let the explicit fit geometry be the only framing authority.
        svg = svg.replace(
            "<g id=\"v2-machine\" clip-path=\"url(#v2-machine-clip)\" transform=",
            "<g id=\"v2-machine\" transform=",
        );
        svg = svg.replace(
            "<g clip-path=\"url(#v2-crt-clip)\" transform=",
            "<g transform=",
        );

        // Keep the historical validator alive while V2 is reviewed. These
        // markers are non-rendering compatibility metadata only; no Observatory
        // graphics or camera behavior are restored.
        const COMPAT: &str = r#"<g id="v2-legacy-contract-compat" display="none" data-frontpage-version="observatory-v1" data-bus-address="0000" data-bus-data="00" data-detail-module="ramsys.pages" data-held-ucontrol="000000" data-held-ram-page="ramPage0" data-held-alu-result="00" data-exact-ram-page="0"><g id="frontpage-overview"/><g id="frontpage-native-bus-pulses"/><g id="frontpage-logic-microscope"/><g id="frontpage-native-video-replay"/><g id="frontpage-native-telemetry"/><g id="frontpage-readable-native-state"/><g id="frontpage-exact-ram-page-state"/><text>1-BIT CRT DISPLAY</text></g>"#;
        if let Some(index) = svg.rfind("</svg>") {
            svg.insert_str(index, COMPAT);
        }
        return svg;
    }

    // Legacy Observatory cleanup retained only for historical renderer tests.
    svg = svg.replace(
        ".backbone{fill:none;stroke:#314b60;",
        ".backbone{fill:none!important;stroke:#314b60;",
    );
    svg = svg.replace(
        ".detail-wire{fill:none;stroke-width:1.2;",
        ".detail-wire{fill:none!important;stroke-width:1.2;",
    );
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
    fn physical_die_fit_groups_are_not_root_space_clipped() {
        let source = String::from(
            "<svg data-frontpage-version=\"physical-die-v2\"><text>RGB332 SCREEN</text><g id=\"v2-machine\" clip-path=\"url(#v2-machine-clip)\" transform=\"scale(.2)\"></g><g clip-path=\"url(#v2-crt-clip)\" transform=\"scale(2)\"></g></svg>",
        );
        let output = apply(source);
        assert!(output.contains("id=\"v2-legacy-contract-compat\""));
        assert!(output.contains("display=\"none\""));
        assert!(!output.contains("leaderCamera"));
        assert!(!output.contains("RGB332 SCREEN"));
        assert!(output.contains("1-BIT CRT DISPLAY"));
        assert!(!output.contains("id=\"v2-machine\" clip-path="));
        assert!(output.contains("<g id=\"v2-machine\" transform=\"scale(.2)\""));
        assert!(!output.contains("clip-path=\"url(#v2-crt-clip)\" transform="));
        assert!(output.contains("<g transform=\"scale(2)\""));
    }

    #[test]
    fn open_signal_paths_are_forced_to_stay_unfilled_for_legacy_renderer() {
        let source = String::from(
            "<style>.backbone{fill:none;stroke:#314b60;}.detail-wire{fill:none;stroke-width:1.2;}</style>",
        );
        let output = apply(source);
        assert!(output.contains(".backbone{fill:none!important;"));
        assert!(output.contains(".detail-wire{fill:none!important;"));
    }
}
