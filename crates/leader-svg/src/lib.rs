#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt::Write;

use leader_core::game::{ALIEN_COLS, ALIEN_ROWS, PLAYER_Y};
use leader_core::rng::hash_seed;
use leader_core::{FrameState, MatchTrace, PhaseKind, ProjectileSnapshot, Rect, Topology, ENEMY_SHOT_SLOTS};

#[derive(Debug, Clone, Copy)]
pub struct RenderConfig {
    pub width: u32,
    pub height: u32,
    pub assembly_seconds: f32,
    pub boot_seconds: f32,
    pub game_seconds: f32,
    pub outro_seconds: f32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            width: 900,
            height: 620,
            assembly_seconds: 46.0,
            boot_seconds: 9.0,
            game_seconds: 74.0,
            outro_seconds: 9.0,
        }
    }
}

impl RenderConfig {
    #[must_use]
    pub fn total(self) -> f32 {
        self.assembly_seconds + self.boot_seconds + self.game_seconds + self.outro_seconds
    }

    #[must_use]
    pub fn game_start(self) -> f32 {
        self.assembly_seconds + self.boot_seconds
    }

    #[must_use]
    pub fn game_end(self) -> f32 {
        self.game_start() + self.game_seconds
    }
}

#[must_use]
pub fn render(topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    let total = config.total();
    let schedule = assembly_schedule(topology, config.assembly_seconds);
    let mut out = String::with_capacity(1_500_000);

    let _ = writeln!(
        out,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}" role="img" aria-labelledby="title desc">"##,
        config.width,
        config.height,
        config.width,
        config.height
    );
    out.push_str(
        r##"<title id="title">Leader — deterministic visual CPU running Space Invaders</title>
<desc id="desc">Hardware assembles node by node, a deterministic machine boots, trace-backed datapath activity appears, and an autonomous Space Invaders match runs to game clear.</desc>
<defs>
  <linearGradient id="frame" x1="0" y1="0" x2="1" y2="1">
    <stop offset="0" stop-color="#ffad62"/>
    <stop offset=".48" stop-color="#ff775d"/>
    <stop offset="1" stop-color="#ff424c"/>
  </linearGradient>
  <filter id="glow" x="-80%" y="-80%" width="260%" height="260%">
    <feGaussianBlur stdDeviation="8" result="blur"/>
    <feMerge><feMergeNode in="blur"/><feMergeNode in="SourceGraphic"/></feMerge>
  </filter>
  <symbol id="alien-a" viewBox="0 0 8 6"><path fill="currentColor" d="M2 0h4v1h1v1h1v2H7v1H6V4H2v1H1V4H0V2h1V1h1zm0 2v1h1V2zm3 0v1h1V2zM1 5h1v1H1zm5 0h1v1H6z"/></symbol>
  <symbol id="alien-b" viewBox="0 0 8 6"><path fill="currentColor" d="M1 0h1v1h4V0h1v1h1v3H7v1H6V4H2v1H1V4H0V1h1zm1 2v1h1V2zm3 0v1h1V2zM0 5h2v1H0zm6 0h2v1H6z"/></symbol>
  <symbol id="player" viewBox="0 0 11 6"><path fill="currentColor" d="M5 0h1v2h3v1h2v3H0V3h2V2h3z"/></symbol>
</defs>
<style>
text{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}
.group{fill:#07101a;fill-opacity:.42;stroke:#35536b;stroke-width:4;stroke-dasharray:18 16}
.group-label{fill:#718ca2;font-size:27px;font-weight:700;letter-spacing:3px}
.node{fill:#0d1721;stroke:#4b6072;stroke-width:3}
.node-head{fill:#0a131c}.node-title{fill:#cad6df;font-size:17px;font-weight:700}.node-kind{fill:#627a8d;font-size:12px}
.wire{fill:none;stroke-width:4;stroke-linecap:round;stroke-linejoin:round;opacity:.50}
.address{stroke:#f2ae4f}.data{stroke:#4bc8f3}.control{stroke:#ef7caf}.clock{stroke:#67d9b3}.carry{stroke:#f7ce62}.video{stroke:#72d4e7}
.hot{fill:none;stroke-width:10;filter:url(#glow)}.hot-cpu{stroke:#67d9b3}.hot-alu{stroke:#f7ce62}.hot-mem{stroke:#f2ae4f}.hot-gpu{stroke:#72d4e7}.hot-ctrl{stroke:#ef7caf}
.static-final{display:none}@media(prefers-reduced-motion:reduce){.animated{display:none}.static-final{display:inline}}
</style>
"##,
    );

    render_chrome(&mut out, topology, trace, total);
    let _ = writeln!(
        out,
        r##"<svg class="animated" id="camera" x="18" y="108" width="864" height="484" viewBox="0 0 {:.0} {:.0}" preserveAspectRatio="xMidYMid meet"><rect width="100%" height="100%" fill="#07101a"/>"##,
        topology.width,
        topology.height
    );
    render_grid(&mut out, topology);
    render_groups(&mut out, topology);
    render_wires(&mut out, topology, &schedule, total);
    render_nodes(&mut out, topology, &schedule, total);
    render_activity(&mut out, topology, trace, config);
    render_display(&mut out, topology, trace, config);
    render_camera(&mut out, topology, config);
    out.push_str("</svg>\n");
    let _ = writeln!(
        out,
        r##"<g class="static-final"><rect x="30" y="118" width="840" height="462" fill="#07101a" stroke="#2e4050"/><text x="450" y="330" text-anchor="middle" fill="#8297a7" font-size="18">REDUCED MOTION · {}-NODE MACHINE</text><text x="450" y="360" text-anchor="middle" fill="#596c7b" font-size="12">cinematic replay disabled by system preference</text></g>"##,
        topology.nodes.len()
    );
    out.push_str("</svg>\n");
    out
}

fn render_chrome(out: &mut String, topology: &Topology, trace: &MatchTrace, total: f32) {
    let status = if trace.finished { "GAME CLEAR" } else { "TRACE LIMIT" };
    let _ = writeln!(
        out,
        r##"<rect x="3" y="3" width="894" height="614" rx="5" fill="#0d1117" stroke="url(#frame)" stroke-width="6"/>
<g font-family="Inter,Arial,sans-serif" font-size="58" font-weight="900" font-style="italic" text-anchor="middle">
  <text x="456" y="66" fill="#ff9b71">LEADER</text>
  <text x="447" y="70" fill="#ff4b4f" opacity=".82">LEADER</text>
  <text x="452" y="74" fill="#e8e677" opacity=".70">LEADER</text>
</g>
<rect x="300" y="18" width="300" height="58" fill="#0d1117" opacity=".74"/>
<text x="450" y="56" text-anchor="middle" fill="#ff9b71" font-family="Inter,Arial,sans-serif" font-size="45" font-weight="900" font-style="italic">LEADER</text>
<text x="450" y="94" text-anchor="middle" fill="#ff9b71" font-family="Inter,Arial,sans-serif" font-size="15" font-weight="800" letter-spacing="2">SYSTEMS ENGINEER · RUST</text>
<text x="28" y="607" fill="#5e6d7c" font-size="9" font-weight="700">TRACE {:016x}</text>
<text x="872" y="607" text-anchor="end" fill="#5e6d7c" font-size="9" font-weight="700">{} · {} NODES · {} SIGNALS · {:.0}s</text>"##,
        trace.seed_hash,
        status,
        topology.nodes.len(),
        topology.links.len(),
        total
    );
}

fn render_grid(out: &mut String, topology: &Topology) {
    out.push_str("<g opacity=\".34\">");
    let mut x = 0.0;
    while x <= topology.width {
        let _ = write!(
            out,
            "<path d=\"M{x:.0} 0V{:.0}\" stroke=\"#102033\" stroke-width=\"2\"/>",
            topology.height
        );
        x += 200.0;
    }
    let mut y = 0.0;
    while y <= topology.height {
        let _ = write!(
            out,
            "<path d=\"M0 {y:.0}H{:.0}\" stroke=\"#102033\" stroke-width=\"2\"/>",
            topology.width
        );
        y += 200.0;
    }
    out.push_str("</g>");
}

fn render_groups(out: &mut String, topology: &Topology) {
    for group in &topology.groups {
        let b = group.bounds;
        let _ = writeln!(
            out,
            r##"<g><rect class="group" x="{:.0}" y="{:.0}" width="{:.0}" height="{:.0}" rx="22"/><text class="group-label" x="{:.0}" y="{:.0}">{}</text></g>"##,
            b.x,
            b.y,
            b.w,
            b.h,
            b.x + 16.0,
            b.y + 34.0,
            xml_escape(&group.label)
        );
    }
}

fn render_nodes(
    out: &mut String,
    topology: &Topology,
    schedule: &HashMap<String, f32>,
    total: f32,
) {
    for node in &topology.nodes {
        let start = schedule.get(&node.id).copied().unwrap_or(0.0);
        let settled = (start + 0.55).min(total - 0.01);
        let k1 = norm(start, total);
        let k2 = norm(settled, total);
        let (dx, dy) = spawn_offset(&node.id);
        let b = node.bounds;
        let _ = writeln!(
            out,
            r##"<g id="node-{}" opacity="0"><animate attributeName="opacity" values="0;0;1;1" keyTimes="0;{k1:.6};{k2:.6};1" dur="{total:.3}s" repeatCount="indefinite"/><g transform="translate({:.0} {:.0})"><animateTransform attributeName="transform" type="translate" additive="sum" values="{dx:.0} {dy:.0};{dx:.0} {dy:.0};0 0;0 0" keyTimes="0;{k1:.6};{k2:.6};1" dur="{total:.3}s" repeatCount="indefinite"/><rect class="node" width="{:.0}" height="{:.0}" rx="8"/><rect class="node-head" x="2" y="2" width="{:.0}" height="26" rx="6"/><text class="node-title" x="9" y="19">{}</text><text class="node-kind" x="9" y="{:.0}">{}</text></g></g>"##,
            xml_escape(&node.id),
            b.x,
            b.y,
            b.w,
            b.h,
            (b.w - 4.0).max(0.0),
            xml_escape(&node.title),
            (b.h - 10.0).max(36.0),
            xml_escape(&node.kind)
        );
    }
}

fn render_wires(
    out: &mut String,
    topology: &Topology,
    schedule: &HashMap<String, f32>,
    total: f32,
) {
    for link in &topology.links {
        let (Some(from), Some(to)) = (topology.node(&link.from), topology.node(&link.to)) else {
            continue;
        };
        let start = schedule
            .get(&link.from)
            .copied()
            .unwrap_or(0.0)
            .max(schedule.get(&link.to).copied().unwrap_or(0.0))
            + 0.12;
        let done = start + 0.42;
        let k1 = norm(start, total);
        let k2 = norm(done, total);
        let path = orthogonal_path(from.bounds, to.bounds);
        let _ = writeln!(
            out,
            r##"<path class="wire {}" d="{}" pathLength="1" stroke-dasharray="1" stroke-dashoffset="1"><animate attributeName="stroke-dashoffset" values="1;1;0;0" keyTimes="0;{k1:.6};{k2:.6};1" dur="{total:.3}s" repeatCount="indefinite"/></path>"##,
            link.signal.css_class(),
            path
        );
    }
}

fn render_activity(out: &mut String, topology: &Topology, trace: &MatchTrace, config: RenderConfig) {
    if trace.micro_samples.is_empty() || trace.total_frames == 0 {
        return;
    }
    let total = config.total();
    let stride = (trace.micro_samples.len() / 160).max(1);
    for sample in trace.micro_samples.iter().step_by(stride) {
        let moment = trace_time(sample.frame, trace.total_frames, config)
            + f32::from(sample.ordinal.min(15)) * 0.004;
        let class = match sample.phase {
            PhaseKind::Alu => "hot hot-alu",
            PhaseKind::MemoryRead | PhaseKind::MemoryWrite => "hot hot-mem",
            PhaseKind::Dma | PhaseKind::Scanout => "hot hot-gpu",
            PhaseKind::Decode | PhaseKind::Input | PhaseKind::VBlank => "hot hot-ctrl",
            PhaseKind::Fetch => "hot hot-cpu",
        };
        let k1 = norm(moment, total);
        let k2 = norm(moment + 0.03, total);
        let k3 = norm(moment + 0.20, total);
        let ids = active_nodes(sample.phase, sample.address, topology);
        if ids.is_empty() {
            continue;
        }
        let _ = write!(
            out,
            r##"<g opacity="0"><animate attributeName="opacity" values="0;0;1;0;0" keyTimes="0;{k1:.6};{k2:.6};{k3:.6};1" dur="{total:.3}s" repeatCount="indefinite"/>"##
        );
        for id in ids {
            if let Some(node) = topology.node(&id) {
                let b = node.bounds;
                let _ = write!(
                    out,
                    r##"<rect class="{class}" x="{:.0}" y="{:.0}" width="{:.0}" height="{:.0}" rx="10"/>"##,
                    b.x - 4.0,
                    b.y - 4.0,
                    b.w + 8.0,
                    b.h + 8.0
                );
            }
        }
        out.push_str("</g>");
    }
}

fn render_display(out: &mut String, topology: &Topology, trace: &MatchTrace, config: RenderConfig) {
    let Some(display) = topology.node("display") else {
        return;
    };
    if trace.frames.is_empty() {
        return;
    }
    let b = display.bounds;
    let scale = 2.42_f32;
    let sx = b.x + 53.0;
    let sy = b.y + 55.0;
    let total = config.total();
    let k1 = norm(config.game_start(), total);
    let k2 = norm(config.game_end(), total);
    let _ = write!(
        out,
        r##"<g opacity="0"><animate attributeName="opacity" values="0;0;1;1;0;0" keyTimes="0;{k1:.6};{:.6};{k2:.6};{:.6};1" dur="{total:.3}s" repeatCount="indefinite"/><rect x="{sx:.1}" y="{sy:.1}" width="{:.1}" height="{:.1}" rx="8" fill="#010505" stroke="#315048" stroke-width="5"/><g transform="translate({sx:.1} {sy:.1}) scale({scale:.3})"><rect width="128" height="96" fill="#010505"/>"##,
        norm(config.game_start() + 0.2, total),
        norm(config.game_end() + 0.2, total),
        128.0 * scale,
        96.0 * scale
    );
    render_game(out, trace, config, total);
    out.push_str("</g></g>");
}

fn render_game(out: &mut String, trace: &MatchTrace, config: RenderConfig, total: f32) {
    let frames = sample_frames(&trace.frames, 180);
    let (fleet_values, fleet_keys) = transform_series(
        &frames,
        trace.total_frames,
        config,
        total,
        |frame| (f32::from(frame.fleet_x), f32::from(frame.fleet_y)),
    );
    let _ = write!(
        out,
        r##"<g color="#b7ff72"><animateTransform attributeName="transform" type="translate" values="{fleet_values}" keyTimes="{fleet_keys}" dur="{total:.3}s" repeatCount="indefinite" calcMode="linear"/>"##
    );
    for row in 0..ALIEN_ROWS {
        for col in 0..ALIEN_COLS {
            let symbol = if (row + col) % 2 == 0 { "alien-a" } else { "alien-b" };
            let _ = write!(
                out,
                r##"<use href="#{symbol}" x="{}" y="{}" width="8" height="6""##,
                col * 12,
                row * 13
            );
            if let Some(kill) = trace.kills.iter().find(|kill| kill.row == row && kill.col == col) {
                let t = trace_time(kill.frame, trace.total_frames, config);
                let k1 = norm(t, total);
                let k2 = norm(t + 0.08, total);
                let _ = write!(
                    out,
                    r##" opacity="1"><animate attributeName="opacity" values="1;1;0;0" keyTimes="0;{k1:.6};{k2:.6};1" dur="{total:.3}s" repeatCount="indefinite"/></use>"##
                );
            } else {
                out.push_str("/>");
            }
        }
    }
    out.push_str("</g>");

    let (player_values, player_keys) = transform_series(
        &frames,
        trace.total_frames,
        config,
        total,
        |frame| (f32::from(frame.player_x - 5), f32::from(PLAYER_Y - 5)),
    );
    let _ = write!(
        out,
        r##"<g color="#b7ff72"><animateTransform attributeName="transform" type="translate" values="{player_values}" keyTimes="{player_keys}" dur="{total:.3}s" repeatCount="indefinite" calcMode="linear"/><use href="#player" width="11" height="6"/></g><rect y="93" width="128" height="1" fill="#17382c"/>"##
    );

    render_projectile_track(out, trace, config, total, true, 0);
    for slot in 0..ENEMY_SHOT_SLOTS {
        render_projectile_track(out, trace, config, total, false, slot);
    }

    let clear = config.game_end();
    let _ = write!(
        out,
        r##"<text x="64" y="50" text-anchor="middle" fill="#b7ff72" font-size="8" font-weight="900" opacity="0">GAME CLEAR<animate attributeName="opacity" values="0;0;1;1;0" keyTimes="0;{:.6};{:.6};{:.6};1" dur="{total:.3}s" repeatCount="indefinite"/></text>"##,
        norm(clear, total),
        norm(clear + 0.20, total),
        norm(total - 0.30, total)
    );
}

fn render_projectile_track(
    out: &mut String,
    trace: &MatchTrace,
    config: RenderConfig,
    total: f32,
    player: bool,
    slot: usize,
) {
    let projectile_at = |frame: &FrameState| -> Option<ProjectileSnapshot> {
        if player {
            frame.player_shot
        } else {
            frame.enemy_shots[slot]
        }
    };

    let mut start: Option<(usize, i16, i16)> = None;
    for (index, frame) in trace.frames.iter().enumerate() {
        let projectile = projectile_at(frame);
        match (start, projectile) {
            (None, Some(projectile)) => start = Some((index, projectile.x, projectile.y)),
            (Some((start_index, start_x, start_y)), None) => {
                let end_index = index.saturating_sub(1);
                if let Some(end) = projectile_at(&trace.frames[end_index]) {
                    render_projectile_segment(
                        out,
                        trace,
                        config,
                        total,
                        player,
                        start_index,
                        end_index,
                        start_x,
                        start_y,
                        end,
                    );
                }
                start = None;
            }
            _ => {}
        }
    }

    if let Some((start_index, start_x, start_y)) = start {
        let end_index = trace.frames.len().saturating_sub(1);
        if let Some(end) = projectile_at(&trace.frames[end_index]) {
            render_projectile_segment(
                out,
                trace,
                config,
                total,
                player,
                start_index,
                end_index,
                start_x,
                start_y,
                end,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_projectile_segment(
    out: &mut String,
    trace: &MatchTrace,
    config: RenderConfig,
    total: f32,
    player: bool,
    start_index: usize,
    end_index: usize,
    start_x: i16,
    start_y: i16,
    end: ProjectileSnapshot,
) {
    let t1 = trace_time(trace.frames[start_index].frame, trace.total_frames, config);
    let t2 = trace_time(trace.frames[end_index].frame, trace.total_frames, config).max(t1 + 0.04);
    let class = if player { "#e8e677" } else { "#ff8065" };
    let _ = write!(
        out,
        r##"<rect fill="{class}" width="1" height="{}" opacity="0"><animate attributeName="opacity" values="0;0;1;1;0;0" keyTimes="0;{:.6};{:.6};{:.6};{:.6};1" dur="{total:.3}s" repeatCount="indefinite"/><animateTransform attributeName="transform" type="translate" values="{start_x} {start_y};{start_x} {start_y};{} {};{} {}" keyTimes="0;{:.6};{:.6};1" dur="{total:.3}s" repeatCount="indefinite"/></rect>"##,
        if player { 4 } else { 3 },
        norm(t1, total),
        norm(t1 + 0.02, total),
        norm(t2, total),
        norm(t2 + 0.02, total),
        end.x,
        end.y,
        end.x,
        end.y,
        norm(t1, total),
        norm(t2, total)
    );
}

fn render_camera(out: &mut String, topology: &Topology, config: RenderConfig) {
    let total = config.total();
    let mut track: Vec<(f32, Rect)> = Vec::new();
    let mut groups = topology.groups.clone();
    groups.sort_by_key(|group| group.assembly_rank);
    let span = config.assembly_seconds / groups.len().max(1) as f32;

    for (index, group) in groups.iter().enumerate() {
        let start = index as f32 * span;
        track.push((start, group.bounds.padded(65.0)));
        track.push((start + span * 0.76, group.bounds.padded(65.0)));
    }
    let full = Rect::new(0.0, 0.0, topology.width, topology.height);
    track.push((config.assembly_seconds, full));

    for (index, id) in ["clk", "pc", "decode", "alu", "bus", "gpu"].iter().enumerate() {
        if let Some(group) = topology.group(id) {
            track.push((config.assembly_seconds + 0.8 + index as f32 * 1.25, group.bounds.padded(85.0)));
        }
    }
    track.push((config.game_start(), full));

    for (index, id) in ["pc", "decode", "alu", "ramsys", "gpu"].iter().enumerate() {
        if let Some(group) = topology.group(id) {
            track.push((config.game_start() + index as f32 * 1.30, group.bounds.padded(90.0)));
        }
    }
    if let Some(display) = topology.node("display") {
        track.push((config.game_start() + 7.5, display.bounds.padded(65.0)));
        track.push((config.game_end() - 1.5, display.bounds.padded(40.0)));
    }
    track.push((config.game_end() + 1.5, full));
    track.push((total - 0.1, full));
    track.sort_by(|left, right| left.0.total_cmp(&right.0));

    let values = track
        .iter()
        .map(|(_, rect)| format!("{:.1} {:.1} {:.1} {:.1}", rect.x, rect.y, rect.w.max(1.0), rect.h.max(1.0)))
        .collect::<Vec<_>>()
        .join(";");
    let keys = track
        .iter()
        .map(|(time, _)| format!("{:.6}", norm(*time, total)))
        .collect::<Vec<_>>()
        .join(";");
    let _ = write!(
        out,
        r##"<animate attributeName="viewBox" values="{values}" keyTimes="{keys}" dur="{total:.3}s" repeatCount="indefinite" calcMode="linear"/>"##
    );
}

fn assembly_schedule(topology: &Topology, seconds: f32) -> HashMap<String, f32> {
    let mut schedule = HashMap::new();
    let mut groups = topology.groups.clone();
    groups.sort_by_key(|group| group.assembly_rank);
    let span = seconds / groups.len().max(1) as f32;

    for (group_index, group) in groups.iter().enumerate() {
        let mut nodes = topology
            .nodes
            .iter()
            .filter(|node| node.group == group.id)
            .collect::<Vec<_>>();
        nodes.sort_by(|left, right| {
            left.bounds
                .y
                .total_cmp(&right.bounds.y)
                .then(left.bounds.x.total_cmp(&right.bounds.x))
        });
        let step = if nodes.is_empty() {
            0.0
        } else {
            span * 0.75 / nodes.len() as f32
        };
        for (node_index, node) in nodes.into_iter().enumerate() {
            schedule.insert(
                node.id.clone(),
                group_index as f32 * span + node_index as f32 * step + 0.12,
            );
        }
    }
    schedule
}

fn active_nodes(phase: PhaseKind, address: Option<u16>, topology: &Topology) -> Vec<String> {
    let mut ids = match phase {
        PhaseKind::Fetch => vec!["clock", "clkGate", "phase0", "pcMuxLo", "pcMuxHi", "addrBuf"],
        PhaseKind::Decode => vec!["opHi", "opLo", "decA", "decB", "microAddr", "microRom"],
        PhaseKind::Input => vec!["kbd", "inputLatch", "dataBuf"],
        PhaseKind::MemoryRead => vec!["addrBuf", "dataBuf"],
        PhaseKind::Alu => vec!["readMuxA", "readMuxB", "aluSel", "writeBus", "flagZ", "flagC", "flagN"],
        PhaseKind::MemoryWrite => vec!["writeBus", "dataBuf", "ctrlBuf"],
        PhaseKind::Dma => vec!["arb", "dmaAddr", "dmaData", "dataBuf", "vramPageDec", "vramPage0"],
        PhaseKind::Scanout => vec!["spriteRom", "xCounter", "yCounter", "pixelMux", "scanShift", "hsync", "vsync", "display"],
        PhaseKind::VBlank => vec!["vsync", "timer", "irqAnd", "irqLatch", "microAddr"],
    }
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();

    if phase == PhaseKind::Alu {
        for bit in 0..8 {
            for prefix in ["xorA", "xorB", "andA", "andB", "orC", "muxR"] {
                ids.push(format!("{prefix}{bit}"));
            }
        }
    }

    if let Some(address) = address {
        match address {
            0x0000..=0x1fff => {
                ids.push("romRowDec".to_owned());
                ids.push(format!("romPage{}", address >> 8));
            }
            0x2000..=0x7fff => {
                ids.push("ramPageDec".to_owned());
                ids.push(format!("ramPage{}", ((address - 0x2000) >> 8).min(95)));
            }
            0x8000..=0x87ff => {
                ids.push("vramPageDec".to_owned());
                ids.push(format!("vramPage{}", ((address - 0x8000) >> 8).min(7)));
            }
            _ => {}
        }
    }
    ids.retain(|id| topology.node(id).is_some());
    ids
}

fn sample_frames(frames: &[FrameState], max_samples: usize) -> Vec<&FrameState> {
    if frames.len() <= max_samples {
        return frames.iter().collect();
    }
    let stride = (frames.len() / max_samples).max(1);
    let mut sampled = frames.iter().step_by(stride).collect::<Vec<_>>();
    if sampled.last().map(|frame| frame.frame) != frames.last().map(|frame| frame.frame) {
        if let Some(last) = frames.last() {
            sampled.push(last);
        }
    }
    sampled
}

fn transform_series<F>(
    frames: &[&FrameState],
    total_frames: u32,
    config: RenderConfig,
    total: f32,
    position: F,
) -> (String, String)
where
    F: Fn(&FrameState) -> (f32, f32),
{
    let first = frames[0];
    let last = *frames.last().unwrap_or(&first);
    let first_pos = position(first);
    let last_pos = position(last);
    let mut values = vec![
        format!("{:.1} {:.1}", first_pos.0, first_pos.1),
        format!("{:.1} {:.1}", first_pos.0, first_pos.1),
    ];
    let mut keys = vec!["0".to_owned(), format!("{:.6}", norm(config.game_start(), total))];

    for frame in frames {
        let pos = position(frame);
        values.push(format!("{:.1} {:.1}", pos.0, pos.1));
        keys.push(format!(
            "{:.6}",
            norm(trace_time(frame.frame, total_frames, config), total)
        ));
    }
    values.push(format!("{:.1} {:.1}", last_pos.0, last_pos.1));
    values.push(format!("{:.1} {:.1}", last_pos.0, last_pos.1));
    keys.push(format!("{:.6}", norm(config.game_end(), total)));
    keys.push("1".to_owned());
    (values.join(";"), keys.join(";"))
}

fn trace_time(frame: u32, total_frames: u32, config: RenderConfig) -> f32 {
    config.game_start()
        + frame as f32 / total_frames.max(1) as f32 * config.game_seconds
}

fn norm(value: f32, total: f32) -> f32 {
    (value / total).clamp(0.0, 1.0)
}

fn spawn_offset(id: &str) -> (f32, f32) {
    let hash = hash_seed(id);
    let distance = 500.0 + ((hash >> 8) & 1023) as f32;
    let cross = -420.0 + ((hash >> 20) & 1023) as f32 * 0.82;
    match hash & 3 {
        0 => (-distance, cross),
        1 => (distance, cross),
        2 => (cross, -distance),
        _ => (cross, distance),
    }
}

fn orthogonal_path(from: Rect, to: Rect) -> String {
    let from_center = (from.x + from.w / 2.0, from.y + from.h / 2.0);
    let to_center = (to.x + to.w / 2.0, to.y + to.h / 2.0);
    let x1 = if to_center.0 >= from_center.0 { from.x + from.w } else { from.x };
    let x2 = if to_center.0 >= from_center.0 { to.x } else { to.x + to.w };
    let middle = (x1 + x2) / 2.0;
    format!("M{x1:.1} {:.1}H{middle:.1}V{:.1}H{x2:.1}", from_center.1, to_center.1)
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
    fn renderer_is_declarative_and_has_camera() {
        let topology = build_topology();
        let trace = Machine::run_match("svg-test", 5_000);
        let svg = render(&topology, &trace, RenderConfig::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("attributeName=\"viewBox\""));
        assert!(svg.contains("GAME CLEAR"));
        assert!(!svg.contains("<script"));
        assert!(!svg.contains("javascript:"));
    }

    #[test]
    fn renderer_replays_all_enemy_projectile_slots() {
        let topology = build_topology();
        let trace = Machine::run_match("svg-multi-shot", 5_000);
        assert!(trace
            .frames
            .iter()
            .any(|frame| frame.enemy_shots.iter().flatten().count() >= 2));
        let svg = render(&topology, &trace, RenderConfig::default());
        assert!(svg.matches("fill=\"#ff8065\"").count() >= 2);
    }
}
