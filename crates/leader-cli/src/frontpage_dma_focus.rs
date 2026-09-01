use std::fmt::Write as _;

use leader_core::{physical_bus_link_values, BusTransactionEvent, BusTransactionKind, MatchTrace, SignalKind, Topology};
use leader_svg::RenderConfig;

const DESIRED_DMA_TIME: f32 = 21.8;

/// Guarantees one renderer-visible native DMA transaction for the technical
/// GPU close-up. Generic presentation sampling is intentionally allowed to be
/// sparse; this probe selects one first-class DMA event from the complete trace
/// and renders its exact core-owned physical propagation path inside the same
/// native bus layer used by all other electrical activity.
#[must_use]
pub fn apply(mut svg: String, topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    if !svg.contains("data-frontpage-version=\"physical-die-v2\"") || trace.total_frames == 0 {
        return svg;
    }
    let Some(event) = select_dma_event(trace, config) else {
        return svg;
    };
    let values = physical_bus_link_values(topology, event);
    if values.is_empty() {
        return svg;
    }

    let moment = trace_moment(event.frame, event.ordinal, trace, config);
    let total = config.total();
    let mut probe = String::with_capacity(values.len() * 420 + 300);
    let _ = writeln!(
        probe,
        r##"<g id="v2-dedicated-dma-focus" data-source="native-dma" data-frame="{}" data-ordinal="{}">"##,
        event.frame, event.ordinal,
    );
    for value in values {
        let start = moment + f32::from(value.rank) * 0.032;
        let end = start + 0.18;
        let k1 = norm(start, total);
        let k2 = norm(start + 0.018, total).max(k1 + 0.000_01);
        let k3 = norm(end, total).max(k2 + 0.000_01);
        let _ = writeln!(
            probe,
            r##"<use href="#v2-wire-{}" class="v2-active-wire {}" opacity="0" data-rank="{}" data-stage="{}" data-value="{}" data-width="{}" data-dedicated-dma="true"><animate attributeName="opacity" values="0;0;1;0;0" keyTimes="0;{k1:.6};{k2:.6};{k3:.6};1" dur="{total:.3}s" repeatCount="indefinite"/></use>"##,
            xml_escape(&value.link_id),
            signal_class(value.signal),
            value.rank,
            xml_escape(value.stage),
            value.value,
            value.width,
        );
    }
    probe.push_str("</g>\n");

    const BUS_GROUP: &str = "<g id=\"v2-native-bus-propagation\">\n";
    if let Some(start) = svg.find(BUS_GROUP) {
        let content_start = start + BUS_GROUP.len();
        if let Some(relative_end) = svg[content_start..].find("</g>\n") {
            svg.insert_str(content_start + relative_end, &probe);
        }
    }
    svg
}

fn select_dma_event(trace: &MatchTrace, config: RenderConfig) -> Option<&BusTransactionEvent> {
    trace
        .bus_transactions
        .iter()
        .filter(|event| event.kind == BusTransactionKind::Dma && event.address.is_some())
        .min_by(|left, right| {
            let left_time = trace_moment(left.frame, left.ordinal, trace, config);
            let right_time = trace_moment(right.frame, right.ordinal, trace, config);
            (left_time - DESIRED_DMA_TIME)
                .abs()
                .total_cmp(&(right_time - DESIRED_DMA_TIME).abs())
        })
}

fn trace_moment(frame: u32, ordinal: u16, trace: &MatchTrace, config: RenderConfig) -> f32 {
    config.game_start()
        + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
        + f32::from(ordinal.min(63)) * 0.0015
}

fn norm(value: f32, total: f32) -> f32 {
    (value / total.max(0.001)).clamp(0.0, 1.0)
}

fn signal_class(signal: SignalKind) -> &'static str {
    match signal {
        SignalKind::Address => "v2-active-address",
        SignalKind::Data => "v2-active-data",
        SignalKind::Control => "v2-active-control",
        SignalKind::Clock => "v2-active-clock",
        SignalKind::Carry => "v2-active-carry",
        SignalKind::Video => "v2-active-video",
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
    fn dedicated_probe_always_exposes_a_real_dma_latch_stage() {
        let topology = build_topology();
        let trace = Machine::run_match("dedicated-dma-focus", 5000);
        let source = String::from("<svg data-frontpage-version=\"physical-die-v2\"><g id=\"v2-native-bus-propagation\">\n</g>\n</svg>");
        let output = apply(source, &topology, &trace, crate::frontpage::render_config());
        assert!(output.contains("id=\"v2-dedicated-dma-focus\""));
        assert!(output.contains("data-source=\"native-dma\""));
        assert!(output.contains("data-stage=\"dma_data_latch\""));
        assert!(output.contains("data-dedicated-dma=\"true\""));
    }
}
