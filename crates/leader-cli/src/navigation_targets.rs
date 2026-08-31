use leader_core::{build_navigation, NavigationModel, Topology};

#[must_use]
pub fn apply(mut svg: String, topology: &Topology) -> String {
    let navigation = build_navigation(topology);
    annotate_targets(&mut svg, &navigation);
    svg
}

fn annotate_targets(svg: &mut String, navigation: &NavigationModel) {
    let Some(root) = navigation.view(&navigation.default_view) else {
        return;
    };

    for module in navigation
        .modules
        .iter()
        .filter(|module| module.level == leader_core::NavigationLevel::Subsystem)
    {
        for node_id in &module.node_ids {
            let path = navigation.view_path_for_node(node_id);
            let target = navigation.deepest_view_for_node(node_id);
            let path_value = path
                .iter()
                .map(|view| view.id.as_str())
                .collect::<Vec<_>>()
                .join("/");
            let target_id = target.map_or(root.id.as_str(), |view| view.id.as_str());
            let parent_target = target
                .and_then(|view| view.parent_view.as_deref())
                .unwrap_or(root.id.as_str());
            insert_node_attribute(
                svg,
                node_id,
                &format!(
                    " data-target-view=\"{}\" data-parent-view=\"{}\" data-view-path=\"{}\"",
                    xml_escape(target_id),
                    xml_escape(parent_target),
                    xml_escape(&path_value)
                ),
            );
        }
    }
}

fn insert_node_attribute(svg: &mut String, node_id: &str, attribute: &str) {
    let marker = format!("<g id=\"node-{}\"", xml_escape(node_id));
    let Some(start) = svg.find(&marker) else {
        return;
    };
    let insert_at = start + marker.len();
    svg.insert_str(insert_at, attribute);
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

    #[test]
    fn physical_nodes_get_deepest_target_parent_and_full_path() {
        let topology = leader_core::build_topology();
        let source = concat!(
            "<svg>",
            "<g id=\"node-microRom\"></g>",
            "<g id=\"node-shieldAddr\"></g>",
            "<g id=\"node-display\"></g>",
            "</svg>"
        )
        .to_owned();
        let output = apply(source, &topology);

        assert!(output.contains(
            "id=\"node-microRom\" data-target-view=\"view-decode.microcode\" data-parent-view=\"view-decode\" data-view-path=\"view-machine/view-decode/view-decode.microcode\""
        ));
        assert!(output.contains(
            "id=\"node-shieldAddr\" data-target-view=\"view-io.shields\" data-parent-view=\"view-io\" data-view-path=\"view-machine/view-io/view-io.shields\""
        ));
        assert!(output.contains("id=\"node-display\" data-target-view=\"view-gpu.scanout\""));
    }

    #[test]
    fn every_rendered_physical_node_receives_a_navigation_target() {
        let topology = leader_core::build_topology();
        let mut source = String::from("<svg>");
        for node in &topology.nodes {
            source.push_str(&format!("<g id=\"node-{}\"></g>", node.id));
        }
        source.push_str("</svg>");
        let output = apply(source, &topology);
        assert_eq!(output.matches("data-target-view=\"").count(), topology.nodes.len());
        assert_eq!(output.matches("data-view-path=\"").count(), topology.nodes.len());
    }
}
