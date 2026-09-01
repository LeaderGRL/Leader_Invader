use std::fmt::Write as _;

use leader_core::Topology;

/// Applies presentation invariants that must hold at every camera scale.
///
/// The physical renderer deliberately owns geometry and trace timing. This
/// pass owns readability: camera-independent typography, exact node labels,
/// restrained dormant wiring, a dedicated hardware viewport, and a
/// pixel-perfect 4:3 CRT raster.
#[must_use]
pub fn apply(mut svg: String, topology: &Topology) -> String {
    if !svg.contains("data-frontpage-version=\"physical-die-v2\"") {
        return svg;
    }

    inject_quality_css(&mut svg);
    replace_legacy_labels(&mut svg, topology);
    reserve_crt_sidebar(&mut svg);
    repair_crt_raster(&mut svg);
    svg
}

fn inject_quality_css(svg: &mut String) {
    let css = r##"
/* Front-page readability contract: world-space typography is camera-safe. */
.v2-group{stroke-width:1.15;stroke-dasharray:14 11;fill-opacity:.065}
.v2-group-label{display:none}
#v2-logic-nodes text{display:none}
.v2-node{stroke-width:1.05}
.v2-wire{stroke-width:.58;opacity:.045}
.v2-active-wire{stroke-width:1.75;filter:none}
.v2-active-wire.v2-active-carry{stroke-width:2.65;filter:url(#v2-glow)}
#v3-group-labels text{font-size:15px;font-weight:900;letter-spacing:1.2px;fill:#7890a2;paint-order:stroke;stroke:#050b11;stroke-width:4px;stroke-linejoin:round}
#v3-node-labels .v3-title{font-weight:850;fill:#b8c9d4;paint-order:stroke;stroke:#071019;stroke-width:2px;stroke-linejoin:round}
#v3-node-labels .v3-kind{font-size:6.2px;font-weight:700;fill:#60798b;letter-spacing:.3px}
#v2-memory-bitcell-fabric{opacity:.90}
#v2-microcode-bitcell-fabric{opacity:.96}
#v2-crt .v2-crt-pixel{shape-rendering:crispEdges}
"##;
    if let Some(index) = svg.find("</style>") {
        svg.insert_str(index, css);
    }
}

fn replace_legacy_labels(svg: &mut String, topology: &Topology) {
    let mut group_labels = String::with_capacity(8_000);
    group_labels.push_str("<g id=\"v3-group-labels\" pointer-events=\"none\">\n");
    for group in &topology.groups {
        let x = group.bounds.x + 14.0;
        let y = group.bounds.y + 22.0;
        let label = xml_escape(&group.label);
        let _ = writeln!(
            group_labels,
            r##"<text data-label-for-group="{}" x="{x:.2}" y="{y:.2}">{label}</text>"##,
            xml_escape(&group.id),
        );
    }
    group_labels.push_str("</g>\n");

    let mut node_labels = String::with_capacity(120_000);
    node_labels.push_str("<g id=\"v3-node-labels\" pointer-events=\"none\">\n");
    for node in &topology.nodes {
        let bounds = node.bounds;
        let title_size = (bounds.h * 0.18).clamp(7.0, 10.5);
        let chars = ((bounds.w - 10.0) / (title_size * 0.61)).floor().max(2.0) as usize;
        let title = if node.id == "display" {
            "1-BIT CRT".to_string()
        } else {
            fit_text(&node.title, chars)
        };
        let x = bounds.x + 5.0;
        let y = bounds.y + title_size + 4.0;
        let _ = writeln!(
            node_labels,
            r##"<g data-label-for-node="{}"><text class="v3-title" x="{x:.2}" y="{y:.2}" font-size="{title_size:.2}px">{}</text>"##,
            xml_escape(&node.id),
            xml_escape(&title),
        );
        if bounds.h >= 58.0 {
            let kind = fit_text(&node.kind, ((bounds.w - 10.0) / 4.1).floor().max(2.0) as usize);
            let kind_y = bounds.y + bounds.h - 6.0;
            let _ = writeln!(
                node_labels,
                r##"<text class="v3-kind" x="{x:.2}" y="{kind_y:.2}">{}</text>"##,
                xml_escape(&kind),
            );
        }
        node_labels.push_str("</g>\n");
    }
    node_labels.push_str("</g>\n");

    if let Some(index) = svg.find("<g id=\"v2-static-wires\">") {
        svg.insert_str(index, &group_labels);
    }
    if let Some(index) = svg.find("<g id=\"v2-memory-byte-fabric\"") {
        svg.insert_str(index, &node_labels);
    }
}

fn reserve_crt_sidebar(svg: &mut String) {
    // The original physical-die clip occupied the full 1152px content width,
    // allowing camera content to appear underneath the fixed CRT. Reserve the
    // right-most 252px exclusively for video/probe UI instead.
    *svg = svg.replace(
        "<clipPath id=\"v2-machine-clip\"><rect x=\"24\" y=\"92\" width=\"1152\" height=\"548\" rx=\"9\"/></clipPath>",
        "<clipPath id=\"v2-machine-clip\"><rect x=\"24\" y=\"92\" width=\"900\" height=\"548\" rx=\"9\"/></clipPath>",
    );
}

fn repair_crt_raster(svg: &mut String) {
    // A transformed clipPath caused the same user-space bug that previously
    // cropped the physical die. The framebuffer already emits bounded pixels,
    // so clipping the transformed raster is unnecessary and harmful.
    *svg = svg.replace("clip-path=\"url(#v2-crt-clip)\" transform=", "transform=");

    // 128×96 is exactly 4:3. Use one uniform scale for both axes instead of
    // stretching 128 pixels to 200 px horizontally and 96 pixels to 144 px.
    *svg = svg.replace(
        "translate(946.000 127.000) scale(1.5625000 1.5000000)",
        "translate(950.000 127.000) scale(1.5000000 1.5000000)",
    );

    // Match the phosphor viewport itself to the same 192×144 raster. This also
    // makes screenshot comparison against native framebuffer pixels exact.
    *svg = svg.replace(
        "x=\"946\" y=\"127\" width=\"200\" height=\"144\" rx=\"8\" fill=\"url(#v2-crt)\"",
        "x=\"950\" y=\"127\" width=\"192\" height=\"144\" rx=\"8\" fill=\"url(#v2-crt)\"",
    );

    // The moving scanline can visually cut through sprites in deterministic
    // screenshots. The native framebuffer is the only content the CRT needs.
    if let Some(start) = svg.find("<rect x=\"946\" y=\"127\" width=\"200\" height=\"2\"") {
        if let Some(relative_end) = svg[start..].find("</rect>") {
            let end = start + relative_end + "</rect>".len();
            svg.replace_range(start..end, "");
        }
    }
}

fn fit_text(value: &str, maximum: usize) -> String {
    let count = value.chars().count();
    if count <= maximum {
        return value.to_string();
    }
    if maximum <= 1 {
        return "…".to_string();
    }
    value.chars().take(maximum - 1).collect::<String>() + "…"
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::build_topology;

    #[test]
    fn quality_pass_emits_one_readable_label_for_every_physical_node() {
        let topology = build_topology();
        let source = "<svg data-frontpage-version=\"physical-die-v2\"><style></style><g id=\"v2-static-wires\"></g><g id=\"v2-memory-byte-fabric\"></g></svg>".to_string();
        let output = apply(source, &topology);
        assert_eq!(output.matches("data-label-for-node=\"").count(), topology.nodes.len());
        assert_eq!(output.matches("data-label-for-group=\"").count(), topology.groups.len());
        assert!(output.contains("1-BIT CRT"));
    }

    #[test]
    fn quality_pass_removes_transformed_crt_clip_and_non_uniform_scale() {
        let topology = build_topology();
        let source = "<svg data-frontpage-version=\"physical-die-v2\"><style></style><g id=\"v2-static-wires\"></g><g id=\"v2-memory-byte-fabric\"></g><g clip-path=\"url(#v2-crt-clip)\" transform=\"translate(946.000 127.000) scale(1.5625000 1.5000000)\"></g></svg>".to_string();
        let output = apply(source, &topology);
        assert!(!output.contains("clip-path=\"url(#v2-crt-clip)\" transform="));
        assert!(output.contains("scale(1.5000000 1.5000000)"));
    }

    #[test]
    fn quality_pass_reserves_a_non_overlapping_crt_sidebar() {
        let topology = build_topology();
        let source = "<svg data-frontpage-version=\"physical-die-v2\"><defs><clipPath id=\"v2-machine-clip\"><rect x=\"24\" y=\"92\" width=\"1152\" height=\"548\" rx=\"9\"/></clipPath></defs><style></style><g id=\"v2-static-wires\"></g><g id=\"v2-memory-byte-fabric\"></g></svg>".to_string();
        let output = apply(source, &topology);
        assert!(output.contains("width=\"900\" height=\"548\""));
        assert!(!output.contains("width=\"1152\" height=\"548\""));
    }
}
