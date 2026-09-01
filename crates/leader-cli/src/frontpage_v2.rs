use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use leader_core::{
    framebuffer_pixel, memory_fabric_specs, orthogonal_route_for_link, physical_alu_link_values,
    physical_bus_link_values, resolve_physical_memory_byte, total_memory_bit_cells,
    total_memory_bytes, MatchTrace, MemoryOwner, Rect, SignalKind, Topology, FRAMEBUFFER_HEIGHT,
    FRAMEBUFFER_WIDTH,
};
use leader_svg::RenderConfig;

const SVG_W: f32 = 1200.0;
const SVG_H: f32 = 675.0;
const MACHINE_VIEW: Rect = Rect {
    x: 24.0,
    y: 92.0,
    w: 1152.0,
    h: 548.0,
};
const CRT_X: f32 = 934.0;
const CRT_Y: f32 = 115.0;
const CRT_W: f32 = 224.0;
const CRT_H: f32 = 168.0;
const CRT_INSET: f32 = 12.0;
const MAX_BUS_EVENTS: usize = 96;
const MAX_ALU_EVENTS: usize = 54;
const MAX_VRAM_FRAMES: usize = 96;

#[derive(Debug, Clone, Copy)]
struct Fit {
    scale: f32,
    tx: f32,
    ty: f32,
}

