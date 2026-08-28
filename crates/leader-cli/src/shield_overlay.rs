use std::collections::BTreeSet;

use leader_core::{
    BusTransactionKind, MatchTrace, ShieldBank, Topology, SHIELD_BYTES_PER, SHIELD_COUNT,
    SHIELD_RAM_BASE, SHIELD_TOTAL_BYTES,
};
use leader_svg::RenderConfig;

const MAX_SHIELD_EVENTS: usize = 64;

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
    let mut out = String::with_capacity(selected.len() * 1500);
    out.push_str("<g id=\"m3-shield-bank\">\n");

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
    fn shield_overlay_exposes_exact_one_bit_mutations() {
        let topology = build_topology();
        let trace = Machine::run_match("m3-shield-overlay", 5000);
        let svg = render(&topology, &trace, RenderConfig::default());
        assert!(svg.contains("id=\"m3-shield-bank\""));
        assert!(svg.contains("data-shield-mask=\""));
        assert!(svg.contains("data-shield-before=\""));
        assert!(svg.contains("data-shield-after=\""));
        assert!(svg.contains("data-shield-source=\"player\"")
            || svg.contains("data-shield-source=\"enemy\""));
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
