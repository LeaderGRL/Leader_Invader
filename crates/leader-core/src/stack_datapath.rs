use crate::logic::{ripple_decrement16, ripple_increment16, Decrement16Trace, PcIncrementTrace};
use crate::trace::{MatchTrace, PhaseKind};

const STACK_WINDOW_START: u16 = 0x7F00;
const STACK_WINDOW_END: u16 = 0x7FFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackDatapathKind {
    Push(Decrement16Trace),
    Pop(PcIncrementTrace),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackDatapathEvent {
    pub frame: u32,
    pub ordinal: u16,
    pub pc: u16,
    pub address: u16,
    pub data: u8,
    pub kind: StackDatapathKind,
}

#[must_use]
pub fn derive_stack_datapath(trace: &MatchTrace) -> Vec<StackDatapathEvent> {
    trace
        .micro_samples
        .iter()
        .filter_map(|sample| {
            let address = sample.address?;
            let data = sample.data?;
            if !(STACK_WINDOW_START..=STACK_WINDOW_END).contains(&address) {
                return None;
            }

            let kind = match sample.phase {
                PhaseKind::MemoryWrite if sample.control == "CPU_WRITE" => {
                    let before = address.wrapping_add(1);
                    let step = ripple_decrement16(before);
                    debug_assert_eq!(step.after, address);
                    StackDatapathKind::Push(step)
                }
                PhaseKind::MemoryRead if sample.control == "CPU_READ" => {
                    let step = ripple_increment16(address);
                    StackDatapathKind::Pop(step)
                }
                _ => return None,
            };

            Some(StackDatapathEvent {
                frame: sample.frame,
                ordinal: sample.ordinal,
                pc: sample.pc,
                address,
                data,
                kind,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Machine;

    #[test]
    fn real_match_contains_balanced_call_return_stack_activity() {
        let trace = Machine::run_match("f3-stack", 5000);
        let events = derive_stack_datapath(&trace);
        let pushes = events
            .iter()
            .filter(|event| matches!(event.kind, StackDatapathKind::Push(_)))
            .count();
        let pops = events
            .iter()
            .filter(|event| matches!(event.kind, StackDatapathKind::Pop(_)))
            .count();
        assert!(pushes > 0);
        assert_eq!(pushes, pops);
    }

    #[test]
    fn push_and_pop_steps_are_bit_accurate() {
        let trace = Machine::run_match("f3-stack-bits", 5000);
        let events = derive_stack_datapath(&trace);
        for event in events {
            match event.kind {
                StackDatapathKind::Push(step) => {
                    assert_eq!(step.after, step.before.wrapping_sub(1));
                    assert_eq!(step.after, event.address);
                }
                StackDatapathKind::Pop(step) => {
                    assert_eq!(step.after, step.before.wrapping_add(1));
                    assert_eq!(step.before, event.address);
                }
            }
        }
    }
}
