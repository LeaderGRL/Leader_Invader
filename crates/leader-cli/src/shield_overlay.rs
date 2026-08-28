use std::collections::BTreeSet;

use leader_core::{
    BusTransactionKind, MatchTrace, ShieldBank, Topology, SHIELD_BYTES_PER, SHIELD_COUNT, SHIELD_H,
    SHIELD_RAM_BASE, SHIELD_TOTAL_BYTES, SHIELD_W, SHIELD_X, SHIELD_Y,
};
use leader_svg::RenderConfig;

const MAX_SHIELD_EVENTS: usize = 64;
const SHIELD_PIXEL_COUNT: usize = SHIELD_COUNT * SHIELD_W * SHIELD_H;

#[derive(Debug, Clone, Copy)]
struct ShieldVisualEvent {
    frame: u32,
    ordinal: u16,
    shield: usize,
    byte: usize,
    mask: u8,
    before: u8,
    after: u8,
    source: &'static str,
}

#[must_use]
pub fn apply(
    mut svg: String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
) -> String {
    if trace.total_frames == 0 {
        return svg;
    }
    let overlay = render(topology, trace, config);
    let Some(svg_close) = svg.rfind("</svg>") else {
        return svg;
    };
    let Some(world_close) = svg[..svg_close].rfind("</g>") else {
        return svg;
    };
    svg.insert_str(world_close, &overlay);
    svg
}

fn render(topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    let events = derive_events(trace);
    let selected = selected_indices(&events);
    let total = config.total();
    let mut out = String::with_capacity(selected.len() * 1500 + 90_000);
    out.push_str("<g id=\"m3-shield-bank\">\n");

    render_game_shields(&mut out, topology, trace, config, &events);

    for index in selected {
        let event = events[index];
        let moment = trace_moment(event.frame, event.ordinal, trace, config);
        let k1 = norm(moment, total);
        let k2 = norm(moment + 0.020, total);
        let k3 = norm(moment + 0.145, total);
        out.push_str(&format!(
            "<g opacity=\"0\" data-shield-index=\"{}\" data-shield-byte=\"{}\" data-shield-mask=\"{:02X}\" data-shield-before=\"{:02X}\" data-shield-after=\"{:02X}\" data-shield-source=\"{}\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>",
            event.shield,
            event.byte,
            event.mask,
            event.before,
            event.after,
            event.source
        ));
        glow(topology, &mut out, "shieldAddr", "#f2ae4f");
        glow(topology, &mut out, "shieldMask", "#ef7caf");
        glow(topology, &mut out, "shieldWriteEnable", "#ff8065");
        glow(
            topology,
            &mut out,
            &format!("shieldRam{}", event.shield),
            "#6dcff6",
        );
        glow(topology, &mut out, "shieldVideoMux", "#72d4e7");
        out.push_str("</g>\n");
    }

    out.push_str("</g>\n");
    out
}

fn render_game_shields(
    out: &mut String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
    events: &[ShieldVisualEvent],
) {
    let Some(display) = topology.node("display") else {
        return;
    };
    let total = config.total();
    let sx = display.bounds.x + 53.0;
    let sy = display.bounds.y + 55.0;
    let scale = 2.42_f32;
    let show1 = norm(config.game_start(), total);
    let show2 = norm(config.game_start() + 0.2, total);
    let hide1 = norm(config.game_end(), total);
    let hide2 = norm(config.game_end() + 0.2, total);
    let damage = damage_times(events);
    let initial = ShieldBank::default();

    out.push_str(&format!(
        "<g id=\"m3-shield-game\" transform=\"translate({sx:.1} {sy:.1}) scale({scale:.3})\" opacity=\"0\"><animate attributeName=\"opacity\" values=\"0;0;1;1;0;0\" keyTimes=\"0;{show1:.6};{show2:.6};{hide1:.6};{hide2:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>"
    ));

    for shield in 0..SHIELD_COUNT {
        for local_y in 0..SHIELD_H {
            for local_x in 0..SHIELD_W {
                if !initial.pixel(shield, local_x, local_y) {
                    continue;
                }
                let index = pixel_index(shield, local_x, local_y);
                let x = SHIELD_X[shield] + local_x as i16;
                let y = SHIELD_Y + local_y as i16;
                out.push_str(&format!(
                    "<rect data-shield-game-pixel=\"{shield}:{local_x}:{local_y}\" x=\"{x}\" y=\"{y}\" width=\"1\" height=\"1\" fill=\"#b7ff72\""
                ));
                if let Some((frame, ordinal)) = damage[index] {
                    let moment = trace_moment(frame, ordinal, trace, config);
                    let k1 = norm(moment, total);
                    let k2 = norm(moment + 0.025, total);
                    out.push_str(&format!(
                        "><animate attributeName=\"opacity\" values=\"1;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/></rect>"
                    ));
                } else {
                    out.push_str("/>");
                }
            }
        }
    }
    out.push_str("</g>\n");
}

fn damage_times(events: &[ShieldVisualEvent]) -> [Option<(u32, u16)>; SHIELD_PIXEL_COUNT] {
    let mut times = [None; SHIELD_PIXEL_COUNT];
    for event in events {
        if event.mask.count_ones() != 1 {
            continue;
        }
        let row = event.byte / 2;
        let byte_col = event.byte % 2;
        let bit_in_byte = 7usize.saturating_sub(event.mask.trailing_zeros() as usize);
        let local_x = byte_col * 8 + bit_in_byte;
        if row < SHIELD_H && local_x < SHIELD_W {
            let index = pixel_index(event.shield, local_x, row);
            times[index].get_or_insert((event.frame, event.ordinal));
        }
    }
    times
}

