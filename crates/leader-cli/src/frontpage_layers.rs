/// Reorders signal propagation so active buses/carry lines run beneath physical
/// components instead of painting over node bodies and labels.
///
/// The groups moved here contain only `<use>` elements and animations, never
/// nested `<g>` elements, so their first closing group tag is their own end.
#[must_use]
pub fn apply(mut svg: String) -> String {
    if !svg.contains("data-frontpage-version=\"physical-die-v2\"") {
        return svg;
    }

    move_flat_group_before(&mut svg, "v2-native-bus-propagation", "<g id=\"v2-logic-nodes\">");
    move_flat_group_before(&mut svg, "v2-native-alu-propagation", "<g id=\"v2-logic-nodes\">");
    svg
}

fn move_flat_group_before(svg: &mut String, group_id: &str, target: &str) {
    let marker = format!("<g id=\"{group_id}\">");
    let Some(start) = svg.find(&marker) else {
        return;
    };
    let Some(end_rel) = svg[start..].find("</g>") else {
        return;
    };
    let end = start + end_rel + "</g>".len();
    let mut group = svg[start..end].to_string();
    group.push('\n');
    svg.replace_range(start..end, "");

    let Some(target_index) = svg.find(target) else {
        return;
    };
    svg.insert_str(target_index, &group);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_signal_layers_are_before_node_bodies() {
        let source = concat!(
            "<svg data-frontpage-version=\"physical-die-v2\">",
            "<g id=\"v2-logic-nodes\"></g>",
            "<g id=\"v2-native-bus-propagation\"><use><animate/></use></g>",
            "<g id=\"v2-native-alu-propagation\"><use><animate/></use></g>",
            "</svg>"
        )
        .to_string();
        let output = apply(source);
        let node = output.find("v2-logic-nodes").unwrap();
        let bus = output.find("v2-native-bus-propagation").unwrap();
        let alu = output.find("v2-native-alu-propagation").unwrap();
        assert!(bus < node);
        assert!(alu < node);
    }
}
