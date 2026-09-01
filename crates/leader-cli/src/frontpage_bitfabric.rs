use std::fmt::Write as _;

use leader_core::{memory_fabric_specs, MatchTrace, MemoryOwner, Rect, Topology};

const BITCELL_COLS_PER_BYTE: usize = 4;
const BITCELL_ROWS_PER_BYTE: usize = 2;
const MICROCODE_ROWS: usize = 256;
const MICROCODE_BITS: usize = 24;
const MAX_MICROCODE_EVENTS: usize = 72;

/// Adds the sub-byte physical fabric that is deliberately kept out of the
/// macro-node topology DOM. Each memory byte is represented by eight bit-cell
/// sites through a repeating page-local pattern, yielding 278,528 visible
/// memory bit locations without allocating 278,528 SVG elements.
#[must_use]
pub fn apply(mut svg: String, topology: &Topology, trace: &MatchTrace) -> String {
    if !svg.contains("data-frontpage-version=\"physical-die-v2\"") {
        return svg;
    }

    let mut defs = String::with_capacity(180_000);
    let mut fabric = String::with_capacity(120_000);
    defs.push_str("<g id=\"v2-bitcell-pattern-definitions\">\n");
    fabric.push_str("<g id=\"v2-memory-bitcell-fabric\" aria-label=\"278528 physical memory bit-cell sites\">\n");

    for spec in memory_fabric_specs() {
        let color = match spec.owner {
            MemoryOwner::Rom => "#a98cff",
            MemoryOwner::Ram => "#61dfff",
            MemoryOwner::Vram => "#a9ff7b",
            MemoryOwner::Mmio | MemoryOwner::Unmapped => continue,
        };
        for page in 0..spec.page_count {
            let node_id = format!("{}{page}", spec.page_prefix);
            let Some(node) = topology.node(&node_id) else {
                continue;
            };
            let geometry = page_geometry(node.bounds);
            let pattern_id = format!("v2-bitpat-{node_id}");
            render_bit_pattern(&mut defs, &pattern_id, geometry, color);
            let _ = writeln!(
                fabric,
                r##"<rect data-bitcell-page="{node_id}" data-bitcell-sites="2048" x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}" fill="url(#{pattern_id})" opacity=".48"/>"##,
                geometry.x,
                geometry.y,
                geometry.w,
                geometry.h,
            );
        }
    }

    defs.push_str("</g>\n");
    fabric.push_str("</g>\n");

    let (micro_defs, micro_fabric) = render_microcode_fabric(topology, trace);
    defs.push_str(&micro_defs);
    fabric.push_str(&micro_fabric);

    if let Some(index) = svg.find("</defs>") {
        svg.insert_str(index, &defs);
    }
    if let Some(index) = svg.find("<g id=\"v2-native-bus-propagation\">") {
        svg.insert_str(index, &fabric);
    }

    // Keep the older byte-cell layer as a faint address lattice behind the
    // true bit-cell fabric while preserving a single valid opacity attribute.
    svg = svg.replace(
        "opacity=\".30\" data-memory-page=",
        "opacity=\".08\" data-memory-page=",
    );
    svg
}

#[derive(Debug, Clone, Copy)]
struct PageGeometry {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    byte_w: f32,
    byte_h: f32,
}

fn page_geometry(bounds: Rect) -> PageGeometry {
    let pad_x = bounds.w * 0.055;
    let pad_top = bounds.h * 0.17;
    let pad_bottom = bounds.h * 0.055;
    let w = (bounds.w - pad_x * 2.0).max(1.0);
    let h = (bounds.h - pad_top - pad_bottom).max(1.0);
    PageGeometry {
        x: bounds.x + pad_x,
        y: bounds.y + pad_top,
        w,
        h,
        byte_w: w / 16.0,
        byte_h: h / 16.0,
    }
}

