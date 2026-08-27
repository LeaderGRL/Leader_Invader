use crate::isa::PcSource;
use crate::logic::{ripple_increment16, PcIncrementTrace};
use crate::trace::{MatchTrace, PhaseKind};

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

/// Builds the physical PC timeline from the real execution stream in O(n).
///
/// Every fetch is guaranteed to have been advanced semantically by `Cpu::next8`
/// through `ripple_increment16`. When a PC-control micro-op is encountered, the
/// next fetch resolves whether the input mux selected a non-sequential source.
#[must_use]
pub fn derive_pc_datapath(trace: &MatchTrace) -> Vec<PcDatapathEvent> {
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
}
