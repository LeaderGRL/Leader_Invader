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

/// Builds the physical PC timeline from the real execution stream.
///
/// Every fetch is guaranteed to have been advanced by `Cpu::next8` through
/// `ripple_increment16`. Non-sequential mux selections are detected at instruction
/// boundaries by comparing the sequential PC after operand fetches with the next
/// instruction fetch address.
#[must_use]
pub fn derive_pc_datapath(trace: &MatchTrace) -> Vec<PcDatapathEvent> {
    let samples = &trace.micro_samples;
    let mut events = Vec::new();

    for sample in samples.iter().filter(|sample| sample.phase == PhaseKind::Fetch) {
        events.push(PcDatapathEvent {
            frame: sample.frame,
            ordinal: sample.ordinal,
            kind: PcDatapathKind::Increment(ripple_increment16(sample.pc)),
        });
    }

    for (index, sample) in samples.iter().enumerate() {
        if sample.phase != PhaseKind::Decode || !is_pc_control(sample.control.as_str()) {
            continue;
        }

        let Some(next_fetch) = samples[index + 1..]
            .iter()
            .find(|candidate| candidate.phase == PhaseKind::Fetch)
        else {
            continue;
        };

        let instruction_start = sample.pc;
        let mut last_fetch = instruction_start;
        for candidate in samples[..index].iter().rev() {
            if candidate.phase == PhaseKind::Fetch {
                last_fetch = candidate.pc;
                if candidate.pc == instruction_start {
                    break;
                }
            }
        }
        let sequential = ripple_increment16(last_fetch).after;
        let source = match sample.control.as_str() {
            "JMP" => Some(PcSource::Jump),
            "CALL" => Some(PcSource::Call),
            "RET" => Some(PcSource::Return),
            "JZ" | "JNZ" | "JLT" | "JGE" | "JC" if next_fetch.pc != sequential => {
                Some(PcSource::Branch)
            }
            _ => None,
        };

        if let Some(source) = source {
            events.push(PcDatapathEvent {
                frame: sample.frame,
                ordinal: sample.ordinal.saturating_add(1),
                kind: PcDatapathKind::Load {
                    before: sequential,
                    after: next_fetch.pc,
                    source,
                },
            });
        }
    }

    events.sort_by_key(|event| (event.frame, event.ordinal));
    events
}

fn is_pc_control(control: &str) -> bool {
    matches!(control, "JMP" | "JZ" | "JNZ" | "JLT" | "JGE" | "JC" | "CALL" | "RET")
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