fn render_bit_pattern(out: &mut String, id: &str, geometry: PageGeometry, color: &str) {
    let bit_w = geometry.byte_w / BITCELL_COLS_PER_BYTE as f32;
    let bit_h = geometry.byte_h / BITCELL_ROWS_PER_BYTE as f32;
    let dot_w = (bit_w * 0.48).max(0.12);
    let dot_h = (bit_h * 0.44).max(0.12);
    let _ = writeln!(
        out,
        r##"<pattern id="{id}" x="{:.4}" y="{:.4}" width="{:.5}" height="{:.5}" patternUnits="userSpaceOnUse">"##,
        geometry.x,
        geometry.y,
        geometry.byte_w,
        geometry.byte_h,
    );
    for bit in 0..8 {
        let column = bit % BITCELL_COLS_PER_BYTE;
        let row = bit / BITCELL_COLS_PER_BYTE;
        let x = column as f32 * bit_w + (bit_w - dot_w) * 0.5;
        let y = row as f32 * bit_h + (bit_h - dot_h) * 0.5;
        let opacity = if bit % 2 == 0 { 0.72 } else { 0.48 };
        let _ = writeln!(
            out,
            r##"<rect x="{x:.5}" y="{y:.5}" width="{dot_w:.5}" height="{dot_h:.5}" fill="{color}" opacity="{opacity:.2}"/>"##,
        );
    }
    out.push_str("</pattern>\n");
}

fn render_microcode_fabric(topology: &Topology, trace: &MatchTrace) -> (String, String) {
    let Some(node) = topology.node("microRom") else {
        return (String::new(), String::new());
    };

    let bounds = node.bounds;
    let pad_x = bounds.w * 0.075;
    let pad_top = bounds.h * 0.25;
    let pad_bottom = bounds.h * 0.08;
    let x = bounds.x + pad_x;
    let y = bounds.y + pad_top;
    let w = (bounds.w - pad_x * 2.0).max(1.0);
    let h = (bounds.h - pad_top - pad_bottom).max(1.0);
    let row_h = h / MICROCODE_ROWS as f32;
    let bit_w = w / MICROCODE_BITS as f32;

    let mut defs = String::with_capacity(6_000);
    let mut fabric = String::with_capacity(80_000);
    let _ = writeln!(
        defs,
        r##"<pattern id="v2-microcode-row-pattern" x="{x:.4}" y="{y:.4}" width="{w:.4}" height="{row_h:.6}" patternUnits="userSpaceOnUse">"##,
    );
    for bit in 0..MICROCODE_BITS {
        let bx = bit as f32 * bit_w + bit_w * 0.24;
        let bw = (bit_w * 0.50).max(0.08);
        let _ = writeln!(
            defs,
            r##"<rect x="{bx:.5}" y="0" width="{bw:.5}" height="{:.6}" fill="#ff78ca" opacity=".46"/>"##,
            (row_h * 0.46).max(0.03),
        );
    }
    defs.push_str("</pattern>\n");

    let _ = writeln!(
        fabric,
        r##"<g id="v2-microcode-bitcell-fabric" data-microcode-rows="256" data-microcode-bits="24" data-microcode-cells="6144"><rect x="{x:.3}" y="{y:.3}" width="{w:.3}" height="{h:.3}" fill="url(#v2-microcode-row-pattern)" opacity=".68"/>"##,
    );

    if !trace.micro_addresses.is_empty() && trace.total_frames > 0 {
        let config = crate::frontpage::render_config();
        let sampled = sample_slice(&trace.micro_addresses, MAX_MICROCODE_EVENTS);
        let total = config.total();
        for (index, event) in sampled.iter().enumerate() {
            let start = trace_moment(event.frame, event.ordinal, trace, config);
            let end = sampled
                .get(index + 1)
                .map_or(config.game_end(), |next| {
                    trace_moment(next.frame, next.ordinal, trace, config)
                })
                .max(start + 0.001);
            let k1 = norm(start, total);
            let k2 = norm(end, total).max(k1 + 0.000_01);
            let row_y = y + f32::from(event.address) * row_h;
            let visible_h = (row_h * 28.0).max(3.0);
            let highlight_y = row_y - visible_h * 0.5;
            let _ = writeln!(
                fabric,
                r##"<g opacity="0" data-uaddr="{:02X}" data-ucontrol="{:06X}" data-uopcode="{:02X}"><animate attributeName="opacity" values="0;1;0;0" keyTimes="0;{k1:.6};{k2:.6};1" calcMode="discrete" dur="{total:.3}s" repeatCount="indefinite"/><rect x="{x:.3}" y="{highlight_y:.3}" width="{w:.3}" height="{visible_h:.3}" fill="#ff72c8" fill-opacity=".10" stroke="#ff9bdc" stroke-width="2.5" vector-effect="non-scaling-stroke" filter="url(#v2-glow)"/>"##,
                event.address,
                event.control_bits,
                event.opcode,
            );
            for bit in 0..MICROCODE_BITS {
                if event.control_bits & (1_u32 << bit) == 0 {
                    continue;
                }
                let bx = x + bit as f32 * bit_w + bit_w * 0.18;
                let bw = bit_w * 0.64;
                let _ = writeln!(
                    fabric,
                    r##"<rect x="{bx:.4}" y="{:.4}" width="{bw:.4}" height="{:.4}" fill="#fff1a1" filter="url(#v2-hot)"/>"##,
                    highlight_y + visible_h * 0.18,
                    visible_h * 0.64,
                );
            }
            fabric.push_str("</g>\n");
        }
    }
    fabric.push_str("</g>\n");
    (defs, fabric)
}

