use std::fmt::Write as _;

use leader_core::{
    build_navigation, memory_owner, physical_activity_nodes, physical_alu_node_values, MatchTrace,
    MemoryOwner, PhaseKind, Rect, Topology,
};
use leader_svg::RenderConfig;

const DETAIL_VIEW: Rect = Rect {
    x: 610.0,
    y: 179.0,
    w: 540.0,
    h: 211.0,
};

/// Adds readable, held native states on top of the fast electrical activity.
///
/// The underlying event stream remains exact. This layer only samples native
/// events for presentation and holds each selected event until the next one so
/// a README viewer can actually read the state instead of seeing a one-frame
/// flash.
#[must_use]
pub fn apply(mut svg: String, topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    let mut overlay = String::with_capacity(220_000);
    overlay.push_str("<g id=\"frontpage-readable-native-state\" aria-hidden=\"true\">\n");
    render_held_bus_state(&mut overlay, topology, trace, config);
    render_held_microcode(&mut overlay, trace, config);
    render_held_alu(&mut overlay, topology, trace, config);
    render_held_ram(&mut overlay, topology, trace, config);
    render_held_microstate(&mut overlay, trace, config);
    overlay.push_str("</g>\n");

    if let Some(index) = svg.rfind("</svg>") {
        svg.insert_str(index, &overlay);
    }
    svg
}

fn render_held_bus_state(out: &mut String, _topology: &Topology, trace: &MatchTrace, config: RenderConfig) {
    if trace.bus_transactions.is_empty() || trace.total_frames == 0 {
        return;
    }
    let candidates = trace
        .bus_transactions
        .iter()
        .filter(|event| event.address.is_some())
        .collect::<Vec<_>>();
    let sampled = sample_refs(&candidates, 72);
    let total = config.total();

    for (index, event) in sampled.iter().enumerate() {
        let start = trace_moment(event.frame, event.ordinal, trace, config);
        if start < config.game_start() || start >= config.game_end() {
            continue;
        }
        let end = sampled
            .get(index + 1)
            .map_or(config.game_end(), |next| trace_moment(next.frame, next.ordinal, trace, config))
            .clamp(start + 0.001, config.game_end());
        let Some(address) = event.address else {
            continue;
        };
        let data = event.data.unwrap_or(0);
        let owner = memory_owner(address);
        let (target_x, target_y, target_w, target_h, target_label) = overview_target(owner, event.kind.as_str());
        let (k1, k2) = hold_window(start, end, total);
        let accent = owner_color(owner);
        let _ = writeln!(
            out,
            r##"<g opacity="0" data-held-bus-kind="{}" data-held-bus-address="{:04X}" data-held-bus-data="{:02X}"><animate attributeName="opacity" values="0;1;0;0" keyTimes="0;{k1:.6};{k2:.6};1" calcMode="discrete" dur="{total:.3}s" repeatCount="indefinite"/><rect x="{target_x:.1}" y="{target_y:.1}" width="{target_w:.1}" height="{target_h:.1}" rx="7" fill="none" stroke="{accent}" stroke-width="2.4" filter="url(#leader-soft-glow)"/><rect x="91" y="545" width="396" height="28" rx="5" fill="#061019" stroke="#315064"/><circle cx="104" cy="559" r="4" fill="{accent}"/><text x="116" y="557" fill="#8ba3b4" font-size="7" font-weight="900">{}</text><text x="116" y="567" fill="#dce9f1" font-size="9" font-weight="900">{}  A {:04X}  D {:02X}  → {}</text></g>"##,
            xml_escape(event.kind.as_str()),
            address,
            data,
            xml_escape(event.kind.as_str()),
            xml_escape(event.control),
            address,
            data,
            target_label,
        );
    }
}

