use std::fmt::Write as _;

use leader_core::MatchTrace;
use leader_svg::RenderConfig;

const FINAL_OUTER_X: f32 = 180.0;
const FINAL_OUTER_Y: f32 = 35.0;
const FINAL_OUTER_W: f32 = 840.0;
const FINAL_OUTER_H: f32 = 600.0;
const FINAL_RASTER_X: f32 = 220.0;
const FINAL_RASTER_Y: f32 = 75.0;
const FINAL_RASTER_SCALE: f32 = 5.9375;
const FOCUS_ZOOM_FROM: f32 = 0.42;
const ROOT_CENTER_X: f32 = 600.0;
const ROOT_CENTER_Y: f32 = 337.5;

/// Adds a terminal camera shot that is sourced from the exact final native VRAM
/// checkpoint. The regular sidebar CRT remains authoritative during gameplay;
/// this overlay only becomes visible during the outro and then stays on screen
/// until the deterministic SVG loop restarts.
#[must_use]
pub fn apply(mut svg: String, trace: &MatchTrace, config: RenderConfig) -> String {
    if !svg.contains("data-frontpage-version=\"physical-die-v2\"") {
        return svg;
    }
    let Some(checkpoint) = trace.vram_checkpoints.last() else {
        return svg;
    };
    let Some(path) = final_checkpoint_path(&svg, checkpoint.frame) else {
        return svg;
    };

    let focus_start = (config.game_end() + 0.35).min(config.total() - 1.25);
    let focus_end = (focus_start + 1.05).min(config.total() - 0.15);
    let k1 = normalized(focus_start, config.total());
    let k2 = normalized(focus_end, config.total()).max(k1 + 0.000_01);
    let zoom_tx = ROOT_CENTER_X * (1.0 - FOCUS_ZOOM_FROM);
    let zoom_ty = ROOT_CENTER_Y * (1.0 - FOCUS_ZOOM_FROM);

    let mut overlay = String::with_capacity(path.len() + 4_096);
    let _ = writeln!(
        overlay,
        r##"<g id="v2-final-crt-focus" opacity="0" data-final-focus="native-vram" data-vram-frame="{}" data-vram-checksum="{:08X}" data-focus-start="{focus_start:.3}" data-focus-end="{focus_end:.3}">
<animate attributeName="opacity" values="0;0;1;1" keyTimes="0;{k1:.7};{k2:.7};1" keySplines="0 0 1 1;0.16 1 0.3 1;0 0 1 1" calcMode="spline" dur="{:.3}s" repeatCount="indefinite"/>
<animateTransform attributeName="transform" attributeType="XML" type="matrix" values="{FOCUS_ZOOM_FROM:.5} 0 0 {FOCUS_ZOOM_FROM:.5} {zoom_tx:.5} {zoom_ty:.5};{FOCUS_ZOOM_FROM:.5} 0 0 {FOCUS_ZOOM_FROM:.5} {zoom_tx:.5} {zoom_ty:.5};1 0 0 1 0 0;1 0 0 1 0 0" keyTimes="0;{k1:.7};{k2:.7};1" keySplines="0 0 1 1;0.16 1 0.3 1;0 0 1 1" calcMode="spline" dur="{:.3}s" repeatCount="indefinite"/>
<rect x="0" y="0" width="1200" height="675" fill="#020406" opacity=".985"/>
<rect x="{FINAL_OUTER_X}" y="{FINAL_OUTER_Y}" width="{FINAL_OUTER_W}" height="{FINAL_OUTER_H}" rx="34" fill="#071019" stroke="#657d89" stroke-width="5"/>
<rect x="194" y="49" width="812" height="572" rx="27" fill="#030807" stroke="#243c35" stroke-width="2"/>
<rect x="{FINAL_RASTER_X}" y="{FINAL_RASTER_Y}" width="760" height="570" rx="13" fill="#010302" stroke="#41614f" stroke-width="2"/>
<path d="{path}" transform="translate({FINAL_RASTER_X:.3} {FINAL_RASTER_Y:.3}) scale({FINAL_RASTER_SCALE:.7})" fill="#b9ff78" shape-rendering="crispEdges" data-final-native-raster="128x96"/>
<rect x="{FINAL_RASTER_X}" y="{FINAL_RASTER_Y}" width="760" height="4" fill="#d7ffbc" opacity=".055"><animate attributeName="y" values="{FINAL_RASTER_Y};641;{FINAL_RASTER_Y}" dur="2.3s" repeatCount="indefinite"/></rect>
<path d="M232 86 C390 63 808 63 968 86" fill="none" stroke="#e8ffe0" stroke-width="2" opacity=".055"/>
<text x="{FINAL_RASTER_X}" y="65" fill="#8fb09b" font-size="10" font-weight="900">FINAL NATIVE VRAM · FRAME {:05} · CHECKSUM {:08X}</text>
<text x="980" y="65" text-anchor="end" fill="#657f70" font-size="9" font-weight="900">VRAM → DMA → SCANOUT · 128×96 · 1 BPP</text>
</g>"##,
        checkpoint.frame,
        checkpoint.checksum,
        config.total(),
        config.total(),
        checkpoint.frame,
        checkpoint.checksum,
    );

    if let Some(index) = svg.rfind("</svg>") {
        svg.insert_str(index, &overlay);
    }
    svg
}

