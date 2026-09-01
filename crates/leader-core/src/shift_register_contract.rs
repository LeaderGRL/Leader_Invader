use crate::{
    program::{SHIFT_DATA, SHIFT_OFFSET, SHIFT_RESULT},
    BusDataSource, BusTransactionKind, MatchTrace, ShiftRegister16, ShiftRegisterEventKind,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShiftRegisterValidation {
    pub data_writes: usize,
    pub offset_writes: usize,
    pub reads: usize,
}

pub fn validate_shift_register_contract(
    trace: &MatchTrace,
) -> Result<ShiftRegisterValidation, String> {
    if trace.shift_register_events.is_empty() {
        return Err("native trace contains no shift-register events".to_owned());
    }

    let mut model = ShiftRegister16::default();
    let mut validation = ShiftRegisterValidation::default();

    for event in &trace.shift_register_events {
        let bus = trace
            .bus_transactions
            .iter()
            .find(|transaction| {
                transaction.frame == event.frame
                    && transaction.ordinal == event.ordinal
                    && transaction.pc == event.pc
                    && transaction.address == Some(event.address)
            })
            .ok_or_else(|| {
                format!(
                    "shift-register event has no same-tick bus transaction at frame={} ordinal={} pc={:04X} address={:04X}",
                    event.frame, event.ordinal, event.pc, event.address
                )
            })?;

        match event.kind {
            ShiftRegisterEventKind::DataWrite {
                before,
                after,
                input,
            } => {
                if event.address != SHIFT_DATA
                    || bus.kind != BusTransactionKind::Write
                    || bus.data != Some(input)
                {
                    return Err(format!(
                        "shift data write does not match CPU bus at frame={} ordinal={}",
                        event.frame, event.ordinal
                    ));
                }
                let expected = model.write_data(input);
                if event.kind != expected || before != expected_before(expected) || after != model.value() {
                    return Err(format!(
                        "shift data state transition is not causal at frame={} ordinal={}",
                        event.frame, event.ordinal
                    ));
                }
                validation.data_writes += 1;
            }
            ShiftRegisterEventKind::OffsetWrite {
                before,
                after,
                input,
            } => {
                if event.address != SHIFT_OFFSET
                    || bus.kind != BusTransactionKind::Write
                    || bus.data != Some(input)
                {
                    return Err(format!(
                        "shift offset write does not match CPU bus at frame={} ordinal={}",
                        event.frame, event.ordinal
                    ));
                }
                let expected = model.write_offset(input);
                if event.kind != expected || before != expected_offset_before(expected) || after != model.offset() {
                    return Err(format!(
                        "shift offset state transition is not causal at frame={} ordinal={}",
                        event.frame, event.ordinal
                    ));
                }
                validation.offset_writes += 1;
            }
            ShiftRegisterEventKind::Read {
                value,
                offset,
                result,
            } => {
                if event.address != SHIFT_RESULT
                    || bus.kind != BusTransactionKind::Read
                    || bus.data != Some(result)
                    || bus.data_source != BusDataSource::Device
                {
                    return Err(format!(
                        "shift result read does not match device bus at frame={} ordinal={}",
                        event.frame, event.ordinal
                    ));
                }
                let expected = model.read_event();
                if event.kind != expected
                    || value != model.value()
                    || offset != model.offset()
                    || result != model.read()
                {
                    return Err(format!(
                        "shift result is not derived from current register state at frame={} ordinal={}",
                        event.frame, event.ordinal
                    ));
                }
                validation.reads += 1;
            }
        }
    }

    if validation.data_writes < 2 {
        return Err("shift-register trace does not exercise two-byte loading".to_owned());
    }
    if validation.offset_writes == 0 {
        return Err("shift-register trace does not exercise offset selection".to_owned());
    }
    if validation.reads == 0 {
        return Err("shift-register trace does not exercise result readback".to_owned());
    }

    Ok(validation)
}

const fn expected_before(kind: ShiftRegisterEventKind) -> u16 {
    match kind {
        ShiftRegisterEventKind::DataWrite { before, .. } => before,
        _ => 0,
    }
}

const fn expected_offset_before(kind: ShiftRegisterEventKind) -> u8 {
    match kind {
        ShiftRegisterEventKind::OffsetWrite { before, .. } => before,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Machine;

    #[test]
    fn boot_self_test_has_native_shift_register_authority() {
        let trace = Machine::run_match("m3-shift-contract", 120);
        let validation =
            validate_shift_register_contract(&trace).expect("valid shift-register trace");
        assert_eq!(validation.data_writes, 2);
        assert_eq!(validation.offset_writes, 1);
        assert_eq!(validation.reads, 1);
    }

    #[test]
    fn corrupting_native_shift_state_is_detected() {
        let mut trace = Machine::run_match("m3-shift-state-negative", 120);
        let event = trace.shift_register_events.get_mut(1).expect("second data write");
        if let ShiftRegisterEventKind::DataWrite { after, .. } = &mut event.kind {
            *after ^= 1;
        }
        let error = validate_shift_register_contract(&trace)
            .expect_err("corrupt shift state must fail");
        assert!(error.contains("state transition is not causal"));
    }

    #[test]
    fn removing_same_tick_bus_authority_is_detected() {
        let mut trace = Machine::run_match("m3-shift-bus-negative", 120);
        let event = trace.shift_register_events[0];
        trace.bus_transactions.retain(|transaction| {
            !(transaction.frame == event.frame
                && transaction.ordinal == event.ordinal
                && transaction.pc == event.pc
                && transaction.address == Some(event.address))
        });
        let error = validate_shift_register_contract(&trace)
            .expect_err("missing shift bus transaction must fail");
        assert!(error.contains("no same-tick bus transaction"));
    }
}
