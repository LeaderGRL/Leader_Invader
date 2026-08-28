use crate::{
    ripple_decrement16, ripple_increment16, BusTransactionKind, MatchTrace, SpEvent, SpEventKind,
};

const STACK_WINDOW_START: u16 = 0x7F00;
const STACK_WINDOW_END: u16 = 0x7FFF;

/// Materializes the first-class SP mutation stream from native stack bus transactions.
///
/// This is a trace-normalization boundary, not semantic reconstruction: every event
/// comes from the exact CPU read/write transaction that accompanies the real PUSH/POP.
pub fn materialize_sp_events(trace: &mut MatchTrace) {
    trace.sp_events.clear();

    for transaction in &trace.bus_transactions {
        let Some(address) = transaction.address else {
            continue;
        };
        let Some(data) = transaction.data else {
            continue;
        };
        if !(STACK_WINDOW_START..=STACK_WINDOW_END).contains(&address) {
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
            event.address
                .is_some_and(|address| (STACK_WINDOW_START..=STACK_WINDOW_END).contains(&address))
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
            || sp.ordinal != bus.ordinal
            || sp.pc != bus.pc
            || Some(sp.address) != bus.address
            || Some(sp.data) != bus.data
            || sp.kind.as_str() != expected_kind
        {
            return Err(format!(
                "SP event diverges from native stack transaction at frame={} ordinal={}",
                bus.frame, bus.ordinal
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
    fn materialized_sp_stream_is_balanced_and_bit_accurate() {
        let mut trace = Machine::run_match("f3-sp-native", 5000);
        materialize_sp_events(&mut trace);
        let count = validate_sp_event_stream(&trace).expect("valid SP stream");
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
    fn corrupted_sp_transition_is_detected() {
        let mut trace = Machine::run_match("f3-sp-negative", 5000);
        materialize_sp_events(&mut trace);
        let event = trace.sp_events.first_mut().expect("SP event");
        event.address ^= 1;
        let error = validate_sp_event_stream(&trace).expect_err("corrupt SP event must fail");
        assert!(error.contains("SP event diverges"));
    }
}
