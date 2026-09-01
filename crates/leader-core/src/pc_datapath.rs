use crate::isa::PcSource;
use crate::logic::{ripple_increment16, PcIncrementTrace};
use crate::trace::{MatchTrace, PcEventKind, PhaseKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcDatapathKind {
    Increment(PcIncrementTrace),
    Load {
        before: u16,
        after: u16,
        source: PcSource,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcDatapathEvent {
    pub frame: u32,
    pub ordinal: u16,
    pub kind: PcDatapathKind,
}

#[derive(Debug, Clone, Copy)]
struct PendingLoad {
    frame: u32,
    ordinal: u16,
    sequential: u16,
    source: PcSource,
    conditional: bool,
}

/// Builds the physical PC timeline.
///
/// Native `PcEvent` entries are emitted at the real ripple-increment and PC-load
/// boundaries and are authoritative. The semantic reconstruction below is kept
/// only for historical traces produced before native PC tracing existed.
#[must_use]
pub fn derive_pc_datapath(trace: &MatchTrace) -> Vec<PcDatapathEvent> {
    if !trace.pc_events.is_empty() {
        return trace
            .pc_events
            .iter()
            .map(|event| PcDatapathEvent {
                frame: event.frame,
                ordinal: event.ordinal,
                kind: match event.kind {
                    PcEventKind::Increment(increment) => PcDatapathKind::Increment(increment),
                    PcEventKind::Load {
                        before,
                        after,
                        source,
                        ..
                    } => PcDatapathKind::Load {
                        before,
                        after,
                        source,
                    },
                },
            })
            .collect();
    }

    derive_legacy_pc_datapath(trace)
}

fn derive_legacy_pc_datapath(trace: &MatchTrace) -> Vec<PcDatapathEvent> {
    let mut events = Vec::new();
    let mut last_fetch = None::<u16>;
    let mut pending = None::<PendingLoad>;

    for sample in &trace.micro_samples {
        if sample.phase == PhaseKind::Fetch {
            if let Some(load) = pending.take() {
                if !load.conditional || sample.pc != load.sequential {
                    events.push(PcDatapathEvent {
                        frame: load.frame,
                        ordinal: load.ordinal,
                        kind: PcDatapathKind::Load {
                            before: load.sequential,
                            after: sample.pc,
                            source: load.source,
                        },
                    });
                }
            }

            let increment = ripple_increment16(sample.pc);
            events.push(PcDatapathEvent {
                frame: sample.frame,
                ordinal: sample.ordinal,
                kind: PcDatapathKind::Increment(increment),
            });
            last_fetch = Some(sample.pc);
            continue;
        }

        if sample.phase != PhaseKind::Decode {
            continue;
        }

        let Some((source, conditional)) = pc_control(sample.control.as_str()) else {
            continue;
        };
        let sequential = ripple_increment16(last_fetch.unwrap_or(sample.pc)).after;
        pending = Some(PendingLoad {
            frame: sample.frame,
            ordinal: sample.ordinal.saturating_add(1),
            sequential,
            source,
            conditional,
        });
    }

    events.sort_by_key(|event| (event.frame, event.ordinal));
    events
}

fn pc_control(control: &str) -> Option<(PcSource, bool)> {
    match control {
        "JMP" => Some((PcSource::Jump, false)),
        "CALL" => Some((PcSource::Call, false)),
        "RET" => Some((PcSource::Return, false)),
        "JZ" | "JNZ" | "JLT" | "JGE" | "JC" => Some((PcSource::Branch, true)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Machine;

    #[test]
    fn real_match_contains_authoritative_pc_ripple_activity() {
        let trace = Machine::run_match("f3-pc", 5000);
        assert!(!trace.pc_events.is_empty());
        let events = derive_pc_datapath(&trace);
        let increments = events
            .iter()
            .filter_map(|event| match event.kind {
                PcDatapathKind::Increment(increment) => Some(increment),
                PcDatapathKind::Load { .. } => None,
            })
            .collect::<Vec<_>>();
        assert!(!increments.is_empty());
        assert!(increments
            .iter()
            .all(|event| event.after == event.before.wrapping_add(1)));
    }

    #[test]
    fn native_pc_stream_is_independent_from_semantic_samples() {
        let trace = Machine::run_match("f3-pc-native", 5000);
        let expected = derive_pc_datapath(&trace);
        assert!(!expected.is_empty());

        let mut without_samples = trace.clone();
        without_samples.micro_samples.clear();
        assert_eq!(derive_pc_datapath(&without_samples), expected);
    }

    #[test]
    fn real_match_uses_nonsequential_pc_mux_sources() {
        let trace = Machine::run_match("f3-pc-mux", 5000);
        let events = derive_pc_datapath(&trace);
        assert!(events.iter().any(|event| matches!(
            event.kind,
            PcDatapathKind::Load {
                source: PcSource::Call,
                ..
            }
        )));
        assert!(events.iter().any(|event| matches!(
            event.kind,
            PcDatapathKind::Load {
                source: PcSource::Return,
                ..
            }
        )));
    }

    #[test]
    fn legacy_pc_reconstruction_remains_available() {
        let mut trace = Machine::run_match("f3-pc-legacy", 5000);
        trace.pc_events.clear();
        let events = derive_pc_datapath(&trace);
        assert!(!events.is_empty());
        assert!(events
            .iter()
            .any(|event| matches!(event.kind, PcDatapathKind::Increment(_))));
    }
}
