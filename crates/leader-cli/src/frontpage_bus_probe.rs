use std::fmt::Write as _;

use leader_core::{BusTransactionEvent, MatchTrace};
use leader_svg::RenderConfig;

const MAX_VISIBLE_BUS_EVENTS: usize = 96;
const MAX_HOLD_SECONDS: f32 = 0.28;

/// Adds a fixed logic-analyzer readout for native bus transactions. The panel
/// never reconstructs state: every visible value comes from one first-class
/// `BusTransactionEvent`, and opacity returns to zero shortly after that exact
/// event instead of holding stale bus data across unrelated microcycles.
#[must_use]
pub fn apply(mut svg: String, trace: &MatchTrace, config: RenderConfig) -> String {
    if !svg.contains("data-frontpage-version=\"physical-die-v2\"") || trace.total_frames == 0 {
        return svg;
    }

    let candidates = trace
        .bus_transactions
        .iter()
        .filter(|event| event.address.is_some())
        .collect::<Vec<_>>();
    let sampled = sample_refs(&candidates, MAX_VISIBLE_BUS_EVENTS);
    if sampled.is_empty() {
        return svg;
    }

    let total = config.total();
    let mut analyzer = String::with_capacity(sampled.len() * 560 + 1_000);
    analyzer.push_str(
        r##"<g id="v2-native-bus-analyzer" data-source="native-bus-transactions">
<rect x="205" y="618" width="790" height="23" rx="5" fill="#050d15" stroke="#284457"/>
<text x="216" y="633" fill="#536d7d" font-size="7.5" font-weight="900">BUS ANALYZER</text>
"##,
    );

    for event in sampled {
        let start = trace_moment(event.frame, event.ordinal, trace, config);
        let end = (start + MAX_HOLD_SECONDS).min(config.game_end());
        if end <= start {
            continue;
        }
        let k1 = normalized(start, total);
        let k2 = normalized(start + 0.008, total).max(k1 + 0.000_01);
        let k3 = normalized(end, total).max(k2 + 0.000_01);
        let address = event.address.map_or_else(|| "----".to_owned(), |value| format!("{value:04X}"));
        let data = event.data.map_or_else(|| "--".to_owned(), |value| format!("{value:02X}"));
        let _ = writeln!(
            analyzer,
            r##"<g opacity="0" data-bus-kind="{}" data-frame="{}" data-ordinal="{}"><animate attributeName="opacity" values="0;0;1;0;0" keyTimes="0;{k1:.7};{k2:.7};{k3:.7};1" dur="{total:.3}s" repeatCount="indefinite"/><text x="300" y="633" fill="#ffbe64" font-size="8" font-weight="900">{} · A {address}</text><text x="470" y="633" fill="#55d9ff" font-size="8" font-weight="900">D {data}</text><text x="545" y="633" fill="#86a8bc" font-size="7.5" font-weight="900">ADDR {}</text><text x="725" y="633" fill="#ff8bd6" font-size="7.5" font-weight="900">DATA {}</text><text x="925" y="633" text-anchor="end" fill="#718795" font-size="7">PC {:04X}</text></g>"##,
            event.kind.as_str(),
            event.frame,
            event.ordinal,
            event.kind.as_str().to_ascii_uppercase(),
            event.address_source.as_str(),
            event.data_source.as_str(),
            event.pc,
        );
    }
    analyzer.push_str("</g>\n");

    if let Some(index) = svg.rfind("</svg>") {
        svg.insert_str(index, &analyzer);
    }
    svg
}

fn trace_moment(
    frame: u32,
    ordinal: u16,
    trace: &MatchTrace,
    config: RenderConfig,
) -> f32 {
    config.game_start()
        + frame as f32 / trace.total_frames.max(1) as f32 * config.game_seconds
        + f32::from(ordinal.min(63)) * 0.0015
}

fn normalized(time: f32, total: f32) -> f32 {
    (time / total.max(0.001)).clamp(0.0, 1.0)
}

fn sample_refs<'a, T>(values: &[&'a T], maximum: usize) -> Vec<&'a T> {
    if values.len() <= maximum {
        return values.to_vec();
    }
    let stride = values.len().div_ceil(maximum);
    let mut sampled = values.iter().step_by(stride).copied().collect::<Vec<_>>();
    if sampled.last().copied().map(std::ptr::from_ref)
        != values.last().copied().map(std::ptr::from_ref)
    {
        if let Some(last) = values.last() {
            sampled.push(*last);
        }
    }
    sampled
}

#[cfg(test)]
mod tests {
    use super::*;
    use leader_core::Machine;

    #[test]
    fn analyzer_exposes_native_address_data_and_sources() {
        let trace = Machine::run_match("frontpage-bus-analyzer", 5000);
        let source = String::from("<svg data-frontpage-version=\"physical-die-v2\"></svg>");
        let output = apply(source, &trace, crate::frontpage::render_config());
        assert!(output.contains("id=\"v2-native-bus-analyzer\""));
        assert!(output.contains("data-source=\"native-bus-transactions\""));
        assert!(output.contains("ADDR program_counter") || output.contains("ADDR cpu"));
        assert!(output.contains("DATA rom") || output.contains("DATA ram") || output.contains("DATA vram"));
    }

    #[test]
    fn analyzer_does_not_hold_stale_values_for_whole_sample_intervals() {
        let trace = Machine::run_match("frontpage-bus-short-pulse", 5000);
        let source = String::from("<svg data-frontpage-version=\"physical-die-v2\"></svg>");
        let output = apply(source, &trace, crate::frontpage::render_config());
        assert!(output.contains("values=\"0;0;1;0;0\""));
        assert!(!output.contains("<script"));
    }
}