#[must_use]
pub fn render(topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    let total = config.total();
    let fit = fit_rect(
        Rect::new(0.0, 0.0, topology.width, topology.height),
        MACHINE_VIEW,
        8.0,
    );
    let mut out = String::with_capacity(5_500_000);

    let _ = writeln!(
        out,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="675" viewBox="0 0 1200 675" role="img" aria-labelledby="title desc" data-frontpage-version="physical-die-v2" data-duration="{total:.3}" data-memory-bytes="{}" data-memory-bit-cells="{}">"##,
        total_memory_bytes(),
        total_memory_bit_cells(),
    );
    out.push_str(
        r##"<title id="title">Leader — physical deterministic CPU running Space Invaders</title>
<desc id="desc">A dense fixed hardware die. Thousands of visible low-level memory cells surround a native CPU datapath while real trace values electrically illuminate exact physical wires, gates, memory addresses and video scanout.</desc>
<defs>
  <linearGradient id="v2-frame" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ffc16b"/><stop offset=".45" stop-color="#ff785b"/><stop offset="1" stop-color="#ff4253"/></linearGradient>
  <radialGradient id="v2-bg" cx="50%" cy="42%" r="78%"><stop offset="0" stop-color="#091725"/><stop offset="1" stop-color="#04080d"/></radialGradient>
  <radialGradient id="v2-crt" cx="50%" cy="45%" r="70%"><stop offset="0" stop-color="#07130b"/><stop offset="1" stop-color="#010302"/></radialGradient>
  <filter id="v2-glow" x="-120%" y="-120%" width="340%" height="340%"><feGaussianBlur stdDeviation="3.2" result="b"/><feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter>
  <filter id="v2-hot" x="-150%" y="-150%" width="400%" height="400%"><feGaussianBlur stdDeviation="6" result="b"/><feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter>
  <clipPath id="v2-machine-clip"><rect x="24" y="92" width="1152" height="548" rx="9"/></clipPath>
  <clipPath id="v2-crt-clip"><rect x="946" y="127" width="200" height="144" rx="8"/></clipPath>
</defs>
<style>
text{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}
.v2-group{fill:#06101a;fill-opacity:.12;stroke:#32485b;stroke-width:7;stroke-dasharray:30 22;vector-effect:non-scaling-stroke}
.v2-group-label{fill:#647b8e;font-weight:900;letter-spacing:8px}
.v2-node{fill:#09141e;stroke:#456175;stroke-width:2.4;vector-effect:non-scaling-stroke}
.v2-node-head{fill:#0d1d29}
.v2-wire{fill:none;stroke-width:1.15;stroke-linecap:round;stroke-linejoin:round;opacity:.16;vector-effect:non-scaling-stroke}
.v2-address{stroke:#ffbe64}.v2-data{stroke:#55d9ff}.v2-control{stroke:#ff72c8}.v2-clock{stroke:#69efc1}.v2-carry{stroke:#ffe36d}.v2-video{stroke:#9cff79}
.v2-active-address{stroke:#ffd08b}.v2-active-data{stroke:#7ae6ff}.v2-active-control{stroke:#ff9add}.v2-active-clock{stroke:#9affdb}.v2-active-carry{stroke:#fff08a}.v2-active-video{stroke:#baff98}
.v2-active-wire{fill:none;stroke-width:5;stroke-linecap:round;stroke-linejoin:round;filter:url(#v2-glow);vector-effect:non-scaling-stroke}
.v2-byte-rom{fill:#9e83ff}.v2-byte-ram{fill:#54d8ff}.v2-byte-vram{fill:#9dff76}
.v2-crt-pixel{fill:#b9ff78}
</style>
"##,
    );

    render_chrome(&mut out, topology, trace);
    render_wire_definitions(&mut out, topology);
    let _ = writeln!(
        out,
        r##"<g id="v2-machine" clip-path="url(#v2-machine-clip)" transform="translate({:.5} {:.5}) scale({:.7})">"##,
        fit.tx, fit.ty, fit.scale,
    );
    render_groups(&mut out, topology, fit.scale);
    render_static_wires(&mut out, topology);
    render_nodes(&mut out, topology, fit.scale);
    render_memory_fabric(&mut out, topology);
    render_native_bus_propagation(&mut out, topology, trace, config);
    render_native_alu_propagation(&mut out, topology, trace, config);
    render_memory_cell_activity(&mut out, topology, trace, config);
    out.push_str("</g>\n");

    render_crt(&mut out, trace, config);
    render_probe_strip(&mut out, trace, config);
    render_contract_metadata(&mut out, topology, trace);
    out.push_str("</svg>\n");
    out
}

fn render_chrome(out: &mut String, topology: &Topology, trace: &MatchTrace) {
    let _ = writeln!(
        out,
        r##"<rect x="4" y="4" width="1192" height="667" rx="12" fill="url(#v2-bg)" stroke="url(#v2-frame)" stroke-width="6"/><rect x="14" y="14" width="1172" height="647" rx="8" fill="none" stroke="#1c2d3b"/>
<g font-family="Inter,Arial,sans-serif" font-style="italic" font-weight="900"><text x="28" y="53" fill="#ff9f74" font-size="37">LEADER</text><text x="30" y="75" fill="#65798a" font-size="9" font-style="normal" letter-spacing="2.4">PHYSICAL TRACE COMPUTER · RUST · NO CAMERA · NO PARTICLES</text></g>
<text x="1170" y="41" text-anchor="end" fill="#8ca0ae" font-size="9" font-weight="900">{} LOGIC NODES · {} PHYSICAL LINKS</text>
<text x="1170" y="58" text-anchor="end" fill="#55d9ff" font-size="10" font-weight="900">34,816 ADDRESSABLE BYTES</text>
<text x="1170" y="75" text-anchor="end" fill="#9dff76" font-size="10" font-weight="900">278,528 MEMORY BIT CELLS</text>
<text x="28" y="654" fill="#546b7c" font-size="8" font-weight="900">TRACE {:016X}</text>"##,
        topology.nodes.len(),
        topology.links.len(),
        trace.seed_hash,
    );
}

fn render_wire_definitions(out: &mut String, topology: &Topology) {
    out.push_str("<defs id=\"v2-wire-definitions\">\n");
    for link in &topology.links {
        let Some(route) = orthogonal_route_for_link(topology, link) else {
            continue;
        };
        let _ = writeln!(
            out,
            "<path id=\"v2-wire-{}\" d=\"{}\"/>",
            xml_escape(&link.id),
            route_path(route),
        );
    }
    out.push_str("</defs>\n");
}

fn render_groups(out: &mut String, topology: &Topology, scale: f32) {
    let label_size = (23.0 / scale).clamp(28.0, 120.0);
    out.push_str("<g id=\"v2-groups\">\n");
    for group in &topology.groups {
        let b = group.bounds;
        let _ = writeln!(
            out,
            r##"<g data-group="{}"><rect class="v2-group" x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="18"/><text class="v2-group-label" x="{:.1}" y="{:.1}" font-size="{label_size:.1}">{}</text></g>"##,
            xml_escape(&group.id), b.x, b.y, b.w, b.h, b.x + 20.0, b.y + 42.0,
            xml_escape(&group.label),
        );
    }
    out.push_str("</g>\n");
}

fn render_static_wires(out: &mut String, topology: &Topology) {
    out.push_str("<g id=\"v2-static-wires\">\n");
    for link in &topology.links {
        let _ = writeln!(
            out,
            r##"<use href="#v2-wire-{}" class="v2-wire {}"/>"##,
            xml_escape(&link.id), signal_class(link.signal, false),
        );
    }
    out.push_str("</g>\n");
}

fn render_nodes(out: &mut String, topology: &Topology, scale: f32) {
    let title_size = (8.5 / scale).clamp(11.0, 42.0);
    out.push_str("<g id=\"v2-logic-nodes\">\n");
    for node in &topology.nodes {
        let b = node.bounds;
        let show_text = b.w * scale >= 11.0 && b.h * scale >= 7.0;
        let _ = write!(
            out,
            r##"<g id="v2-node-{}" data-node-kind="{}"><rect class="v2-node" x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="5"/>"##,
            xml_escape(&node.id), xml_escape(&node.kind), b.x, b.y, b.w, b.h,
        );
        if show_text {
            let _ = write!(
                out,
                r##"<text x="{:.1}" y="{:.1}" fill="#9db1bf" font-size="{title_size:.1}" font-weight="800">{}</text>"##,
                b.x + 6.0,
                b.y + title_size + 4.0,
                xml_escape(&node.title),
            );
        }
        out.push_str("</g>\n");
    }
    out.push_str("</g>\n");
}

fn render_memory_fabric(out: &mut String, topology: &Topology) {
    out.push_str("<g id=\"v2-memory-byte-fabric\" aria-label=\"34816 addressable byte cells\">\n");
    for spec in memory_fabric_specs() {
        let class = match spec.owner {
            MemoryOwner::Rom => "v2-byte-rom",
            MemoryOwner::Ram => "v2-byte-ram",
            MemoryOwner::Vram => "v2-byte-vram",
            MemoryOwner::Mmio | MemoryOwner::Unmapped => continue,
        };
        for page in 0..spec.page_count {
            let node_id = format!("{}{page}", spec.page_prefix);
            let Some(node) = topology.node(&node_id) else {
                continue;
            };
            let path = byte_matrix_path(node.bounds);
            let _ = writeln!(
                out,
                r##"<path class="{class}" opacity=".30" data-memory-page="{node_id}" data-byte-cells="256" d="{path}"/>"##,
            );
        }
    }
    out.push_str("</g>\n");
}

fn render_native_bus_propagation(
    out: &mut String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
) {
    if trace.bus_transactions.is_empty() || trace.total_frames == 0 {
        return;
    }
    let candidates = trace
        .bus_transactions
        .iter()
        .filter(|event| event.address.is_some())
        .collect::<Vec<_>>();
    let sampled = sample_refs(&candidates, MAX_BUS_EVENTS);
    let total = config.total();
    out.push_str("<g id=\"v2-native-bus-propagation\">\n");

    for event in sampled {
        let moment = trace_moment(event.frame, event.ordinal, trace, config);
        let values = physical_bus_link_values(topology, *event);
        if values.is_empty() {
            continue;
        }
        for value in values {
            let start = moment + f32::from(value.rank) * 0.032;
            let end = start + 0.18;
            let k1 = norm(start, total);
            let k2 = norm(start + 0.018, total).max(k1 + 0.000_01);
            let k3 = norm(end, total).max(k2 + 0.000_01);
            let _ = writeln!(
                out,
                r##"<use href="#v2-wire-{}" class="v2-active-wire {}" opacity="0" data-rank="{}" data-stage="{}" data-value="{}" data-width="{}"><animate attributeName="opacity" values="0;0;1;0;0" keyTimes="0;{k1:.6};{k2:.6};{k3:.6};1" dur="{total:.3}s" repeatCount="indefinite"/></use>"##,
                xml_escape(&value.link_id),
                signal_class(value.signal, true),
                value.rank,
                xml_escape(value.stage),
                value.value,
                value.width,
            );
        }
    }
    out.push_str("</g>\n");
}

fn render_native_alu_propagation(
    out: &mut String,
    _topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
) {
    if trace.alu_events.is_empty() || trace.total_frames == 0 {
        return;
    }
    let sampled = sample_slice(&trace.alu_events, MAX_ALU_EVENTS);
    let total = config.total();
    out.push_str("<g id=\"v2-native-alu-propagation\">\n");
    for event in sampled {
        let moment = trace_moment(event.frame, event.ordinal, trace, config);
        for value in physical_alu_link_values(event.trace)
            .into_iter()
            .filter(|value| value.selected && value.value)
        {
            let start = moment + f32::from(value.rank) * 0.025;
            let end = start + 0.16;
            let k1 = norm(start, total);
            let k2 = norm(start + 0.018, total).max(k1 + 0.000_01);
            let k3 = norm(end, total).max(k2 + 0.000_01);
            let _ = writeln!(
                out,
                r##"<use href="#v2-wire-{}" class="v2-active-wire v2-active-carry" opacity="0" data-alu-bit="{}" data-rank="{}" data-stage="{}"><animate attributeName="opacity" values="0;0;1;0;0" keyTimes="0;{k1:.6};{k2:.6};{k3:.6};1" dur="{total:.3}s" repeatCount="indefinite"/></use>"##,
                xml_escape(&value.link_id), value.bit, value.rank, xml_escape(value.stage),
            );
        }
    }
    out.push_str("</g>\n");
}

fn render_memory_cell_activity(
    out: &mut String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
) {
    if trace.bus_transactions.is_empty() || trace.total_frames == 0 {
        return;
    }
    let candidates = trace
        .bus_transactions
        .iter()
        .filter(|event| event.address.is_some() && event.data.is_some())
        .collect::<Vec<_>>();
    let sampled = sample_refs(&candidates, MAX_BUS_EVENTS);
    let total = config.total();
    out.push_str("<g id=\"v2-exact-memory-cell-activity\">\n");
    for event in sampled {
        let Some(address) = event.address else {
            continue;
        };
        let Some(byte) = resolve_physical_memory_byte(address, event.data.unwrap_or(0)) else {
            continue;
        };
        let page_id = match byte.address.owner {
            MemoryOwner::Rom => format!("romPage{}", byte.address.page),
            MemoryOwner::Ram => format!("ramPage{}", byte.address.page),
            MemoryOwner::Vram => format!("vramPage{}", byte.address.page),
            MemoryOwner::Mmio | MemoryOwner::Unmapped => continue,
        };
        let Some(page) = topology.node(&page_id) else {
            continue;
        };
        let cell = byte_cell_rect(page.bounds, byte.address.row, byte.address.column);
        let moment = trace_moment(event.frame, event.ordinal, trace, config) + 0.15;
        let k1 = norm(moment, total);
        let k2 = norm(moment + 0.018, total).max(k1 + 0.000_01);
        let k3 = norm(moment + 0.34, total).max(k2 + 0.000_01);
        let bit_string = bits_string(byte.bits_lsb_first);
        let _ = writeln!(
            out,
            r##"<g opacity="0" data-memory-owner="{}" data-memory-address="{address:04X}" data-memory-page="{}" data-memory-byte="{}" data-memory-bits="{bit_string}"><animate attributeName="opacity" values="0;0;1;0;0" keyTimes="0;{k1:.6};{k2:.6};{k3:.6};1" dur="{total:.3}s" repeatCount="indefinite"/><rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="#ffffff" stroke="#ffffff" stroke-width="2.2" vector-effect="non-scaling-stroke" filter="url(#v2-hot)"/><rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="8" fill="none" stroke="#ffffff" stroke-width="3" vector-effect="non-scaling-stroke" opacity=".42"/></g>"##,
            owner_name(byte.address.owner),
            byte.address.page,
            byte.address.byte,
            cell.x,
            cell.y,
            cell.w,
            cell.h,
            page.bounds.x,
            page.bounds.y,
            page.bounds.w,
            page.bounds.h,
        );
    }
    out.push_str("</g>\n");
}

fn render_crt(out: &mut String, trace: &MatchTrace, config: RenderConfig) {
    let inner_x = CRT_X + CRT_INSET;
    let inner_y = CRT_Y + CRT_INSET;
    let inner_w = CRT_W - CRT_INSET * 2.0;
    let inner_h = CRT_H - CRT_INSET * 2.0;
    let total = config.total();
    let _ = writeln!(
        out,
        r##"<g id="v2-crt"><rect x="{CRT_X}" y="{CRT_Y}" width="{CRT_W}" height="{CRT_H}" rx="16" fill="#071019" stroke="#526a78" stroke-width="3"/><rect x="{inner_x}" y="{inner_y}" width="{inner_w}" height="{inner_h}" rx="8" fill="url(#v2-crt)" stroke="#355445"/><text x="{:.1}" y="{:.1}" fill="#789082" font-size="8" font-weight="900">1-BIT CRT · 128×96 · NATIVE VRAM</text>"##,
        CRT_X + 12.0,
        CRT_Y - 7.0,
    );

    if !trace.vram_checkpoints.is_empty() && trace.total_frames > 0 {
        let frames = sample_slice(&trace.vram_checkpoints, MAX_VRAM_FRAMES);
        let sx = inner_w / FRAMEBUFFER_WIDTH as f32;
        let sy = inner_h / FRAMEBUFFER_HEIGHT as f32;
        let _ = writeln!(
            out,
            r##"<g clip-path="url(#v2-crt-clip)" transform="translate({inner_x:.3} {inner_y:.3}) scale({sx:.7} {sy:.7})">"##,
        );
        for (index, checkpoint) in frames.iter().enumerate() {
            let start = trace_frame_time(checkpoint.frame, trace, config);
            let end = frames
                .get(index + 1)
                .map_or(config.game_end(), |next| trace_frame_time(next.frame, trace, config))
                .max(start + 0.001);
            let k1 = norm(start, total);
            let k2 = norm(end, total).max(k1 + 0.000_01);
            let path = framebuffer_path(&checkpoint.bytes);
            let pixels = framebuffer_population(&checkpoint.bytes);
            let _ = writeln!(
                out,
                r##"<path class="v2-crt-pixel" d="{path}" opacity="0" data-vram-frame="{}" data-vram-checksum="{:08X}" data-vram-pixels="{pixels}"><animate attributeName="opacity" values="0;1;0;0" keyTimes="0;{k1:.6};{k2:.6};1" calcMode="discrete" dur="{total:.3}s" repeatCount="indefinite"/></path>"##,
                checkpoint.frame, checkpoint.checksum,
            );
        }
        out.push_str("</g>\n");
    }
    let _ = writeln!(
        out,
        r##"<rect x="{inner_x}" y="{inner_y}" width="{inner_w}" height="2" fill="#d7ffbc" opacity=".08"><animate attributeName="y" values="{inner_y};{:.1};{inner_y}" dur="2.3s" repeatCount="indefinite"/></rect></g>"##,
        inner_y + inner_h - 2.0,
    );
}

fn render_probe_strip(out: &mut String, trace: &MatchTrace, config: RenderConfig) {
    if trace.micro_cycles.is_empty() || trace.total_frames == 0 {
        return;
    }
    let sampled = sample_slice(&trace.micro_cycles, 72);
    let total = config.total();
    let _ = writeln!(out, r##"<g id="v2-logic-probe"><rect x="205" y="645" width="790" height="20" rx="5" fill="#061019" stroke="#243b4b"/></g>"##);
    for (index, event) in sampled.iter().enumerate() {
        let start = trace_moment(event.frame, event.ordinal, trace, config);
        let end = sampled
            .get(index + 1)
            .map_or(config.game_end(), |next| trace_moment(next.frame, next.ordinal, trace, config))
            .max(start + 0.001);
        let k1 = norm(start, total);
        let k2 = norm(end, total).max(k1 + 0.000_01);
        let _ = writeln!(
            out,
            r##"<g opacity="0"><animate attributeName="opacity" values="0;1;0;0" keyTimes="0;{k1:.6};{k2:.6};1" calcMode="discrete" dur="{total:.3}s" repeatCount="indefinite"/><text x="220" y="659" fill="#8299aa" font-size="8" font-weight="900">µ{} · {}</text><text x="405" y="659" fill="#ffd07e" font-size="8" font-weight="900">PC {:04X} · MAR {:04X}</text><text x="610" y="659" fill="#72e4ff" font-size="8" font-weight="900">MDR {:02X} · IR {:02X}</text><text x="785" y="659" fill="#ff8bd6" font-size="8" font-weight="900">{}</text></g>"##,
            xml_escape(event.phase.as_str()),
            xml_escape(event.kind.as_str()),
            event.pc,
            event.mar,
            event.mdr,
            event.ir,
            xml_escape(event.control),
        );
    }
}

fn render_contract_metadata(out: &mut String, topology: &Topology, trace: &MatchTrace) {
    let _ = writeln!(
        out,
        r##"<g id="v2-contract" display="none" data-node-count="{}" data-link-count="{}" data-memory-bytes="{}" data-memory-bit-cells="{}" data-bus-events="{}" data-alu-events="{}" data-vram-frames="{}"/>"##,
        topology.nodes.len(),
        topology.links.len(),
        total_memory_bytes(),
        total_memory_bit_cells(),
        trace.bus_transactions.len(),
        trace.alu_events.len(),
        trace.vram_checkpoints.len(),
    );
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

fn route_path(route: [[f32; 2]; 4]) -> String {
    format!(
        "M{:.1} {:.1}H{:.1}V{:.1}H{:.1}",
        route[0][0], route[0][1], route[1][0], route[2][1], route[3][0],
    )
}

fn signal_class(signal: SignalKind, active: bool) -> &'static str {
    match (signal, active) {
        (SignalKind::Address, false) => "v2-address",
        (SignalKind::Data, false) => "v2-data",
        (SignalKind::Control, false) => "v2-control",
        (SignalKind::Clock, false) => "v2-clock",
        (SignalKind::Carry, false) => "v2-carry",
        (SignalKind::Video, false) => "v2-video",
        (SignalKind::Address, true) => "v2-active-address",
        (SignalKind::Data, true) => "v2-active-data",
        (SignalKind::Control, true) => "v2-active-control",
        (SignalKind::Clock, true) => "v2-active-clock",
        (SignalKind::Carry, true) => "v2-active-carry",
        (SignalKind::Video, true) => "v2-active-video",
    }
}

fn byte_matrix_path(bounds: Rect) -> String {
    let pad_x = bounds.w * 0.055;
    let pad_y = bounds.h * 0.17;
    let usable_w = (bounds.w - pad_x * 2.0).max(1.0);
    let usable_h = (bounds.h - pad_y - bounds.h * 0.055).max(1.0);
    let cell_w = usable_w / 16.0;
    let cell_h = usable_h / 16.0;
    let dot_w = (cell_w * 0.55).max(0.7);
    let dot_h = (cell_h * 0.52).max(0.7);
    let mut path = String::with_capacity(14_000);
    for row in 0..16 {
        for column in 0..16 {
            let x = bounds.x + pad_x + column as f32 * cell_w + (cell_w - dot_w) * 0.5;
            let y = bounds.y + pad_y + row as f32 * cell_h + (cell_h - dot_h) * 0.5;
            let _ = write!(path, "M{x:.2} {y:.2}h{dot_w:.2}v{dot_h:.2}h-{dot_w:.2}z");
        }
    }
    path
}

fn byte_cell_rect(bounds: Rect, row: usize, column: usize) -> Rect {
    let pad_x = bounds.w * 0.055;
    let pad_y = bounds.h * 0.17;
    let usable_w = (bounds.w - pad_x * 2.0).max(1.0);
    let usable_h = (bounds.h - pad_y - bounds.h * 0.055).max(1.0);
    let cell_w = usable_w / 16.0;
    let cell_h = usable_h / 16.0;
    Rect::new(
        bounds.x + pad_x + column as f32 * cell_w,
        bounds.y + pad_y + row as f32 * cell_h,
        cell_w,
        cell_h,
    )
}

fn framebuffer_path(bytes: &[u8]) -> String {
    let mut path = String::with_capacity(30_000);
    for y in 0..FRAMEBUFFER_HEIGHT {
        let mut x = 0;
        while x < FRAMEBUFFER_WIDTH {
            if framebuffer_pixel(bytes, x, y) != Some(true) {
                x += 1;
                continue;
            }
            let start = x;
            x += 1;
            while x < FRAMEBUFFER_WIDTH && framebuffer_pixel(bytes, x, y) == Some(true) {
                x += 1;
            }
            let width = x - start;
            let _ = write!(path, "M{start} {y}h{width}v1h-{width}z");
        }
    }
    path
}

fn framebuffer_population(bytes: &[u8]) -> usize {
    bytes.iter().map(|byte| byte.count_ones() as usize).sum()
}

fn bits_string(bits: [bool; 8]) -> String {
    bits.iter()
        .rev()
        .map(|value| if *value { '1' } else { '0' })
        .collect()
}

fn owner_name(owner: MemoryOwner) -> &'static str {
    match owner {
        MemoryOwner::Rom => "rom",
        MemoryOwner::Ram => "ram",
        MemoryOwner::Vram => "vram",
        MemoryOwner::Mmio => "mmio",
        MemoryOwner::Unmapped => "unmapped",
    }
}

fn trace_moment(frame: u32, ordinal: u16, trace: &MatchTrace, config: RenderConfig) -> f32 {
    config.game_start()
        + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
        + f32::from(ordinal.min(63)) * 0.0015
}

fn trace_frame_time(frame: u32, trace: &MatchTrace, config: RenderConfig) -> f32 {
    config.game_start() + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
}

fn norm(value: f32, total: f32) -> f32 {
    (value / total).clamp(0.0, 1.0)
}

fn sample_refs<'a, T>(values: &'a [&'a T], maximum: usize) -> Vec<&'a T> {
    if values.len() <= maximum {
        return values.to_vec();
    }
    let stride = values.len().div_ceil(maximum);
    values.iter().step_by(stride).copied().collect()
}

fn sample_slice<T>(values: &[T], maximum: usize) -> Vec<&T> {
    if values.len() <= maximum {
        return values.iter().collect();
    }
    let stride = values.len().div_ceil(maximum);
    let mut sampled = values.iter().step_by(stride).collect::<Vec<_>>();
    if sampled.last().map(|value| *value as *const T) != values.last().map(|value| value as *const T) {
        if let Some(last) = values.last() {
            sampled.push(last);
        }
    }
    sampled
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
    fn physical_die_contains_no_camera_or_particles() {
        let topology = build_topology();
        let trace = Machine::run_match("v2-no-particles", 5000);
        let svg = render(&topology, &trace, RenderConfig::default());
        assert!(svg.contains("data-frontpage-version=\"physical-die-v2\""));
        assert!(svg.contains("id=\"v2-memory-byte-fabric\""));
        assert!(svg.contains("id=\"v2-native-bus-propagation\""));
        assert!(svg.contains("id=\"v2-native-alu-propagation\""));
        assert!(!svg.contains("animateMotion"));
        assert!(!svg.contains("leaderCamera"));
        assert!(!svg.contains("attributeName=\"viewBox\""));
    }

    #[test]
    fn physical_die_declares_real_memory_density() {
        let topology = build_topology();
        let trace = Machine::run_match("v2-memory-density", 5000);
        let svg = render(&topology, &trace, RenderConfig::default());
        assert!(svg.contains("data-memory-bytes=\"34816\""));
        assert!(svg.contains("data-memory-bit-cells=\"278528\""));
        assert!(svg.contains("data-byte-cells=\"256\""));
        assert!(svg.contains("data-memory-bits=\""));
    }

    #[test]
    fn crt_frames_are_exclusive_and_preserve_pixel_population() {
        let topology = build_topology();
        let trace = Machine::run_match("v2-crt", 5000);
        let svg = render(&topology, &trace, RenderConfig::default());
        assert!(svg.contains("data-vram-pixels=\""));
        assert!(svg.contains("values=\"0;1;0;0\""));
        assert!(!svg.contains("values=\"0;1;1;0\""));
    }
}