fn render_held_microcode(out: &mut String, trace: &MatchTrace, config: RenderConfig) {
    let candidates = trace
        .micro_addresses
        .iter()
        .filter(|event| {
            let moment = trace_moment(event.frame, event.ordinal, trace, config);
            (46.0..68.0).contains(&moment)
        })
        .collect::<Vec<_>>();
    let sampled = sample_refs(&candidates, 18);
    let total = config.total();

    for (index, event) in sampled.iter().enumerate() {
        let start = trace_moment(event.frame, event.ordinal, trace, config).max(46.0);
        let end = sampled
            .get(index + 1)
            .map_or(68.0, |next| trace_moment(next.frame, next.ordinal, trace, config))
            .clamp(start + 0.001, 68.0);
        let (k1, k2) = hold_window(start, end, total);
        let _ = writeln!(
            out,
            r##"<g opacity="0" data-held-uaddr="{:02X}" data-held-ucontrol="{:06X}"><animate attributeName="opacity" values="0;1;0;0" keyTimes="0;{k1:.6};{k2:.6};1" calcMode="discrete" dur="{total:.3}s" repeatCount="indefinite"/><rect x="600" y="174" width="556" height="230" rx="9" fill="#07111a" stroke="#21394b"/><text x="612" y="195" fill="#728ca0" font-size="8" font-weight="900" letter-spacing="1.4">NATIVE CONTROL WORD [23:0]</text><text x="1144" y="195" text-anchor="end" fill="#ff91d8" font-size="9" font-weight="900">µADDR {:02X} · OP {:02X}</text>"##,
            event.address,
            event.control_bits,
            event.address,
            event.opcode,
        );

        for bit in 0..24_u32 {
            let column = bit % 12;
            let row = bit / 12;
            let x = 614.0 + column as f32 * 44.0;
            let y = 218.0 + row as f32 * 54.0;
            let on = event.control_bits & (1_u32 << bit) != 0;
            let fill = if on { "#7a205f" } else { "#0b1823" };
            let stroke = if on { "#ff91d8" } else { "#3d596c" };
            let text = if on { "#ffd5ef" } else { "#71899a" };
            let _ = write!(
                out,
                "<g data-control-bit=\"{bit}\" data-control-value=\"{}\"><rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"36\" height=\"30\" rx=\"4\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.4\"/><text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" fill=\"{text}\" font-size=\"7\" font-weight=\"900\">C{bit:02}</text><circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"2.8\" fill=\"{stroke}\"/></g>",
                u8::from(on),
                x + 18.0,
                y + 13.0,
                x + 18.0,
                y + 22.0,
            );
        }
        let _ = writeln!(
            out,
            r##"<rect x="614" y="334" width="528" height="54" rx="6" fill="#061019" stroke="#2d4a5f"/><text x="626" y="352" fill="#698397" font-size="7" font-weight="900">CONTROL ROM 256 × 24 · PHYSICAL µSEQUENCER</text><text x="626" y="370" fill="#ff91d8" font-size="10" font-weight="900">CTRL {:06X}</text><text x="748" y="370" fill="#d9e7ef" font-size="9" font-weight="900">{}</text><text x="1128" y="370" text-anchor="end" fill="#7d96a8" font-size="8" font-weight="900">SOURCE {}</text></g>"##,
            event.control_bits,
            xml_escape(event.label),
            xml_escape(event.source.as_str()),
        );
    }
}

