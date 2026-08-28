use crate::logic::{ripple_decrement16, ripple_increment16, Decrement16Trace, PcIncrementTrace};
use crate::trace::{BusTransactionKind, MatchTrace, PhaseKind, SpEventKind};

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
    if !trace.sp_events.is_empty() {
        return trace
            .sp_events
            .iter()
            .map(|event| StackDatapathEvent {
                frame: event.frame,
                ordinal: event.ordinal,
                pc: event.pc,
                address: event.address,
                data: event.data,
                kind: match event.kind {
                    SpEventKind::Push(step) => StackDatapathKind::Push(step),
                    SpEventKind::Pop(step) => StackDatapathKind::Pop(step),
                },
            })
            .collect();
    }

    if !trace.bus_transactions.is_empty() {
        return derive_bus_stack_datapath(trace);
    }

    derive_legacy_stack_datapath(trace)
}

fn derive_bus_stack_datapath(trace: &MatchTrace) -> Vec<StackDatapathEvent> {
    trace
        .bus_transactions
        .iter()
        .filter_map(|transaction| {
            let address = transaction.address?;
            let data = transaction.data?;
            if !(STACK_WINDOW_START..=STACK_WINDOW_END).contains(&address) {
                return None;
            }

            let kind = match transaction.kind {
                BusTransactionKind::Write => {
                    let before = address.wrapping_add(1);
                    let step = ripple_decrement16(before);
                    debug_assert_eq!(step.after, address);
                    StackDatapathKind::Push(step)
                }
                BusTransactionKind::Read => {
                    let step = ripple_increment16(address);
                    StackDatapathKind::Pop(step)
                }
                _ => return None,
            };

            Some(StackDatapathEvent {
                frame: transaction.frame,
                ordinal: transaction.ordinal,
                pc: transaction.pc,
                address,
                data,
                kind,
            })
        })
        .collect()
}

fn derive_legacy_stack_datapath(trace: &MatchTrace) -> Vec<StackDatapathEvent> {
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
    fn first_class_sp_stream_is_independent_from_bus_and_semantic_samples() {
        let mut trace = Machine::run_match("f3-stack-sp-native", 5000);
        let expected = derive_stack_datapath(&trace);
        assert!(!expected.is_empty());

        trace.bus_transactions.clear();
        trace.micro_samples.clear();
        assert_eq!(derive_stack_datapath(&trace), expected);
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

    #[test]
    fn bus_stack_reconstruction_remains_available_for_old_native_traces() {
        let mut trace = Machine::run_match("f3-stack-bus-fallback", 5000);
        let native = derive_stack_datapath(&trace);
        assert!(!native.is_empty());

        trace.sp_events.clear();
        let fallback = derive_stack_datapath(&trace);
        assert!(!fallback.is_empty());
        assert_eq!(fallback.len(), native.len());
        for (legacy, current) in fallback.iter().zip(native) {
            assert_eq!(legacy.frame, current.frame);
            assert_eq!(legacy.pc, current.pc);
            assert_eq!(legacy.address, current.address);
            assert_eq!(legacy.data, current.data);
            assert!(matches!(
                (legacy.kind, current.kind),
                (StackDatapathKind::Push(_), StackDatapathKind::Push(_))
                    | (StackDatapathKind::Pop(_), StackDatapathKind::Pop(_))
            ));
        }
    }

    #[test]
    fn semantic_stack_reconstruction_remains_available_for_historical_traces() {
        let mut trace = Machine::run_match("f3-stack-legacy", 5000);
        trace.sp_events.clear();
        trace.bus_transactions.clear();
        let events = derive_stack_datapath(&trace);
        assert!(!events.is_empty());
    }
}
