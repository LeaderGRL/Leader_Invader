use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use leader_core::{
    build_navigation, framebuffer_pixel, memory_owner, physical_activity_nodes,
    physical_alu_node_values, MatchTrace, MemoryOwner, NavigationModel, PhaseKind, Rect,
    SignalKind, Topology, FRAMEBUFFER_HEIGHT, FRAMEBUFFER_WIDTH,
};
use leader_svg::RenderConfig;

const WIDTH: f32 = 1200.0;
const HEIGHT: f32 = 675.0;
const OVERVIEW_X: f32 = 24.0;
const OVERVIEW_Y: f32 = 116.0;
const OVERVIEW_W: f32 = 540.0;
const OVERVIEW_H: f32 = 500.0;
const DETAIL_X: f32 = 584.0;
const DETAIL_Y: f32 = 116.0;
const DETAIL_W: f32 = 592.0;
const DETAIL_H: f32 = 306.0;
const VIDEO_X: f32 = 584.0;
const VIDEO_Y: f32 = 438.0;
const VIDEO_W: f32 = 592.0;
const VIDEO_H: f32 = 178.0;

#[derive(Debug, Clone, Copy)]
struct Block {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Block {
    fn center(self) -> (f32, f32) {
        (self.x + self.w * 0.5, self.y + self.h * 0.5)
    }
}

#[derive(Debug, Clone, Copy)]
struct DetailScene {
    module: &'static str,
    title: &'static str,
    subtitle: &'static str,
    start: f32,
    end: f32,
}

const DETAIL_SCENES: [DetailScene; 8] = [
    DetailScene {
        module: "pc.fetch",
        title: "PROGRAM COUNTER / FETCH",
        subtitle: "16-BIT RIPPLE ADDRESS PATH",
        start: 0.05,
        end: 13.0,
    },
    DetailScene {
        module: "regs.readwrite",
        title: "REGISTER FILE",
        subtitle: "BIT-LEVEL READ / WRITEBACK",
        start: 13.0,
        end: 29.0,
    },
    DetailScene {
        module: "romsys.pages",
        title: "PROGRAM ROM",
        subtitle: "8 KiB / 32 PHYSICAL PAGES",
        start: 29.0,
        end: 46.0,
    },
    DetailScene {
        module: "decode.microcode",
        title: "MICROCODE / CONTROL ROM",
        subtitle: "256 × 24 NATIVE CONTROL WORD",
        start: 46.0,
        end: 68.0,
    },
    DetailScene {
        module: "alu.ripple",
        title: "8-BIT RIPPLE ALU",
        subtitle: "LIVE GATE VALUES / CARRY PROPAGATION",
        start: 68.0,
        end: 86.0,
    },
    DetailScene {
        module: "ramsys.pages",
        title: "WORK RAM",
        subtitle: "24 KiB / EXACT ADDRESSED PAGE",
        start: 86.0,
        end: 104.0,
    },
    DetailScene {
        module: "bus.stack",
        title: "SYSTEM BUS / STACK",
        subtitle: "ADDRESS + DATA + CALL / RET PATH",
        start: 104.0,
        end: 118.0,
    },
    DetailScene {
        module: "gpu.scanout",
        title: "VIDEO DMA / SCANOUT",
        subtitle: "VRAM → SHIFT → PIXEL → CRT",
        start: 118.0,
        end: 137.9,
    },
];

#[must_use]
pub fn render(topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    let total = config.total();
    let navigation = build_navigation(topology);
    let mut out = String::with_capacity(2_400_000);

    let _ = writeln!(
        out,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="675" viewBox="0 0 1200 675" role="img" aria-labelledby="title desc" data-frontpage-version="observatory-v1" data-duration="{total:.3}">"##
    );
    out.push_str(
        r##"<title id="title">Leader — deterministic hardware observatory</title>
<desc id="desc">A fixed GitHub-safe hardware observatory. Native CPU, bus, RAM, ALU and video activity remain synchronized while isolated detail views avoid camera overlap.</desc>
<defs>
  <linearGradient id="leader-frame" x1="0" y1="0" x2="1" y2="1">
    <stop offset="0" stop-color="#ffba62"/>
    <stop offset=".45" stop-color="#ff7b58"/>
    <stop offset="1" stop-color="#ff4656"/>
  </linearGradient>
  <linearGradient id="leader-panel" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="#0b1420"/>
    <stop offset="1" stop-color="#071019"/>
  </linearGradient>
  <radialGradient id="leader-crt" cx="50%" cy="45%" r="70%">
    <stop offset="0" stop-color="#07150d"/>
    <stop offset="1" stop-color="#010403"/>
  </radialGradient>
  <filter id="leader-glow" x="-80%" y="-80%" width="260%" height="260%">
    <feGaussianBlur stdDeviation="5" result="blur"/>
    <feMerge><feMergeNode in="blur"/><feMergeNode in="SourceGraphic"/></feMerge>
  </filter>
  <filter id="leader-soft-glow" x="-80%" y="-80%" width="260%" height="260%">
    <feGaussianBlur stdDeviation="2.5" result="blur"/>
    <feMerge><feMergeNode in="blur"/><feMergeNode in="SourceGraphic"/></feMerge>
  </filter>
  <pattern id="leader-grid" width="24" height="24" patternUnits="userSpaceOnUse">
    <path d="M24 0H0V24" fill="none" stroke="#183047" stroke-width="1" opacity=".26"/>
  </pattern>
  <clipPath id="clip-overview"><rect x="30" y="148" width="528" height="454" rx="10"/></clipPath>
  <clipPath id="clip-detail"><rect x="596" y="162" width="568" height="248" rx="10"/></clipPath>
  <clipPath id="clip-crt"><rect x="606" y="474" width="244" height="126" rx="11"/></clipPath>
</defs>
<style>
text{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}
.panel{fill:url(#leader-panel);stroke:#243b50;stroke-width:1.5}
.panel-kicker{fill:#67839a;font-size:10px;font-weight:800;letter-spacing:2px}
.panel-title{fill:#d5e3ed;font-size:16px;font-weight:900;letter-spacing:1px}
.panel-meta{fill:#587184;font-size:9px;font-weight:700;letter-spacing:1px}
.module-shell{fill:#0a1621;stroke:#476278;stroke-width:1.5}
.module-title{fill:#c7d7e2;font-size:8px;font-weight:900;letter-spacing:.5px}
.module-meta{fill:#5f788c;font-size:6px;font-weight:700}
.backbone{fill:none;stroke:#314b60;stroke-width:1.5;opacity:.42}
.native-address{stroke:#ffbd66;fill:#ffbd66}
.native-data{stroke:#58d6ff;fill:#58d6ff}
.native-control{stroke:#ff79c9;fill:#ff79c9}
.native-video{stroke:#9cff78;fill:#9cff78}
.native-clock{stroke:#7ff0c4;fill:#7ff0c4}
.detail-node{fill:#0b1824;stroke:#59778e;stroke-width:1.2;vector-effect:non-scaling-stroke}
.detail-wire{fill:none;stroke-width:1.2;opacity:.44;vector-effect:non-scaling-stroke}
.detail-label{fill:#c7d8e4;font-weight:800}
.telemetry-label{fill:#5f798d;font-size:9px;font-weight:800;letter-spacing:1px}
.telemetry-value{fill:#d8e7ef;font-size:11px;font-weight:900}
.crt-pixel{fill:#b8ff72}
</style>
"##,
    );

    render_chrome(&mut out, topology, trace);
    render_overview(&mut out, topology, trace, config);
    render_detail_panel(&mut out, topology, &navigation, trace, config);
    render_video_panel(&mut out, trace, config);
    render_bottom_telemetry(&mut out, trace, config);
    render_contract_metadata(&mut out, topology, &navigation, trace);

    out.push_str("</svg>\n");
    out
}

fn render_chrome(out: &mut String, topology: &Topology, trace: &MatchTrace) {
    let status = if trace.finished { "GAME CLEAR" } else { "TRACE LIMIT" };
    let _ = writeln!(
        out,
        r##"<rect x="5" y="5" width="1190" height="665" rx="11" fill="#070b11" stroke="url(#leader-frame)" stroke-width="6"/>
<rect x="17" y="17" width="1166" height="646" rx="8" fill="none" stroke="#1e2d3a"/>
<g font-family="Inter,Arial,sans-serif" font-style="italic" font-weight="900" text-anchor="middle">
  <text x="605" y="63" fill="#ff9e72" font-size="49">LEADER</text>
  <text x="596" y="67" fill="#ff4e58" opacity=".72" font-size="49">LEADER</text>
  <text x="600" y="70" fill="#e7df6d" opacity=".34" font-size="49">LEADER</text>
  <rect x="485" y="28" width="230" height="51" fill="#070b11" opacity=".70"/>
  <text x="600" y="62" fill="#ff9e72" font-size="42">LEADER</text>
</g>
<text x="600" y="88" text-anchor="middle" fill="#ff9e72" font-size="12" font-weight="900" letter-spacing="3">DETERMINISTIC CPU · NATIVE TRACE · GITHUB SVG</text>
<text x="26" y="104" fill="#52697c" font-size="8" font-weight="800">TRACE {:016x}</text>
<text x="1174" y="104" text-anchor="end" fill="#52697c" font-size="8" font-weight="800">{} · {} NODES · {} SIGNALS</text>"##,
        trace.seed_hash,
        status,
        topology.nodes.len(),
        topology.links.len(),
    );
}

fn render_overview(out: &mut String, topology: &Topology, trace: &MatchTrace, config: RenderConfig) {
    let total = config.total();
    let _ = writeln!(
        out,
        r##"<g id="frontpage-overview"><rect class="panel" x="{OVERVIEW_X}" y="{OVERVIEW_Y}" width="{OVERVIEW_W}" height="{OVERVIEW_H}" rx="13"/><text class="panel-kicker" x="42" y="139">LIVE SIGNAL FIELD</text><text class="panel-meta" x="548" y="139" text-anchor="end">FIXED STAGE · NO CAMERA</text><rect x="30" y="148" width="528" height="454" rx="10" fill="url(#leader-grid)"/></g>"##
    );

    let blocks = overview_blocks();
    render_backbone(out, topology, &blocks);

    let mut groups = topology.groups.iter().collect::<Vec<_>>();
    groups.sort_by_key(|group| group.assembly_rank);
    let span = config.assembly_seconds / groups.len().max(1) as f32;
    for (index, group) in groups.iter().enumerate() {
        let Some(block) = blocks.get(group.id.as_str()).copied() else {
            continue;
        };
        let appear = index as f32 * span + 0.25;
        let settled = (appear + 0.42).min(config.assembly_seconds - 0.01);
        let k1 = norm(appear, total);
        let k2 = norm(settled, total);
        let _ = writeln!(
            out,
            r##"<g data-subsystem="{}" opacity="0"><animate attributeName="opacity" values="0;0;1;1" keyTimes="0;{k1:.6};{k2:.6};1" dur="{total:.3}s" repeatCount="indefinite"/><rect class="module-shell" x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="7"/><path d="M{:.1} {:.1}H{:.1}" stroke="#273d50"/><text class="module-title" x="{:.1}" y="{:.1}">{}</text><text class="module-meta" x="{:.1}" y="{:.1}">{}</text></g>"##,
            xml_escape(&group.id),
            block.x,
            block.y,
            block.w,
            block.h,
            block.x,
            block.y + 18.0,
            block.x + block.w,
            block.x + 7.0,
            block.y + 13.0,
            xml_escape(short_group_label(&group.id)),
            block.x + 7.0,
            block.y + block.h - 7.0,
            xml_escape(group_capacity_label(&group.id)),
        );
    }

    render_bus_pulses(out, trace, &blocks, config);
}

fn render_backbone(out: &mut String, topology: &Topology, blocks: &HashMap<&'static str, Block>) {
    let mut seen = HashSet::new();
    out.push_str("<g id=\"frontpage-backbone\" clip-path=\"url(#clip-overview)\">\n");
    for link in &topology.links {
        let Some(from) = topology.node(&link.from) else {
            continue;
        };
        let Some(to) = topology.node(&link.to) else {
            continue;
        };
        if from.group == to.group {
            continue;
        }
        let Some(a) = blocks.get(from.group.as_str()).copied() else {
            continue;
        };
        let Some(b) = blocks.get(to.group.as_str()).copied() else {
            continue;
        };
        let key = if from.group <= to.group {
            format!("{}:{}", from.group, to.group)
        } else {
            format!("{}:{}", to.group, from.group)
        };
        if !seen.insert(key) {
            continue;
        }
        let (ax, ay) = a.center();
        let (bx, by) = b.center();
        let mid = (ax + bx) * 0.5;
        let class = signal_class(link.signal);
        let _ = writeln!(
            out,
            "<path class=\"backbone {class}\" d=\"M{ax:.1} {ay:.1}H{mid:.1}V{by:.1}H{bx:.1}\"/>"
        );
    }
    out.push_str("</g>\n");
}

fn render_bus_pulses(
    out: &mut String,
    trace: &MatchTrace,
    blocks: &HashMap<&'static str, Block>,
    config: RenderConfig,
) {
    if trace.bus_transactions.is_empty() || trace.total_frames == 0 {
        return;
    }
    let total = config.total();
    let stride = (trace.bus_transactions.len() / 140).max(1);
    out.push_str("<g id=\"frontpage-native-bus-pulses\" clip-path=\"url(#clip-overview)\">\n");

    for event in trace.bus_transactions.iter().step_by(stride) {
        let Some(address) = event.address else {
            continue;
        };
        let Some(source_id) = source_group(event.address_source.as_str(), event.kind.as_str()) else {
            continue;
        };
        let target_id = target_group(memory_owner(address), event.kind.as_str());
        let Some(source) = blocks.get(source_id).copied() else {
            continue;
        };
        let Some(target) = blocks.get(target_id).copied() else {
            continue;
        };
        let Some(bus) = blocks.get("bus").copied() else {
            continue;
        };

        let (sx, sy) = source.center();
        let (bx, by) = bus.center();
        let (tx, ty) = target.center();
        let forward = format!("M{sx:.1} {sy:.1}L{bx:.1} {by:.1}L{tx:.1} {ty:.1}");
        let reverse = format!("M{tx:.1} {ty:.1}L{bx:.1} {by:.1}L{sx:.1} {sy:.1}");
        let moment = trace_moment(event.frame, event.ordinal, trace, config);
        let address_window = event_window(moment, 0.44, total);
        let data_window = event_window(moment + 0.12, 0.46, total);
        let data_path = if matches!(event.kind.as_str(), "read" | "fetch") {
            &reverse
        } else if matches!(event.kind.as_str(), "dma" | "scanout") {
            &reverse
        } else {
            &forward
        };
        let data = event.data.unwrap_or(0);

        let _ = writeln!(
            out,
            r##"<g data-bus-kind="{}" data-bus-address="{:04X}" data-bus-data="{:02X}" data-bus-owner="{}"><circle r="4.6" class="native-address" filter="url(#leader-soft-glow)" opacity="0"><animate attributeName="opacity" values="0;0;1;0;0" keyTimes="0;{:.6};{:.6};{:.6};1" dur="{total:.3}s" repeatCount="indefinite"/><animateMotion path="{}" keyPoints="0;0;1;1" keyTimes="0;{:.6};{:.6};1" dur="{total:.3}s" repeatCount="indefinite"/></circle><circle r="4.2" class="native-data" filter="url(#leader-soft-glow)" opacity="0"><animate attributeName="opacity" values="0;0;1;0;0" keyTimes="0;{:.6};{:.6};{:.6};1" dur="{total:.3}s" repeatCount="indefinite"/><animateMotion path="{}" keyPoints="0;0;1;1" keyTimes="0;{:.6};{:.6};1" dur="{total:.3}s" repeatCount="indefinite"/></circle><rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="7" fill="none" stroke="#58d6ff" stroke-width="3" filter="url(#leader-soft-glow)" opacity="0"><animate attributeName="opacity" values="0;0;1;0;0" keyTimes="0;{:.6};{:.6};{:.6};1" dur="{total:.3}s" repeatCount="indefinite"/></rect><g opacity="0"><animate attributeName="opacity" values="0;0;1;0;0" keyTimes="0;{:.6};{:.6};{:.6};1" dur="{total:.3}s" repeatCount="indefinite"/><rect x="{:.1}" y="{:.1}" width="116" height="24" rx="5" fill="#061019" stroke="#31566f"/><text x="{:.1}" y="{:.1}" fill="#ffcf83" font-size="8" font-weight="900">A {:04X}</text><text x="{:.1}" y="{:.1}" fill="#78e5ff" font-size="8" font-weight="900">D {:02X}</text></g></g>"##,
            event.kind.as_str(),
            address,
            data,
            owner_name(memory_owner(address)),
            address_window.0,
            address_window.1,
            address_window.2,
            xml_escape(&forward),
            address_window.0,
            address_window.2,
            data_window.0,
            data_window.1,
            data_window.2,
            xml_escape(data_path),
            data_window.0,
            data_window.2,
            target.x,
            target.y,
            target.w,
            target.h,
            data_window.0,
            data_window.1,
            data_window.2,
            data_window.0,
            data_window.1,
            data_window.2,
            (target.x + target.w - 120.0).max(34.0),
            target.y - 29.0,
            (target.x + target.w - 114.0).max(40.0),
            target.y - 13.0,
            address,
            (target.x + target.w - 58.0).max(96.0),
            target.y - 13.0,
            data,
        );
    }
    out.push_str("</g>\n");
}

fn render_detail_panel(
    out: &mut String,
    topology: &Topology,
    navigation: &NavigationModel,
    trace: &MatchTrace,
    config: RenderConfig,
) {
    let _ = writeln!(
        out,
        r##"<g id="frontpage-logic-microscope"><rect class="panel" x="{DETAIL_X}" y="{DETAIL_Y}" width="{DETAIL_W}" height="{DETAIL_H}" rx="13"/><text class="panel-kicker" x="602" y="139">LOGIC MICROSCOPE</text><text class="panel-meta" x="1158" y="139" text-anchor="end">ISOLATED CANONICAL VIEW</text><rect x="596" y="150" width="568" height="260" rx="10" fill="url(#leader-grid)"/></g>"##
    );
    for scene in DETAIL_SCENES {
        render_detail_scene(out, topology, navigation, trace, config, scene);
    }
}

fn render_detail_scene(
    out: &mut String,
    topology: &Topology,
    navigation: &NavigationModel,
    trace: &MatchTrace,
    config: RenderConfig,
    scene: DetailScene,
) {
    let Some(module) = navigation.module(scene.module) else {
        return;
    };
    if module.node_ids.is_empty() {
        return;
    }
    let total = config.total();
    let start = norm(scene.start.min(total - 0.03), total).max(0.000_01);
    let end = norm(scene.end.min(total - 0.02), total).max(start + 0.000_01);
    let inner = Rect::new(610.0, 179.0, 540.0, 211.0);
    let fit = fit_rect(module.bounds, inner, 8.0);
    let node_ids = module.node_ids.iter().map(String::as_str).collect::<HashSet<_>>();
    let font_size = (10.0 / fit.scale).clamp(8.0, 30.0);

    let _ = writeln!(
        out,
        r##"<g data-detail-module="{}" opacity="0"><animate attributeName="opacity" values="0;1;0;0" keyTimes="0;{start:.6};{end:.6};1" calcMode="discrete" dur="{total:.3}s" repeatCount="indefinite"/><text class="panel-title" x="610" y="165">{}</text><text class="panel-meta" x="1150" y="165" text-anchor="end">{}</text><g clip-path="url(#clip-detail)" transform="translate({:.4} {:.4}) scale({:.6})">"##,
        xml_escape(scene.module),
        xml_escape(scene.title),
        xml_escape(scene.subtitle),
        fit.tx,
        fit.ty,
        fit.scale,
    );

    for link in &topology.links {
        if !node_ids.contains(link.from.as_str()) || !node_ids.contains(link.to.as_str()) {
            continue;
        }
        let Some(from) = topology.node(&link.from) else {
            continue;
        };
        let Some(to) = topology.node(&link.to) else {
            continue;
        };
        let (x1, y1) = center(from.bounds);
        let (x2, y2) = center(to.bounds);
        let mid = (x1 + x2) * 0.5;
        let _ = writeln!(
            out,
            "<path class=\"detail-wire {}\" d=\"M{x1:.1} {y1:.1}H{mid:.1}V{y2:.1}H{x2:.1}\"/>",
            signal_class(link.signal)
        );
    }

    for node_id in &module.node_ids {
        let Some(node) = topology.node(node_id) else {
            continue;
        };
        let b = node.bounds;
        let _ = writeln!(
            out,
            r##"<g data-detail-node="{}"><rect class="detail-node" x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="{:.2}"/><text class="detail-label" x="{:.1}" y="{:.1}" text-anchor="middle" dominant-baseline="middle" font-size="{font_size:.2}">{}</text></g>"##,
            xml_escape(&node.id),
            b.x,
            b.y,
            b.w,
            b.h,
            5.0 / fit.scale,
            b.x + b.w * 0.5,
            b.y + b.h * 0.5,
            xml_escape(&node.title),
        );
    }

    render_detail_activity(out, topology, trace, config, scene, &node_ids, fit.scale);
    out.push_str("</g>");
    render_detail_event_labels(out, trace, config, scene);
    out.push_str("</g>\n");
}

fn render_detail_activity(
    out: &mut String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
    scene: DetailScene,
    node_ids: &HashSet<&str>,
    scale: f32,
) {
    let total = config.total();
    let bus_stride = (trace.bus_transactions.len() / 96).max(1);
    for event in trace.bus_transactions.iter().step_by(bus_stride) {
        let moment = trace_moment(event.frame, event.ordinal, trace, config);
        if moment < scene.start || moment >= scene.end {
            continue;
        }
        let phase = match event.kind.as_str() {
            "fetch" => PhaseKind::Fetch,
            "read" => PhaseKind::MemoryRead,
            "write" => PhaseKind::MemoryWrite,
            "input" => PhaseKind::Input,
            "dma" => PhaseKind::Dma,
            "scanout" => PhaseKind::Scanout,
            _ => continue,
        };
        let active = physical_activity_nodes(phase, event.address);
        let window = event_window(moment, 0.22, total);
        for id in active {
            if !node_ids.contains(id.as_str()) {
                continue;
            }
            let Some(node) = topology.node(&id) else {
                continue;
            };
            let b = node.bounds;
            let _ = writeln!(
                out,
                r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="{:.2}" fill="#58d6ff" fill-opacity=".18" stroke="#8ce8ff" stroke-width="{:.3}" vector-effect="non-scaling-stroke" filter="url(#leader-soft-glow)" opacity="0"><animate attributeName="opacity" values="0;0;1;0;0" keyTimes="0;{:.6};{:.6};{:.6};1" dur="{total:.3}s" repeatCount="indefinite"/></rect>"##,
                b.x - 2.0 / scale,
                b.y - 2.0 / scale,
                b.w + 4.0 / scale,
                b.h + 4.0 / scale,
                6.0 / scale,
                2.2,
                window.0,
                window.1,
                window.2,
            );
        }
    }

    if scene.module == "alu.ripple" {
        let stride = (trace.alu_events.len() / 72).max(1);
        for event in trace.alu_events.iter().step_by(stride) {
            let moment = trace_moment(event.frame, event.ordinal, trace, config);
            if moment < scene.start || moment >= scene.end {
                continue;
            }
            let window = event_window(moment, 0.28, total);
            for value in physical_alu_node_values(event.trace).into_iter().filter(|value| value.value) {
                if !node_ids.contains(value.node_id.as_str()) {
                    continue;
                }
                let Some(node) = topology.node(&value.node_id) else {
                    continue;
                };
                let b = node.bounds;
                let _ = writeln!(
                    out,
                    r##"<rect data-alu-bit="{}" data-alu-stage="{}" x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="{:.2}" fill="#ffe16a" fill-opacity=".24" stroke="#fff08c" stroke-width="2.4" vector-effect="non-scaling-stroke" filter="url(#leader-soft-glow)" opacity="0"><animate attributeName="opacity" values="0;0;1;0;0" keyTimes="0;{:.6};{:.6};{:.6};1" dur="{total:.3}s" repeatCount="indefinite"/></rect>"##,
                    value.bit,
                    xml_escape(value.stage),
                    b.x,
                    b.y,
                    b.w,
                    b.h,
                    5.0 / scale,
                    window.0,
                    window.1,
                    window.2,
                );
            }
        }
    }
}

fn render_detail_event_labels(out: &mut String, trace: &MatchTrace, config: RenderConfig, scene: DetailScene) {
    let total = config.total();
    if scene.module == "alu.ripple" {
        let stride = (trace.alu_events.len() / 48).max(1);
        for event in trace.alu_events.iter().step_by(stride) {
            let moment = trace_moment(event.frame, event.ordinal, trace, config);
            if moment < scene.start || moment >= scene.end {
                continue;
            }
            let window = event_window(moment, 0.34, total);
            let _ = writeln!(
                out,
                r##"<g opacity="0" data-alu-op="{}"><animate attributeName="opacity" values="0;0;1;0;0" keyTimes="0;{:.6};{:.6};{:.6};1" dur="{total:.3}s" repeatCount="indefinite"/><rect x="610" y="386" width="540" height="22" rx="4" fill="#07131d" stroke="#2e4d62"/><text x="620" y="401" fill="#f7df78" font-size="9" font-weight="900">{}  {:02X}  {:02X}  → {:02X}   CARRY {:03X}</text></g>"##,
                event.trace.op.as_str(),
                window.0,
                window.1,
                window.2,
                xml_escape(event.trace.op.as_str()),
                event.trace.lhs,
                event.trace.rhs,
                event.trace.result,
                event.trace.carry_chain,
            );
        }
    } else if scene.module == "decode.microcode" {
        let stride = (trace.micro_addresses.len() / 52).max(1);
        for event in trace.micro_addresses.iter().step_by(stride) {
            let moment = trace_moment(event.frame, event.ordinal, trace, config);
            if moment < scene.start || moment >= scene.end {
                continue;
            }
            let window = event_window(moment, 0.32, total);
            let _ = writeln!(
                out,
                r##"<g opacity="0" data-uaddr="{:02X}" data-ucontrol="{:06X}"><animate attributeName="opacity" values="0;0;1;0;0" keyTimes="0;{:.6};{:.6};{:.6};1" dur="{total:.3}s" repeatCount="indefinite"/><rect x="610" y="386" width="540" height="22" rx="4" fill="#07131d" stroke="#2e4d62"/><text x="620" y="401" fill="#ff91d8" font-size="9" font-weight="900">µADDR {:02X} · OPCODE {:02X} · CTRL {:06X} · {}</text></g>"##,
                event.address,
                event.control_bits,
                window.0,
                window.1,
                window.2,
                event.address,
                event.opcode,
                event.control_bits,
                xml_escape(event.label),
            );
        }
    } else if scene.module == "ramsys.pages" || scene.module == "bus.stack" || scene.module == "gpu.scanout" {
        let stride = (trace.bus_transactions.len() / 62).max(1);
        for event in trace.bus_transactions.iter().step_by(stride) {
            let moment = trace_moment(event.frame, event.ordinal, trace, config);
            if moment < scene.start || moment >= scene.end {
                continue;
            }
            let Some(address) = event.address else {
                continue;
            };
            if scene.module == "ramsys.pages" && memory_owner(address) != MemoryOwner::Ram {
                continue;
            }
            if scene.module == "gpu.scanout" && !matches!(event.kind.as_str(), "dma" | "scanout") {
                continue;
            }
            let window = event_window(moment, 0.32, total);
            let _ = writeln!(
                out,
                r##"<g opacity="0"><animate attributeName="opacity" values="0;0;1;0;0" keyTimes="0;{:.6};{:.6};{:.6};1" dur="{total:.3}s" repeatCount="indefinite"/><rect x="610" y="386" width="540" height="22" rx="4" fill="#07131d" stroke="#2e4d62"/><text x="620" y="401" fill="#7be5ff" font-size="9" font-weight="900">{} · A {:04X} · D {:02X} · PC {:04X} · {}</text></g>"##,
                window.0,
                window.1,
                window.2,
                xml_escape(event.kind.as_str()),
                address,
                event.data.unwrap_or(0),
                event.pc,
                xml_escape(event.control),
            );
        }
    }
}

fn render_video_panel(out: &mut String, trace: &MatchTrace, config: RenderConfig) {
    let _ = writeln!(
        out,
        r##"<g id="frontpage-native-video-replay"><rect class="panel" x="{VIDEO_X}" y="{VIDEO_Y}" width="{VIDEO_W}" height="{VIDEO_H}" rx="13"/><text class="panel-kicker" x="602" y="461">NATIVE VIDEO PIPELINE</text><text class="panel-meta" x="1158" y="461" text-anchor="end">128 × 96 · 1BPP · MSB-FIRST</text><rect x="604" y="472" width="248" height="130" rx="12" fill="#020705" stroke="#315347" stroke-width="2"/><rect x="606" y="474" width="244" height="126" rx="11" fill="url(#leader-crt)"/></g>"##
    );
    render_vram_replay(out, trace, config);
    render_video_pipeline(out, trace, config);
}

fn render_vram_replay(out: &mut String, trace: &MatchTrace, config: RenderConfig) {
    if trace.vram_checkpoints.is_empty() || trace.total_frames == 0 {
        return;
    }
    let total = config.total();
    let samples = sample_checkpoints(&trace.vram_checkpoints, 72);
    let sx = 606.0;
    let sy = 474.0;
    let scale_x = 244.0 / FRAMEBUFFER_WIDTH as f32;
    let scale_y = 126.0 / FRAMEBUFFER_HEIGHT as f32;
    let _ = writeln!(
        out,
        "<g clip-path=\"url(#clip-crt)\" transform=\"translate({sx:.1} {sy:.1}) scale({scale_x:.6} {scale_y:.6})\">"
    );
    for (index, checkpoint) in samples.iter().enumerate() {
        let start = trace_time(checkpoint.frame, trace.total_frames, config);
        let end = samples
            .get(index + 1)
            .map_or(config.game_end(), |next| trace_time(next.frame, trace.total_frames, config))
            .max(start + 0.001);
        let path = framebuffer_runs(&checkpoint.bytes);
        if path.is_empty() {
            continue;
        }
        let k1 = norm(start, total);
        let k2 = norm(end, total).max(k1 + 0.000_01);
        let _ = writeln!(
            out,
            r##"<path class="crt-pixel" data-vram-frame="{}" data-vram-checksum="{:08X}" d="{}" opacity="0"><animate attributeName="opacity" values="0;1;0;0" keyTimes="0;{k1:.6};{k2:.6};1" calcMode="discrete" dur="{total:.3}s" repeatCount="indefinite"/></path>"##,
            checkpoint.frame,
            checkpoint.checksum,
            path,
        );
    }
    out.push_str("</g>\n");

    let _ = writeln!(
        out,
        r##"<rect x="606" y="474" width="244" height="3" fill="#d8ffba" opacity=".08"><animate attributeName="y" values="474;597;474" dur="2.7s" repeatCount="indefinite"/></rect>"##
    );
}

fn render_video_pipeline(out: &mut String, trace: &MatchTrace, config: RenderConfig) {
    let stages = [
        ("VRAM", 872.0, 489.0, 54.0),
        ("DMA", 940.0, 489.0, 52.0),
        ("SHIFT", 1006.0, 489.0, 58.0),
        ("PIXEL", 1078.0, 489.0, 58.0),
    ];
    out.push_str("<g id=\"frontpage-video-stages\">\n");
    for (label, x, y, w) in stages {
        let _ = writeln!(
            out,
            r##"<rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="28" rx="5" fill="#09151e" stroke="#3f5c6d"/><text x="{:.1}" y="507" text-anchor="middle" fill="#b8cbd7" font-size="8" font-weight="900">{label}</text>"##,
            x + w * 0.5
        );
    }
    out.push_str("<path d=\"M926 503H940M992 503H1006M1064 503H1078M1136 503H1150\" stroke=\"#416272\" stroke-width=\"2\"/>\n");

    if trace.total_frames > 0 {
        let stride = (trace.bus_transactions.len() / 54).max(1);
        let total = config.total();
        for event in trace.bus_transactions.iter().step_by(stride) {
            if !matches!(event.kind.as_str(), "dma" | "scanout") {
                continue;
            }
            let moment = trace_moment(event.frame, event.ordinal, trace, config);
            let window = event_window(moment, 0.30, total);
            let (x, width) = if event.kind.as_str() == "dma" {
                (940.0, 52.0)
            } else {
                (1006.0, 130.0)
            };
            let _ = writeln!(
                out,
                r##"<rect data-video-stage="{}" x="{x:.1}" y="489" width="{width:.1}" height="28" rx="5" fill="#9cff78" fill-opacity=".16" stroke="#b8ff91" stroke-width="2.4" filter="url(#leader-soft-glow)" opacity="0"><animate attributeName="opacity" values="0;0;1;0;0" keyTimes="0;{:.6};{:.6};{:.6};1" dur="{total:.3}s" repeatCount="indefinite"/></rect>"##,
                event.kind.as_str(),
                window.0,
                window.1,
                window.2,
            );
        }
    }

    render_frame_telemetry(out, trace, config);
    out.push_str("</g>\n");
}

fn render_frame_telemetry(out: &mut String, trace: &MatchTrace, config: RenderConfig) {
    if trace.frames.is_empty() || trace.total_frames == 0 {
        return;
    }
    let total = config.total();
    let stride = (trace.frames.len() / 36).max(1);
    let frames = trace.frames.iter().step_by(stride).collect::<Vec<_>>();
    for (index, frame) in frames.iter().enumerate() {
        let start = trace_time(frame.frame, trace.total_frames, config);
        let end = frames
            .get(index + 1)
            .map_or(config.game_end(), |next| trace_time(next.frame, trace.total_frames, config))
            .max(start + 0.001);
        let k1 = norm(start, total);
        let k2 = norm(end, total).max(k1 + 0.000_01);
        let _ = writeln!(
            out,
            r##"<g opacity="0" data-video-frame="{}"><animate attributeName="opacity" values="0;1;0;0" keyTimes="0;{k1:.6};{k2:.6};1" calcMode="discrete" dur="{total:.3}s" repeatCount="indefinite"/><text class="telemetry-label" x="872" y="548">FRAME</text><text class="telemetry-value" x="928" y="548">{:04}</text><text class="telemetry-label" x="1000" y="548">SCORE</text><text class="telemetry-value" x="1054" y="548">{:03}</text><text class="telemetry-label" x="872" y="572">LIVES</text><text class="telemetry-value" x="928" y="572">{}</text><text class="telemetry-label" x="1000" y="572">PC</text><text class="telemetry-value" x="1054" y="572">{:04X}</text><text class="telemetry-label" x="872" y="596">ALIENS</text><text class="telemetry-value" x="928" y="596">{:02}</text><text class="telemetry-label" x="1000" y="596">VRAM</text><text x="1054" y="596" fill="#9cff78" font-size="9" font-weight="900">{:08X}</text></g>"##,
            frame.frame,
            frame.frame,
            frame.score,
            frame.lives,
            frame.pc,
            frame.alive_rows.iter().map(|row| row.count_ones()).sum::<u32>(),
            frame.vram_checksum,
        );
    }
}

fn render_bottom_telemetry(out: &mut String, trace: &MatchTrace, config: RenderConfig) {
    let _ = writeln!(
        out,
        r##"<g id="frontpage-native-telemetry"><rect x="24" y="628" width="1152" height="24" rx="6" fill="#071019" stroke="#20394c"/><text x="36" y="644" fill="#587489" font-size="8" font-weight="900">NATIVE µSTATE</text></g>"##
    );
    if trace.micro_cycles.is_empty() || trace.total_frames == 0 {
        return;
    }
    let total = config.total();
    let stride = (trace.micro_cycles.len() / 90).max(1);
    for event in trace.micro_cycles.iter().step_by(stride) {
        let moment = trace_moment(event.frame, event.ordinal, trace, config);
        let window = event_window(moment, 0.30, total);
        let _ = writeln!(
            out,
            r##"<g opacity="0" data-micro-pc="{:04X}" data-micro-mar="{:04X}" data-micro-mdr="{:02X}" data-micro-ir="{:02X}"><animate attributeName="opacity" values="0;0;1;0;0" keyTimes="0;{:.6};{:.6};{:.6};1" dur="{total:.3}s" repeatCount="indefinite"/><text x="146" y="644" fill="#d2e3ed" font-size="9" font-weight="900">{} / {}</text><text x="300" y="644" fill="#ffcf83" font-size="9" font-weight="900">PC {:04X}</text><text x="392" y="644" fill="#ffcf83" font-size="9" font-weight="900">MAR {:04X}</text><text x="500" y="644" fill="#78e5ff" font-size="9" font-weight="900">MDR {:02X}</text><text x="592" y="644" fill="#78e5ff" font-size="9" font-weight="900">IR {:02X}</text><text x="674" y="644" fill="#ff91d8" font-size="9" font-weight="900">{}</text></g>"##,
            event.pc,
            event.mar,
            event.mdr,
            event.ir,
            window.0,
            window.1,
            window.2,
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

fn render_contract_metadata(
    out: &mut String,
    topology: &Topology,
    navigation: &NavigationModel,
    trace: &MatchTrace,
) {
    let _ = writeln!(
        out,
        r##"<g id="frontpage-contract-metadata" display="none" data-default-view="{}" data-navigation-views="{}" data-trace-frames="{}" data-bus-events="{}" data-alu-events="{}" data-vram-checkpoints="{}" data-node-count="{}" data-link-count="{}">"##,
        xml_escape(&navigation.default_view),
        navigation.views.len(),
        trace.total_frames,
        trace.bus_transactions.len(),
        trace.alu_events.len(),
        trace.vram_checkpoints.len(),
        topology.nodes.len(),
        topology.links.len(),
    );
    for scene in DETAIL_SCENES {
        let _ = write!(out, "<g data-detail-module=\"{}\"/>", xml_escape(scene.module));
    }
    out.push_str("</g>\n");
}

fn overview_blocks() -> HashMap<&'static str, Block> {
    HashMap::from([
        ("clk", Block { x: 44.0, y: 176.0, w: 84.0, h: 56.0 }),
        ("pc", Block { x: 146.0, y: 158.0, w: 116.0, h: 82.0 }),
        ("decode", Block { x: 284.0, y: 152.0, w: 132.0, h: 92.0 }),
        ("gpu", Block { x: 438.0, y: 158.0, w: 104.0, h: 86.0 }),
        ("regs", Block { x: 106.0, y: 286.0, w: 150.0, h: 98.0 }),
        ("alu", Block { x: 284.0, y: 276.0, w: 170.0, h: 110.0 }),
        ("io", Block { x: 44.0, y: 418.0, w: 112.0, h: 82.0 }),
        ("romsys", Block { x: 174.0, y: 418.0, w: 112.0, h: 82.0 }),
        ("ramsys", Block { x: 304.0, y: 412.0, w: 132.0, h: 88.0 }),
        ("vramsys", Block { x: 454.0, y: 418.0, w: 88.0, h: 82.0 }),
        ("bus", Block { x: 82.0, y: 528.0, w: 414.0, h: 52.0 }),
    ])
}

fn short_group_label(id: &str) -> &'static str {
    match id {
        "clk" => "CLOCK",
        "pc" => "PC / FETCH",
        "decode" => "DECODE / µROM",
        "regs" => "REGISTER FILE",
        "alu" => "8-BIT ALU",
        "romsys" => "PROGRAM ROM",
        "ramsys" => "WORK RAM",
        "bus" => "SYSTEM BUS",
        "vramsys" => "VIDEO RAM",
        "io" => "INPUT / IRQ",
        "gpu" => "GPU / SCANOUT",
        _ => "SUBSYSTEM",
    }
}

fn group_capacity_label(id: &str) -> &'static str {
    match id {
        "clk" => "OSC · T0/T1/T2",
        "pc" => "16 BIT",
        "decode" => "256 × 24",
        "regs" => "8 × 8 BIT",
        "alu" => "8 RIPPLE SLICES",
        "romsys" => "8 KiB",
        "ramsys" => "24 KiB",
        "bus" => "ADDR 16 · DATA 8",
        "vramsys" => "2 KiB",
        "io" => "MMIO / TIMER",
        "gpu" => "DMA / 128×96",
        _ => "NATIVE",
    }
}

fn source_group(address_source: &str, kind: &str) -> Option<&'static str> {
    if matches!(kind, "dma" | "scanout") {
        return Some("gpu");
    }
    match address_source {
        "program_counter" => Some("pc"),
        "cpu" => Some("regs"),
        "dma" => Some("gpu"),
        "none" => Some("io"),
        _ => None,
    }
}

fn target_group(owner: MemoryOwner, kind: &str) -> &'static str {
    if kind == "scanout" {
        return "gpu";
    }
    match owner {
        MemoryOwner::Rom => "romsys",
        MemoryOwner::Ram => "ramsys",
        MemoryOwner::Vram => "vramsys",
        MemoryOwner::Mmio => "io",
        MemoryOwner::Unmapped => "bus",
    }
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

fn signal_class(signal: SignalKind) -> &'static str {
    match signal {
        SignalKind::Address => "native-address",
        SignalKind::Data => "native-data",
        SignalKind::Control => "native-control",
        SignalKind::Clock => "native-clock",
        SignalKind::Carry => "native-address",
        SignalKind::Video => "native-video",
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
    let tx = viewport.x + (viewport.w - rendered_w) * 0.5 - bounds.x * scale;
    let ty = viewport.y + (viewport.h - rendered_h) * 0.5 - bounds.y * scale;
    Fit { scale, tx, ty }
}

fn center(bounds: Rect) -> (f32, f32) {
    (bounds.x + bounds.w * 0.5, bounds.y + bounds.h * 0.5)
}

fn trace_moment(frame: u32, ordinal: u16, trace: &MatchTrace, config: RenderConfig) -> f32 {
    config.game_start()
        + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
        + f32::from(ordinal.min(63)) * 0.0012
}

fn trace_time(frame: u32, total_frames: u32, config: RenderConfig) -> f32 {
    config.game_start() + frame as f32 / total_frames.max(1) as f32 * config.game_seconds
}

fn event_window(moment: f32, duration: f32, total: f32) -> (f32, f32, f32) {
    let k1 = norm(moment, total);
    let k2 = norm(moment + 0.025, total).max(k1 + 0.000_01);
    let k3 = norm(moment + duration, total).max(k2 + 0.000_01);
    (k1, k2, k3)
}

fn norm(value: f32, total: f32) -> f32 {
    (value / total).clamp(0.0, 1.0)
}

fn framebuffer_runs(bytes: &[u8]) -> String {
    let mut path = String::new();
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

fn sample_checkpoints(
    checkpoints: &[leader_core::VramCheckpoint],
    max_samples: usize,
) -> Vec<&leader_core::VramCheckpoint> {
    if checkpoints.len() <= max_samples {
        return checkpoints.iter().collect();
    }
    let stride = checkpoints.len().div_ceil(max_samples);
    let mut sampled = checkpoints.iter().step_by(stride).collect::<Vec<_>>();
    if sampled.last().map(|checkpoint| checkpoint.frame)
        != checkpoints.last().map(|checkpoint| checkpoint.frame)
    {
        if let Some(last) = checkpoints.last() {
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
    fn fixed_frontpage_has_no_camera_animation() {
        let topology = build_topology();
        let trace = Machine::run_match("frontpage-fixed", 5000);
        let svg = render(&topology, &trace, RenderConfig::default());
        assert!(svg.contains("id=\"frontpage-overview\""));
        assert!(svg.contains("id=\"frontpage-logic-microscope\""));
        assert!(svg.contains("id=\"frontpage-native-video-replay\""));
        assert!(svg.contains("data-frontpage-version=\"observatory-v1\""));
        assert!(!svg.contains("leaderCamera"));
        assert!(!svg.contains("attributeName=\"viewBox\""));
    }

    #[test]
    fn frontpage_carries_native_bus_and_exact_ram_metadata() {
        let topology = build_topology();
        let trace = Machine::run_match("frontpage-native", 5000);
        let svg = render(&topology, &trace, RenderConfig::default());
        assert!(svg.contains("id=\"frontpage-native-bus-pulses\""));
        assert!(svg.contains("data-bus-address=\""));
        assert!(svg.contains("data-bus-data=\""));
        assert!(svg.contains("data-detail-module=\"ramsys.pages\""));
        assert!(svg.contains("data-vram-frame=\""));
    }

    #[test]
    fn vram_frames_explicitly_return_to_zero_opacity() {
        let topology = build_topology();
        let trace = Machine::run_match("frontpage-vram", 5000);
        let svg = render(&topology, &trace, RenderConfig::default());
        assert!(svg.contains("values=\"0;1;0;0\""));
        assert!(!svg.contains("values=\"0;1;1;0\""));
    }
}