fn final_checkpoint_path(svg: &str, frame: u32) -> Option<String> {
    let marker = format!("data-vram-frame=\"{frame}\"");
    let marker_index = svg.rfind(&marker)?;
    let path_start = svg[..marker_index].rfind("<path class=\"v2-crt-pixel\"")?;
    let opening_end = svg[path_start..].find('>')? + path_start;
    let opening = &svg[path_start..=opening_end];
    attribute_value(opening, "d")
}

fn attribute_value(element: &str, attribute: &str) -> Option<String> {
    let marker = format!("{attribute}=\"");
    let start = element.find(&marker)? + marker.len();
    let end = element[start..].find('"')? + start;
    Some(element[start..end].to_owned())
}

fn normalized(time: f32, total: f32) -> f32 {
    (time / total.max(0.001)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::Machine;

    #[test]
    fn final_focus_reuses_the_last_native_vram_path() {
        let trace = Machine::run_match("final-crt-focus", 5000);
        let checkpoint = trace.vram_checkpoints.last().expect("native VRAM checkpoint");
        let source = format!(
            "<svg data-frontpage-version=\"physical-die-v2\"><path class=\"v2-crt-pixel\" d=\"M0 0h1v1h-1z\" opacity=\"0\" data-vram-frame=\"{}\" data-vram-checksum=\"{:08X}\"></path></svg>",
            checkpoint.frame, checkpoint.checksum,
        );
        let output = apply(source, &trace, crate::frontpage::render_config());
        assert!(output.contains("id=\"v2-final-crt-focus\""));
        assert!(output.contains("data-final-focus=\"native-vram\""));
        assert!(output.contains(&format!("data-vram-frame=\"{}\"", checkpoint.frame)));
        assert!(output.contains("d=\"M0 0h1v1h-1z\" transform=\"translate(220.000 75.000) scale(5.9375000)\""));
        assert!(output.contains("data-final-native-raster=\"128x96\""));
    }

    #[test]
    fn final_focus_remains_declarative_and_holds_until_loop_reset() {
        let trace = Machine::run_match("final-crt-declarative", 5000);
        let checkpoint = trace.vram_checkpoints.last().expect("native VRAM checkpoint");
        let source = format!(
            "<svg data-frontpage-version=\"physical-die-v2\"><path class=\"v2-crt-pixel\" d=\"M0 0h1v1h-1z\" data-vram-frame=\"{}\"></path></svg>",
            checkpoint.frame,
        );
        let output = apply(source, &trace, crate::frontpage::render_config());
        assert!(output.contains("values=\"0;0;1;1\""));
        assert!(output.contains("type=\"matrix\""));
        assert!(!output.contains("<script"));
        assert!(!output.contains("javascript:"));
    }
}