fn render_held_alu(out: &mut String, topology: &Topology, trace: &MatchTrace, config: RenderConfig) {
    let navigation = build_navigation(topology);
    let Some(module) = navigation.module("alu.ripple") else {
        return;
    };
    let fit = fit_rect(module.bounds, DETAIL_VIEW, 8.0);
    let candidates = trace
        .alu_events
        .iter()
        .filter(|event| {
            let moment = trace_moment(event.frame, event.ordinal, trace, config);
            (68.0..86.0).contains(&moment)
        })
        .collect::<Vec<_>>();
    let sampled = sample_refs(&candidates, 16);
    let total = config.total();

    for (index, event) in sampled.iter().enumerate() {
        let start = trace_moment(event.frame, event.ordinal, trace, config).max(68.0);
        let end = sampled
            .get(index + 1)
            .map_or(86.0, |next| trace_moment(next.frame, next.ordinal, trace, config))
            .clamp(start + 0.001, 86.0);
        let (k1, k2) = hold_window(start, end, total);
        let _ = write!(
            out,
            "<g opacity=\"0\" data-held-alu-op=\"{}\" data-held-alu-result=\"{:02X}\"><animate attributeName=\"opacity\" values=\"0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};1\" calcMode=\"discrete\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>",
            xml_escape(event.trace.op.as_str()),
            event.trace.result,
        );
        for state in physical_alu_node_values(event.trace).into_iter().filter(|state| state.value) {
            let Some(node) = topology.node(&state.node_id) else {
                continue;
            };
            let b = screen_rect(node.bounds, fit);
            let _ = write!(
                out,
                "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"3\" fill=\"#ffe16a\" fill-opacity=\".18\" stroke=\"#ffe97f\" stroke-width=\"2\" filter=\"url(#leader-soft-glow)\"/>",
                b.x,
                b.y,
                b.w,
                b.h,
            );
        }
        let _ = writeln!(
            out,
            r##"<rect x="610" y="384" width="540" height="24" rx="5" fill="#061019" stroke="#604f2b"/><text x="621" y="400" fill="#ffe681" font-size="9" font-weight="900">{} · A {:02X} · B {:02X} · RESULT {:02X} · CARRY {:03X}</text></g>"##,
            xml_escape(event.trace.op.as_str()),
            event.trace.lhs,
            event.trace.rhs,
            event.trace.result,
            event.trace.carry_chain,
        );
    }
}

fn render_held_ram(out: &mut String, topology: &Topology, trace: &MatchTrace, config: RenderConfig) {
    let navigation = build_navigation(topology);
    let Some(module) = navigation.module("ramsys.pages") else {
        return;
    };
    let fit = fit_rect(module.bounds, DETAIL_VIEW, 8.0);
    let candidates = trace
        .bus_transactions
        .iter()
        .filter(|event| {
            let moment = trace_moment(event.frame, event.ordinal, trace, config);
            event.address.is_some()
                && memory_owner(event.address.unwrap_or(0)) == MemoryOwner::Ram
                && (86.0..104.0).contains(&moment)
        })
        .collect::<Vec<_>>();
    let sampled = sample_refs(&candidates, 18);
    let total = config.total();

    for (index, event) in sampled.iter().enumerate() {
        let start = trace_moment(event.frame, event.ordinal, trace, config).max(86.0);
        let end = sampled
            .get(index + 1)
            .map_or(104.0, |next| trace_moment(next.frame, next.ordinal, trace, config))
            .clamp(start + 0.001, 104.0);
        let Some(address) = event.address else {
            continue;
        };
        let phase = match event.kind.as_str() {
            "read" => PhaseKind::MemoryRead,
            "write" => PhaseKind::MemoryWrite,
            _ => PhaseKind::MemoryRead,
        };
        let active = physical_activity_nodes(phase, Some(address));
        let page = active
            .iter()
            .find(|id| id.starts_with("ramPage"))
            .and_then(|id| topology.node(id));
        let Some(page) = page else {
            continue;
        };
        let b = screen_rect(page.bounds, fit);
        let (k1, k2) = hold_window(start, end, total);
        let _ = writeln!(
            out,
            r##"<g opacity="0" data-held-ram-address="{:04X}" data-held-ram-page="{}"><animate attributeName="opacity" values="0;1;0;0" keyTimes="0;{k1:.6};{k2:.6};1" calcMode="discrete" dur="{total:.3}s" repeatCount="indefinite"/><rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="4" fill="#58d6ff" fill-opacity=".25" stroke="#96edff" stroke-width="2.6" filter="url(#leader-soft-glow)"/><rect x="610" y="384" width="540" height="24" rx="5" fill="#061019" stroke="#2b586e"/><text x="621" y="400" fill="#8ceaff" font-size="9" font-weight="900">{} · {} · A {:04X} · D {:02X} · {}</text></g>"##,
            address,
            xml_escape(&page.id),
            b.x,
            b.y,
            b.w,
            b.h,
            xml_escape(&page.title),
            xml_escape(event.kind.as_str()),
            address,
            event.data.unwrap_or(0),
            xml_escape(event.control),
        );
    }
}

