use leader_core::{FormationCadenceTraceEvent, MatchTrace, Topology};
use leader_svg::RenderConfig;

const MAX_PRESENTED_EVENTS: usize = 80;

#[must_use]
pub fn apply(
    mut svg: String,
    topology: &Topology,
    trace: &MatchTrace,
    config: RenderConfig,
) -> String {
    if trace.formation_cadence_events.is_empty() || trace.total_frames == 0 {
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
    let total = config.total();
    let events = sampled_events(&trace.formation_cadence_events);
    let mut out = String::with_capacity(events.len() * 900);
    out.push_str("<g id=\"m3-formation-cadence\">\n");

    for trace_event in events {
        let event = trace_event.event;
        let moment = trace_moment(trace_event.frame, trace_event.ordinal, trace, config) + 0.012;
        let k1 = norm(moment, total);
        let k2 = norm(moment + 0.020, total);
        let k3 = norm(moment + 0.145, total);
        out.push_str(&format!(
            "<g opacity=\"0\" data-cadence-alive=\"{}\" data-cadence-divisor=\"{}\" data-cadence-counter=\"{}:{}\" data-cadence-tick=\"{}\"><animate attributeName=\"opacity\" values=\"0;0;1;0;0\" keyTimes=\"0;{k1:.6};{k2:.6};{k3:.6};1\" dur=\"{total:.3}s\" repeatCount=\"indefinite\"/>",
            event.alive,
            event.divisor,
            event.before,
            event.after,
            u8::from(event.tick)
        ));
        glow(topology, &mut out, "formationAlive", "#6dcff6");
        glow(topology, &mut out, "formationDivider", "#e8e677");
        glow(topology, &mut out, "formationCounter", "#67d9b3");
        if event.tick {
            glow(topology, &mut out, "formationTick", "#ffb45b");
        }
        out.push_str("</g>\n");
    }

    out.push_str("</g>\n");
    out
}

fn sampled_events(events: &[FormationCadenceTraceEvent]) -> Vec<&FormationCadenceTraceEvent> {
    if events.len() <= MAX_PRESENTED_EVENTS {
        return events.iter().collect();
    }

    let stride = events.len().div_ceil(MAX_PRESENTED_EVENTS.saturating_sub(8).max(1));
    let mut out = Vec::with_capacity(MAX_PRESENTED_EVENTS + 8);
    let mut tick_seen = [false; 4];

    for (index, event) in events.iter().enumerate() {
        let divisor = usize::from(event.event.divisor.min(3));
        let speed_transition = index > 0 && events[index - 1].event.divisor != event.event.divisor;
        let first_tick_for_divisor = event.event.tick && !tick_seen[divisor];
        if first_tick_for_divisor {
            tick_seen[divisor] = true;
        }
        if index == 0
            || index + 1 == events.len()
            || index % stride == 0
            || speed_transition
            || first_tick_for_divisor
        {
            if out.last().is_none_or(|last: &&FormationCadenceTraceEvent| {
                !std::ptr::eq(*last, event)
            }) {
                out.push(event);
            }
        }
    }
    out
}

fn glow(topology: &Topology, out: &mut String, id: &str, color: &str) {
    let Some(node) = topology.node(id) else {
        return;
    };
    let b = node.bounds;
    out.push_str(&format!(
        "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"7\" fill=\"{color}\" fill-opacity=\".24\" stroke=\"{color}\" stroke-width=\"7\" filter=\"url(#glow)\"/>",
        b.x - 3.0,
        b.y - 3.0,
        b.w + 6.0,
        b.h + 6.0
    ));
}

fn trace_moment(frame: u32, ordinal: u16, trace: &MatchTrace, config: RenderConfig) -> f32 {
    config.game_start()
        + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
        + f32::from(ordinal.min(63)) * 0.0018
}

fn norm(value: f32, total: f32) -> f32 {
    (value / total).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::{build_topology, Machine};

    #[test]
    fn cadence_overlay_preserves_all_speed_bands_and_tick_states() {
        let topology = build_topology();
        let trace = Machine::run_match("m3-cadence-overlay", 5000);
        let svg = render(&topology, &trace, RenderConfig::default());
        assert!(svg.contains("id=\"m3-formation-cadence\""));
        assert!(svg.contains("data-cadence-divisor=\"3\""));
        assert!(svg.contains("data-cadence-divisor=\"2\""));
        assert!(svg.contains("data-cadence-divisor=\"1\""));
        assert!(svg.contains("data-cadence-tick=\"1\""));
        assert!(svg.contains("data-cadence-tick=\"0\""));
    }

    #[test]
    fn cadence_sampling_is_bounded_but_keeps_transitions() {
        let trace = Machine::run_match("m3-cadence-sampling", 5000);
        let sampled = sampled_events(&trace.formation_cadence_events);
        assert!(sampled.len() <= MAX_PRESENTED_EVENTS + 8);
        for divisor in [3, 2, 1] {
            assert!(sampled.iter().any(|event| event.event.divisor == divisor));
            assert!(sampled
                .iter()
                .any(|event| event.event.divisor == divisor && event.event.tick));
        }
    }
}