fn trace_moment(
    frame: u32,
    ordinal: u16,
    trace: &MatchTrace,
    config: leader_svg::RenderConfig,
) -> f32 {
    config.game_start()
        + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
        + f32::from(ordinal.min(63)) * 0.0015
}

fn norm(value: f32, total: f32) -> f32 {
    (value / total).clamp(0.0, 1.0)
}

fn sample_slice<T>(values: &[T], maximum: usize) -> Vec<&T> {
    if values.len() <= maximum {
        return values.iter().collect();
    }
    let stride = values.len().div_ceil(maximum);
    let mut sampled = values.iter().step_by(stride).collect::<Vec<_>>();
    if sampled.last().map(|value| *value as *const T)
        != values.last().map(|value| value as *const T)
    {
        if let Some(last) = values.last() {
            sampled.push(last);
        }
    }
    sampled
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::{build_topology, Machine};
    use leader_svg::RenderConfig;

    #[test]
    fn bitfabric_adds_all_memory_bit_sites_without_dom_explosion() {
        let topology = build_topology();
        let trace = Machine::run_match("v2-bitfabric", 5000);
        let source = crate::frontpage::physical_die::render(&topology, &trace, RenderConfig::default());
        let output = apply(source, &topology, &trace);
        assert!(output.contains("id=\"v2-memory-bitcell-fabric\""));
        assert_eq!(output.matches("data-bitcell-sites=\"2048\"").count(), 136);
        assert!(output.contains("aria-label=\"278528 physical memory bit-cell sites\""));
        assert!(!output.contains("opacity=\".08\" opacity="));
    }

    #[test]
    fn microcode_fabric_is_256_by_24_and_uses_native_control_words() {
        let topology = build_topology();
        let trace = Machine::run_match("v2-microcode-fabric", 5000);
        let source = crate::frontpage::physical_die::render(&topology, &trace, RenderConfig::default());
        let output = apply(source, &topology, &trace);
        assert!(output.contains("data-microcode-rows=\"256\""));
        assert!(output.contains("data-microcode-bits=\"24\""));
        assert!(output.contains("data-microcode-cells=\"6144\""));
        assert!(output.contains("data-ucontrol=\""));
    }
}
