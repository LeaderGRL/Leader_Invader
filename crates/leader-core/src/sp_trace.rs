use crate::{
    memory_map::STACK_REGION, ripple_decrement16, ripple_increment16, BusTransactionKind,
    MatchTrace, SpEvent, SpEventKind,
};

const MAX_NATIVE_SP_BUS_GAP: u16 = 4;

/// Compatibility fallback for historical traces that predate CPU-native `SpEvent`s.
///
/// Current execution emits the first-class SP stream directly from the CPU at the
/// exact ripple increment/decrement mutation point. Existing native events are
/// therefore authoritative and are never overwritten here. Only an empty legacy
/// trace is reconstructed from its exact stack-window bus transactions.
pub fn materialize_sp_events(trace: &mut MatchTrace) {
    if !trace.sp_events.is_empty() {
        return;
    }

    for transaction in &trace.bus_transactions {
        let Some(address) = transaction.address else {
            continue;
        };
        let Some(data) = transaction.data else {
            continue;
        };
        if !STACK_REGION.contains(address) {
            continue;
        }

        let kind = match transaction.kind {
            BusTransactionKind::Write => {
                let step = ripple_decrement16(address.wrapping_add(1));
                debug_assert_eq!(step.after, address);
                SpEventKind::Push(step)
            }
            BusTransactionKind::Read => {
                let step = ripple_increment16(address);
                SpEventKind::Pop(step)
            }
            _ => continue,
        };

        trace.sp_events.push(SpEvent {
            frame: transaction.frame,
            ordinal: transaction.ordinal,
            pc: transaction.pc,
            address,
            data,
            kind,
            control: transaction.control,
        });
    }
}

pub fn validate_sp_event_stream(trace: &MatchTrace) -> Result<usize, String> {
    if trace.sp_events.is_empty() {
        return Err("native trace contains no first-class SP events".to_owned());
    }

    let stack_transactions = trace
        .bus_transactions
        .iter()
        .filter(|event| {
            event.address.is_some_and(|address| STACK_REGION.contains(address))
                && matches!(event.kind, BusTransactionKind::Read | BusTransactionKind::Write)
        })
        .collect::<Vec<_>>();

    if stack_transactions.len() != trace.sp_events.len() {
        return Err(format!(
            "SP event count differs from native stack transactions: sp={} bus={}",
            trace.sp_events.len(),
            stack_transactions.len()
        ));
    }

    for (sp, bus) in trace.sp_events.iter().zip(stack_transactions) {
        let expected_kind = match bus.kind {
            BusTransactionKind::Write => "push",
            BusTransactionKind::Read => "pop",
            _ => unreachable!(),
        };
        if sp.frame != bus.frame
            || sp.pc != bus.pc
            || Some(sp.address) != bus.address
            || Some(sp.data) != bus.data
            || sp.kind.as_str() != expected_kind
        {
            return Err(format!(
                "SP event diverges from native stack transaction at frame={} bus_ordinal={} sp_ordinal={}",
                bus.frame, bus.ordinal, sp.ordinal
            ));
        }

        if sp.ordinal < bus.ordinal || sp.ordinal - bus.ordinal > MAX_NATIVE_SP_BUS_GAP {
            return Err(format!(
                "SP event is not locally ordered after stack transaction at frame={} bus_ordinal={} sp_ordinal={}",
                bus.frame, bus.ordinal, sp.ordinal
            ));
        }

        match sp.kind {
            SpEventKind::Push(step) => {
                if step.after != step.before.wrapping_sub(1) || step.after != sp.address {
                    return Err(format!(
                        "invalid SP PUSH ripple transition {:04X}->{:04X} address={:04X}",
                        step.before, step.after, sp.address
                    ));
                }
            }
            SpEventKind::Pop(step) => {
                if step.after != step.before.wrapping_add(1) || step.before != sp.address {
                    return Err(format!(
                        "invalid SP POP ripple transition {:04X}->{:04X} address={:04X}",
                        step.before, step.after, sp.address
                    ));
                }
            }
        }
    }

    Ok(trace.sp_events.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Machine;

    #[test]
    fn stack_window_is_canonical_memory_map_region() {
        assert_eq!(STACK_REGION.start, crate::memory_map::STACK_BASE);
        assert_eq!(STACK_REGION.end, crate::memory_map::STACK_END);
    }

    #[test]
    fn cpu_emits_balanced_bit_accurate_sp_stream_directly() {
        let trace = Machine::run_match("f3-sp-cpu-native", 5000);
        assert!(!trace.sp_events.is_empty());
        let count = validate_sp_event_stream(&trace).expect("valid direct CPU SP stream");
        assert!(count > 0);
        let pushes = trace
            .sp_events
            .iter()
            .filter(|event| matches!(event.kind, SpEventKind::Push(_)))
            .count();
        let pops = trace
            .sp_events
            .iter()
            .filter(|event| matches!(event.kind, SpEventKind::Pop(_)))
            .count();
        assert_eq!(pushes, pops);
    }

    #[test]
    fn materialization_is_noop_when_cpu_native_stream_exists() {
        let mut trace = Machine::run_match("f3-sp-noop", 5000);
        let expected = trace.sp_events.clone();
        assert!(!expected.is_empty());
        materialize_sp_events(&mut trace);
        assert_eq!(trace.sp_events, expected);
    }

    #[test]
    fn historical_bus_fallback_reconstructs_same_sp_stream_semantics() {
        let mut trace = Machine::run_match("f3-sp-fallback", 5000);
        let expected = trace.sp_events.clone();
        assert!(!expected.is_empty());
        trace.sp_events.clear();
        materialize_sp_events(&mut trace);
        assert_eq!(trace.sp_events.len(), expected.len());

        for (legacy, native) in trace.sp_events.iter().zip(expected) {
            assert_eq!(legacy.frame, native.frame);
            assert_eq!(legacy.pc, native.pc);
            assert_eq!(legacy.address, native.address);
            assert_eq!(legacy.data, native.data);
            assert_eq!(legacy.kind.as_str(), native.kind.as_str());
            assert_eq!(legacy.kind.before(), native.kind.before());
            assert_eq!(legacy.kind.after(), native.kind.after());
            assert_eq!(legacy.kind.chain(), native.kind.chain());
        }
    }

    #[test]
    fn corrupted_sp_transition_is_detected() {
        let mut trace = Machine::run_match("f3-sp-negative", 5000);
        let event = trace.sp_events.first_mut().expect("SP event");
        event.address ^= 1;
        let error = validate_sp_event_stream(&trace).expect_err("corrupt SP event must fail");
        assert!(error.contains("SP event diverges"));
    }

    #[test]
    fn implausibly_delayed_sp_event_is_detected() {
        let mut trace = Machine::run_match("f3-sp-order-negative", 5000);
        let event = trace.sp_events.first_mut().expect("SP event");
        event.ordinal = event.ordinal.saturating_add(MAX_NATIVE_SP_BUS_GAP + 1);
        let error = validate_sp_event_stream(&trace).expect_err("delayed SP event must fail");
        assert!(error.contains("locally ordered"));
    }
}
