use std::fmt::Write as _;

use leader_core::{
    resolve_physical_memory_byte, BusTransactionEvent, MatchTrace, MemoryOwner, Rect, Topology,
};
use leader_svg::RenderConfig;

const ROM_FOCUS_TIME: f32 = 17.4;
const RAM_FOCUS_TIME: f32 = 25.5;
const VRAM_FOCUS_TIME: f32 = 5.3;

#[derive(Debug, Clone, Copy)]
struct FocusSpec {
    owner: MemoryOwner,
    desired_time: f32,
}

/// Guarantees one exact native byte access for each critical memory subsystem.
/// The general renderer remains sparsely sampled for readability, while these
/// probes select first-class bus transactions from the complete trace near the
/// technical camera scenes. No address, byte value or bit state is synthesized.
#[must_use]
pub fn apply(mut svg: String, topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    if !svg.contains("data-frontpage-version=\"physical-die-v2\"") || trace.total_frames == 0 {
        return svg;
    }

    let specs = [
        FocusSpec {
            owner: MemoryOwner::Vram,
            desired_time: VRAM_FOCUS_TIME,
        },
        FocusSpec {
            owner: MemoryOwner::Rom,
            desired_time: ROM_FOCUS_TIME,
        },
        FocusSpec {
            owner: MemoryOwner::Ram,
            desired_time: RAM_FOCUS_TIME,
        },
    ];

    let mut probes = String::with_capacity(4_000);
    for spec in specs {
        let Some(event) = select_event(trace, config, spec) else {
            continue;
        };
        let Some(address) = event.address else {
            continue;
        };
        let Some(byte) = resolve_physical_memory_byte(address, event.data.unwrap_or(0)) else {
            continue;
        };
        let Some(prefix) = owner_prefix(byte.address.owner) else {
            continue;
        };
        let Some(page) = topology.node(&format!("{prefix}{}", byte.address.page)) else {
            continue;
        };

        let cell = byte_cell_rect(page.bounds, byte.address.row, byte.address.column);
        let moment = trace_moment(event.frame, event.ordinal, trace, config) + 0.15;
        let total = config.total();
        let k1 = norm(moment, total);
        let k2 = norm(moment + 0.018, total).max(k1 + 0.000_01);
        let k3 = norm(moment + 0.34, total).max(k2 + 0.000_01);
        let bits = bits_string(byte.bits_lsb_first);
        let _ = writeln!(
            probes,
            r##"<g opacity="0" data-memory-owner="{}" data-memory-address="{address:04X}" data-memory-page="{}" data-memory-byte="{}" data-memory-bits="{bits}" data-dedicated-memory="true" data-source-frame="{}" data-source-ordinal="{}"><animate attributeName="opacity" values="0;0;1;0;0" keyTimes="0;{k1:.6};{k2:.6};{k3:.6};1" dur="{total:.3}s" repeatCount="indefinite"/><rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="#ffffff" stroke="#ffffff" stroke-width="2.2" vector-effect="non-scaling-stroke" filter="url(#v2-hot)"/><rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="8" fill="none" stroke="#ffffff" stroke-width="3" vector-effect="non-scaling-stroke" opacity=".42"/></g>"##,
            owner_name(byte.address.owner),
            byte.address.page,
            byte.address.byte,
            event.frame,
            event.ordinal,
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

    const MEMORY_GROUP: &str = "<g id=\"v2-exact-memory-cell-activity\">\n";
    if let Some(start) = svg.find(MEMORY_GROUP) {
        svg.insert_str(start + MEMORY_GROUP.len(), &probes);
    }
    svg
}

fn select_event(trace: &MatchTrace, config: RenderConfig, spec: FocusSpec) -> Option<&BusTransactionEvent> {
    trace
        .bus_transactions
        .iter()
        .filter(|event| {
            let (Some(address), Some(_)) = (event.address, event.data) else {
                return false;
            };
            resolve_physical_memory_byte(address, event.data.unwrap_or(0))
                .is_some_and(|byte| byte.address.owner == spec.owner)
        })
        .min_by(|left, right| {
            let left_time = trace_moment(left.frame, left.ordinal, trace, config) + 0.17;
            let right_time = trace_moment(right.frame, right.ordinal, trace, config) + 0.17;
            (left_time - spec.desired_time)
                .abs()
                .total_cmp(&(right_time - spec.desired_time).abs())
        })
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

fn owner_prefix(owner: MemoryOwner) -> Option<&'static str> {
    match owner {
        MemoryOwner::Rom => Some("romPage"),
        MemoryOwner::Ram => Some("ramPage"),
        MemoryOwner::Vram => Some("vramPage"),
        MemoryOwner::Mmio | MemoryOwner::Unmapped => None,
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

fn bits_string(bits: [bool; 8]) -> String {
    bits.iter()
        .rev()
        .map(|value| if *value { '1' } else { '0' })
        .collect()
}

fn trace_moment(frame: u32, ordinal: u16, trace: &MatchTrace, config: RenderConfig) -> f32 {
    config.game_start()
        + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
        + f32::from(ordinal.min(63)) * 0.0015
}

fn norm(value: f32, total: f32) -> f32 {
    (value / total.max(0.001)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::{build_topology, Machine};

    #[test]
    fn dedicated_probes_cover_all_three_physical_memory_owners() {
        let topology = build_topology();
        let trace = Machine::run_match("dedicated-memory-focus", 5000);
        let source = String::from("<svg data-frontpage-version=\"physical-die-v2\"><g id=\"v2-exact-memory-cell-activity\">\n</g></svg>");
        let output = apply(source, &topology, &trace, crate::frontpage::render_config());
        assert!(output.contains("data-memory-owner=\"rom\""));
        assert!(output.contains("data-memory-owner=\"ram\""));
        assert!(output.contains("data-memory-owner=\"vram\""));
        assert_eq!(output.matches("data-dedicated-memory=\"true\"").count(), 3);
    }
}