fn render_held_microstate(out: &mut String, trace: &MatchTrace, config: RenderConfig) {
    if trace.micro_cycles.is_empty() || trace.total_frames == 0 {
        return;
    }
    let candidates = trace.micro_cycles.iter().collect::<Vec<_>>();
    let sampled = sample_refs(&candidates, 84);
    let total = config.total();
    for (index, event) in sampled.iter().enumerate() {
        let start = trace_moment(event.frame, event.ordinal, trace, config);
        if start < config.game_start() || start >= config.game_end() {
            continue;
        }
        let end = sampled
            .get(index + 1)
            .map_or(config.game_end(), |next| trace_moment(next.frame, next.ordinal, trace, config))
            .clamp(start + 0.001, config.game_end());
        let (k1, k2) = hold_window(start, end, total);
        let _ = writeln!(
            out,
            r##"<g opacity="0" data-held-micro-pc="{:04X}" data-held-micro-mar="{:04X}"><animate attributeName="opacity" values="0;1;0;0" keyTimes="0;{k1:.6};{k2:.6};1" calcMode="discrete" dur="{total:.3}s" repeatCount="indefinite"/><rect x="137" y="631" width="1026" height="17" rx="3" fill="#071019"/><text x="148" y="643" fill="#9fb2bf" font-size="8" font-weight="900">{} / {}</text><text x="270" y="643" fill="#ffcf83" font-size="8" font-weight="900">PC {:04X}</text><text x="350" y="643" fill="#ffcf83" font-size="8" font-weight="900">MAR {:04X}</text><text x="446" y="643" fill="#78e5ff" font-size="8" font-weight="900">MDR {:02X}</text><text x="526" y="643" fill="#78e5ff" font-size="8" font-weight="900">IR {:02X}</text><text x="602" y="643" fill="#ff91d8" font-size="8" font-weight="900">{}</text></g>"##,
            event.pc,
            event.mar,
            event.phase.as_str(),
            event.kind.as_str(),
            event.pc,
            event.mar,
            event.mdr,
            event.ir,
            xml_escape(event.control),
        );
    }
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

fn hold_window(start: f32, end: f32, total: f32) -> (f32, f32) {
    (norm(start, total), norm(end, total))
}

fn trace_moment(frame: u32, ordinal: u16, trace: &MatchTrace, config: RenderConfig) -> f32 {
    config.game_start()
        + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
        + f32::from(ordinal.min(63)) * 0.0012
}

fn norm(value: f32, total: f32) -> f32 {
    (value / total).clamp(0.0, 1.0)
}

fn overview_target(owner: MemoryOwner, kind: &str) -> (f32, f32, f32, f32, &'static str) {
    if kind == "scanout" {
        return (438.0, 158.0, 104.0, 86.0, "GPU / SCANOUT");
    }
    match owner {
        MemoryOwner::Rom => (174.0, 418.0, 112.0, 82.0, "PROGRAM ROM"),
        MemoryOwner::Ram => (304.0, 412.0, 132.0, 88.0, "WORK RAM"),
        MemoryOwner::Vram => (454.0, 418.0, 88.0, 82.0, "VIDEO RAM"),
        MemoryOwner::Mmio => (44.0, 418.0, 112.0, 82.0, "MMIO / IO"),
        MemoryOwner::Unmapped => (82.0, 528.0, 414.0, 52.0, "SYSTEM BUS"),
    }
}

fn owner_color(owner: MemoryOwner) -> &'static str {
    match owner {
        MemoryOwner::Rom => "#58d6ff",
        MemoryOwner::Ram => "#7ff0c4",
        MemoryOwner::Vram => "#9cff78",
        MemoryOwner::Mmio => "#ff91d8",
        MemoryOwner::Unmapped => "#ffbd66",
    }
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
    fn readable_layer_contains_held_native_state() {
        let topology = build_topology();
        let trace = Machine::run_match("frontpage-held", 5000);
        let source = String::from("<svg></svg>");
        let output = apply(source, &topology, &trace, RenderConfig::default());
        assert!(output.contains("id=\"frontpage-readable-native-state\""));
        assert!(output.contains("data-held-bus-address=\""));
        assert!(output.contains("data-held-ucontrol=\""));
        assert!(output.contains("data-held-alu-result=\""));
        assert!(output.contains("data-held-ram-page=\""));
        assert!(output.contains("data-held-micro-pc=\""));
    }
}
