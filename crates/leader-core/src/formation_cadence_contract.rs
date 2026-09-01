use std::collections::HashSet;

use crate::{
    program::RAM_BASE, BusTransactionKind, FormationCadence, MatchTrace,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FormationCadenceValidation {
    pub clocks: usize,
    pub ticks: usize,
    pub divisor3: usize,
    pub divisor2: usize,
    pub divisor1: usize,
    pub movement_transactions: usize,
}

pub fn validate_formation_cadence_contract(
    trace: &MatchTrace,
) -> Result<FormationCadenceValidation, String> {
    if trace.formation_cadence_events.is_empty() {
        return Err("native trace contains no formation cadence events".to_owned());
    }

    let mut model = FormationCadence::default();
    let mut validation = FormationCadenceValidation::default();
    let mut tick_keys = HashSet::new();

    for trace_event in &trace.formation_cadence_events {
        let expected = model.clock(trace_event.event.alive);
        if trace_event.event != expected {
            return Err(format!(
                "formation cadence transition is not causal at frame={} ordinal={} pc={:04X}: expected {:?}, got {:?}",
                trace_event.frame,
                trace_event.ordinal,
                trace_event.pc,
                expected,
                trace_event.event
            ));
        }

        validation.clocks += 1;
        match trace_event.event.divisor {
            3 => validation.divisor3 += 1,
            2 => validation.divisor2 += 1,
            1 => validation.divisor1 += 1,
            other => {
                return Err(format!(
                    "formation cadence emitted unsupported divisor {other} at frame={} ordinal={}",
                    trace_event.frame, trace_event.ordinal
                ));
            }
        }

        if trace_event.event.tick {
            validation.ticks += 1;
            tick_keys.insert((trace_event.frame, trace_event.pc));
        }
    }

    for transaction in &trace.bus_transactions {
        if !is_fleet_transaction(transaction) {
            continue;
        }
        validation.movement_transactions += 1;
        if !tick_keys.contains(&(transaction.frame, transaction.pc)) {
            return Err(format!(
                "fleet transaction escaped formation cadence tick at frame={} ordinal={} pc={:04X} control={}",
                transaction.frame, transaction.ordinal, transaction.pc, transaction.control
            ));
        }
    }

    for trace_event in trace
        .formation_cadence_events
        .iter()
        .filter(|trace_event| trace_event.event.tick)
    {
        let has_fleet_read = trace.bus_transactions.iter().any(|transaction| {
            transaction.frame == trace_event.frame
                && transaction.pc == trace_event.pc
                && transaction.kind == BusTransactionKind::Read
                && transaction.address == Some(RAM_BASE + 1)
                && transaction.control == "FLEET_X_READ"
        });
        if !has_fleet_read {
            return Err(format!(
                "formation cadence tick produced no fleet read at frame={} ordinal={} pc={:04X}",
                trace_event.frame, trace_event.ordinal, trace_event.pc
            ));
        }
    }

    if validation.ticks == 0 {
        return Err("formation cadence trace contains no movement ticks".to_owned());
    }
    if validation.divisor3 == 0 || validation.divisor2 == 0 || validation.divisor1 == 0 {
        return Err("formation cadence trace does not exercise all 3/2/1 speed bands".to_owned());
    }
    if validation.movement_transactions == 0 {
        return Err("formation cadence trace contains no fleet transactions".to_owned());
    }

    Ok(validation)
}

fn is_fleet_transaction(transaction: &crate::BusTransactionEvent) -> bool {
    matches!(
        transaction.control,
        "FLEET_X_READ" | "FLEET_X_WRITE" | "FLEET_Y_WRITE" | "FLEET_DIR_WRITE" | "FLEET_RESET"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Machine;

    #[test]
    fn full_match_cadence_is_causal_and_gates_fleet() {
        let trace = Machine::run_match("m3-cadence-contract", 5000);
        let validation =
            validate_formation_cadence_contract(&trace).expect("valid formation cadence trace");
        assert!(validation.clocks > validation.ticks);
        assert!(validation.divisor3 > 0);
        assert!(validation.divisor2 > 0);
        assert!(validation.divisor1 > 0);
        assert!(validation.movement_transactions > 0);
    }

    #[test]
    fn corrupting_counter_transition_is_detected() {
        let mut trace = Machine::run_match("m3-cadence-state-negative", 5000);
        let event = trace
            .formation_cadence_events
            .get_mut(1)
            .expect("second cadence event");
        event.event.after ^= 1;
        let error = validate_formation_cadence_contract(&trace)
            .expect_err("corrupt cadence state must fail");
        assert!(error.contains("transition is not causal"));
    }

    #[test]
    fn fleet_activity_without_tick_is_detected() {
        let mut trace = Machine::run_match("m3-cadence-gate-negative", 5000);
        let wait = trace
            .formation_cadence_events
            .iter()
            .find(|event| !event.event.tick)
            .copied()
            .expect("non-tick cadence event");
        let mut injected = trace
            .bus_transactions
            .iter()
            .find(|transaction| transaction.control == "FLEET_X_READ")
            .copied()
            .expect("fleet read transaction");
        injected.frame = wait.frame;
        injected.ordinal = wait.ordinal;
        injected.pc = wait.pc;
        trace.bus_transactions.push(injected);

        let error = validate_formation_cadence_contract(&trace)
            .expect_err("fleet activity outside cadence tick must fail");
        assert!(error.contains("escaped formation cadence tick"));
    }
}
