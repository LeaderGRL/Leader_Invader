use std::fmt::Write as _;

use leader_core::{
    build_navigation, memory_owner, physical_activity_nodes, MatchTrace, MemoryOwner, PhaseKind, Rect,
    Topology,
};
use leader_svg::RenderConfig;

const DETAIL_VIEW: Rect = Rect {
    x: 610.0,
    y: 179.0,
    w: 540.0,
    h: 211.0,
};

/// Replaces the ambiguous RAM decoder highlight with the exact numbered page.
///
/// `ramPageDec` intentionally shares the `ramPage` prefix with `ramPage0..95`,
/// so presentation code must explicitly require an all-numeric suffix.
#[must_use]
pub fn apply(mut svg: String, topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    // Hide the earlier ambiguous highlight while keeping its native metadata in
    // the document. The exact overlay below becomes the visible authority.
    svg = svg.replace(
        "stroke=\"#96edff\" stroke-width=\"2.6\" filter=\"url(#leader-soft-glow)\"",
        "stroke=\"none\" stroke-width=\"0\"",
    );

    // The physical framebuffer is monochrome 1bpp, not RGB332.
    svg = svg.replace("RGB332 SCREEN", "1-BIT CRT DISPLAY");

    let navigation = build_navigation(topology);
    let Some(module) = navigation.module("ramsys.pages") else {
        return svg;
    };
    let fit = fit_rect(module.bounds, DETAIL_VIEW, 8.0);

    let candidates = trace
        .bus_transactions
        .iter()
        .filter(|event| {
            let Some(address) = event.address else {
                return false;
            };
            let moment = trace_moment(event.frame, event.ordinal, trace, config);
            memory_owner(address) == MemoryOwner::Ram && (86.0..104.0).contains(&moment)
        })
        .collect::<Vec<_>>();
    let sampled = sample_refs(&candidates, 18);
    if sampled.is_empty() {
        return svg;
    }

    let total = config.total();
    let mut overlay = String::with_capacity(28_000);
    overlay.push_str("<g id=\"frontpage-exact-ram-page-state\" aria-hidden=\"true\">\n");

    for (index, event) in sampled.iter().enumerate() {
        let start = trace_moment(event.frame, event.ordinal, trace, config).max(86.0);
        let end = sampled
            .get(index + 1)
            .map_or(104.0, |next| trace_moment(next.frame, next.ordinal, trace, config))
            .clamp(start + 0.001, 104.0);
        let Some(address) = event.address else {
            continue;
        };
        let phase = if event.kind.as_str() == "write" {
            PhaseKind::MemoryWrite
        } else {
            PhaseKind::MemoryRead
        };
        let active = physical_activity_nodes(phase, Some(address));
        let page = active
            .iter()
            .find(|id| is_numbered_ram_page(id))
            .and_then(|id| topology.node(id));
        let Some(page) = page else {
            continue;
        };

        let page_number = page
            .id
            .strip_prefix("ramPage")
            .and_then(|suffix| suffix.parse::<usize>().ok())
            .unwrap_or(0);
        let b = screen_rect(page.bounds, fit);
        let k1 = norm(start, total);
        let k2 = norm(end, total).max(k1 + 0.000_01);
        let data = event.data.unwrap_or(0);

        let _ = writeln!(
            overlay,
            r##"<g opacity="0" data-exact-ram-page="{page_number}" data-exact-ram-address="{address:04X}" data-exact-ram-data="{data:02X}"><animate attributeName="opacity" values="0;1;0;0" keyTimes="0;{k1:.6};{k2:.6};1" calcMode="discrete" dur="{total:.3}s" repeatCount="indefinite"/><rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="4" fill="#61f2cf" fill-opacity=".30" stroke="#b8fff0" stroke-width="3" filter="url(#leader-glow)"/><rect x="610" y="384" width="540" height="24" rx="5" fill="#061019" stroke="#3b7d72"/><text x="621" y="400" fill="#adffed" font-size="9" font-weight="900">RAM {page_number:02X} · {} · A {address:04X} · D {data:02X} · {}</text></g>"##,
            b.x,
            b.y,
            b.w,
            b.h,
            xml_escape(event.kind.as_str()),
            xml_escape(event.control),
        );
    }

    overlay.push_str("</g>\n");
    if let Some(index) = svg.rfind("</svg>") {
        svg.insert_str(index, &overlay);
    }
    svg
}

fn is_numbered_ram_page(id: &str) -> bool {
    let Some(suffix) = id.strip_prefix("ramPage") else {
        return false;
    };
    !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
}

#[derive(Debug, Clone, Copy)]
struct Fit {
    scale: f32,
    tx: f32,
    ty: f32,
}

fn fit_rect(bounds: Rect, viewport: Rect, padding: f32) -> Fit {
    let safe_w = (viewport.w - padding * 2.0).max(1.0);
    let safe_h = (viewport.h - padding * 2.0).max(1.0);
    let scale = (safe_w / bounds.w.max(1.0)).min(safe_h / bounds.h.max(1.0));
    let rendered_w = bounds.w * scale;
    let rendered_h = bounds.h * scale;
    Fit {
        scale,
        tx: viewport.x + (viewport.w - rendered_w) * 0.5 - bounds.x * scale,
        ty: viewport.y + (viewport.h - rendered_h) * 0.5 - bounds.y * scale,
    }
}

fn screen_rect(bounds: Rect, fit: Fit) -> Rect {
    Rect::new(
        fit.tx + bounds.x * fit.scale,
        fit.ty + bounds.y * fit.scale,
        bounds.w * fit.scale,
        bounds.h * fit.scale,
    )
}

fn sample_refs<'a, T>(values: &'a [&'a T], maximum: usize) -> Vec<&'a T> {
    if values.len() <= maximum {
        return values.to_vec();
    }
    let stride = values.len().div_ceil(maximum);
    values.iter().step_by(stride).copied().collect()
}

fn trace_moment(frame: u32, ordinal: u16, trace: &MatchTrace, config: RenderConfig) -> f32 {
    config.game_start()
        + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
        + f32::from(ordinal.min(63)) * 0.0012
}

fn norm(value: f32, total: f32) -> f32 {
    (value / total).clamp(0.0, 1.0)
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
    use leader_core::{build_topology, Machine};

    #[test]
    fn numbered_ram_page_filter_rejects_decoder() {
        assert!(!is_numbered_ram_page("ramPageDec"));
        assert!(is_numbered_ram_page("ramPage0"));
        assert!(is_numbered_ram_page("ramPage95"));
    }

    #[test]
    fn exact_ram_overlay_contains_numbered_page_state() {
        let topology = build_topology();
        let trace = Machine::run_match("frontpage-exact-ram", 5000);
        let svg = apply(
            String::from("<svg>RGB332 SCREEN</svg>"),
            &topology,
            &trace,
            RenderConfig::default(),
        );
        assert!(svg.contains("id=\"frontpage-exact-ram-page-state\""));
        assert!(svg.contains("data-exact-ram-page=\""));
        assert!(!svg.contains("RGB332 SCREEN"));
        assert!(svg.contains("1-BIT CRT DISPLAY"));
    }
}