const fn pixel_index(shield: usize, local_x: usize, local_y: usize) -> usize {
    shield * SHIELD_W * SHIELD_H + local_y * SHIELD_W + local_x
}

fn derive_events(trace: &MatchTrace) -> Vec<ShieldVisualEvent> {
    let mut model = *ShieldBank::default().bytes();
    let mut events = Vec::new();
    for transaction in trace.bus_transactions.iter().filter(|transaction| {
        transaction.kind == BusTransactionKind::Write
            && transaction.address.is_some_and(|address| {
                (SHIELD_RAM_BASE..SHIELD_RAM_BASE + SHIELD_TOTAL_BYTES as u16).contains(&address)
            })
    }) {
        let Some(address) = transaction.address else {
            continue;
        };
        let Some(after) = transaction.data else {
            continue;
        };
        let index = usize::from(address - SHIELD_RAM_BASE);
        let before = model[index];
        let mask = before ^ after;
        let source = match transaction.control {
            "SHIELD_DAMAGE_PLAYER" => "player",
            "SHIELD_DAMAGE_ENEMY" => "enemy",
            _ => "invalid",
        };
        events.push(ShieldVisualEvent {
            frame: transaction.frame,
            ordinal: transaction.ordinal,
            shield: index / SHIELD_BYTES_PER,
            byte: index % SHIELD_BYTES_PER,
            mask,
            before,
            after,
            source,
        });
        model[index] = after;
    }
    events
}

fn selected_indices(events: &[ShieldVisualEvent]) -> Vec<usize> {
    if events.len() <= MAX_SHIELD_EVENTS {
        return (0..events.len()).collect();
    }

    let mut selected = BTreeSet::new();
    selected.insert(0usize);
    selected.insert(events.len() - 1);

    for shield in 0..SHIELD_COUNT {
        if let Some(index) = events.iter().position(|event| event.shield == shield) {
            selected.insert(index);
        }
    }
    for source in ["player", "enemy"] {
        if let Some(index) = events.iter().position(|event| event.source == source) {
            selected.insert(index);
        }
    }

    let remaining = MAX_SHIELD_EVENTS.saturating_sub(selected.len());
    if remaining > 0 {
        let stride = events.len().div_ceil(remaining).max(1);
        for index in (0..events.len()).step_by(stride) {
            if selected.len() >= MAX_SHIELD_EVENTS {
                break;
            }
            selected.insert(index);
        }
    }

    selected.into_iter().take(MAX_SHIELD_EVENTS).collect()
}

fn glow(topology: &Topology, out: &mut String, id: &str, color: &str) {
    let Some(node) = topology.node(id) else {
        return;
    };
    let b = node.bounds;
    out.push_str(&format!(
        "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"7\" fill=\"{color}\" fill-opacity=\".22\" stroke=\"{color}\" stroke-width=\"7\" filter=\"url(#glow)\"/>",
        b.x - 3.0,
        b.y - 3.0,
        b.w + 6.0,
        b.h + 6.0
    ));
}

fn trace_moment(frame: u32, ordinal: u16, trace: &MatchTrace, config: RenderConfig) -> f32 {
    config.game_start()
        + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
        + f32::from(ordinal.min(15)) * 0.0035
}

fn norm(value: f32, total: f32) -> f32 {
    (value / total).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::{build_topology, Machine};

    #[test]
    fn shield_overlay_exposes_exact_one_bit_mutations_and_crt_pixels() {
        let topology = build_topology();
        let trace = Machine::run_match("m3-shield-overlay", 5000);
        let svg = render(&topology, &trace, RenderConfig::default());
        assert!(svg.contains("id=\"m3-shield-bank\""));
        assert!(svg.contains("id=\"m3-shield-game\""));
        assert!(svg.contains("data-shield-game-pixel=\""));
        assert!(svg.contains("data-shield-mask=\""));
        assert!(svg.contains("data-shield-before=\""));
        assert!(svg.contains("data-shield-after=\""));
        assert!(svg.contains("data-shield-source=\"player\"")
            || svg.contains("data-shield-source=\"enemy\""));
    }

    #[test]
    fn shield_damage_times_map_one_hot_masks_to_exact_pixels() {
        let trace = Machine::run_match("m3-shield-pixel-map", 5000);
        let events = derive_events(&trace);
        let times = damage_times(&events);
        assert_eq!(times.iter().flatten().count(), events.len());
    }

    #[test]
    fn shield_sampling_is_strictly_bounded_and_keeps_available_sources() {
        let trace = Machine::run_match("m3-shield-sampling", 5000);
        let events = derive_events(&trace);
        let selected = selected_indices(&events);
        assert!(selected.len() <= MAX_SHIELD_EVENTS);
        for source in ["player", "enemy"] {
            if events.iter().any(|event| event.source == source) {
                assert!(selected.iter().any(|index| events[*index].source == source));
            }
        }
        for shield in 0..SHIELD_COUNT {
            if events.iter().any(|event| event.shield == shield) {
                assert!(selected.iter().any(|index| events[*index].shield == shield));
            }
        }
    }
}
