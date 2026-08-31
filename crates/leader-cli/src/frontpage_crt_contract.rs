use leader_svg::RenderConfig;

/// Keep the final native VRAM checkpoint visible through the outro.
///
/// The physical die renderer intentionally ends gameplay at `game_end()`, but
/// the GitHub front page continues for a short outro. A blank CRT during that
/// interval looks like a rendering failure, so only the final checkpoint is
/// held until the SVG loop resets. Earlier checkpoints remain mutually
/// exclusive and retain their native timing.
#[must_use]
pub fn apply(mut svg: String, config: RenderConfig) -> String {
    const PATH_MARKER: &str = "<path class=\"v2-crt-pixel\"";
    const ANIMATE_PREFIX: &str = "<animate attributeName=\"opacity\" values=\"0;1;0;0\" keyTimes=\"0;";

    let Some(path_start) = svg.rfind(PATH_MARKER) else {
        return svg;
    };
    let Some(path_end_rel) = svg[path_start..].find("</path>") else {
        return svg;
    };
    let path_end = path_start + path_end_rel + "</path>".len();
    let segment = &svg[path_start..path_end];
    let Some(animate_rel) = segment.find(ANIMATE_PREFIX) else {
        return svg;
    };
    let animate_start = path_start + animate_rel;
    let key_start = animate_start + ANIMATE_PREFIX.len();
    let Some(key_end_rel) = svg[key_start..path_end].find(';') else {
        return svg;
    };
    let key_end = key_start + key_end_rel;
    let first_visible_key = svg[key_start..key_end].to_string();
    let Some(animate_end_rel) = svg[animate_start..path_end].find("/>") else {
        return svg;
    };
    let animate_end = animate_start + animate_end_rel + 2;

    let replacement = format!(
        "<animate attributeName=\"opacity\" values=\"0;1;1\" keyTimes=\"0;{first_visible_key};1\" calcMode=\"discrete\" dur=\"{:.3}s\" repeatCount=\"indefinite\"/>",
        config.total(),
    );
    svg.replace_range(animate_start..animate_end, &replacement);
    svg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_last_crt_checkpoint_is_held_through_outro() {
        let source = concat!(
            "<svg>",
            "<path class=\"v2-crt-pixel\"><animate attributeName=\"opacity\" values=\"0;1;0;0\" keyTimes=\"0;.1;.2;1\" calcMode=\"discrete\" dur=\"59.000s\" repeatCount=\"indefinite\"/></path>",
            "<path class=\"v2-crt-pixel\"><animate attributeName=\"opacity\" values=\"0;1;0;0\" keyTimes=\"0;.8;.9;1\" calcMode=\"discrete\" dur=\"59.000s\" repeatCount=\"indefinite\"/></path>",
            "</svg>"
        )
        .to_string();
        let config = crate::frontpage::render_config();
        let output = apply(source, config);
        assert_eq!(output.matches("values=\"0;1;0;0\"").count(), 1);
        assert_eq!(output.matches("values=\"0;1;1\"").count(), 1);
        assert!(output.contains("keyTimes=\"0;.8;1\""));
    }
}
