use crate::{derive_stack_datapath, MatchTrace, PcEventKind, PcSource, StackDatapathKind};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CallStackValidation {
    pub call_pairs: usize,
    pub return_pairs: usize,
    pub stack_bytes: usize,
}

pub fn validate_call_stack_contract(trace: &MatchTrace) -> Result<CallStackValidation, String> {
    let events = derive_stack_datapath(trace);
    if events.is_empty() {
        return Err("native trace contains no stack datapath activity".to_owned());
    }

    let mut validation = CallStackValidation::default();
    let mut byte_stack = Vec::<u8>::new();
    let mut pending_call_high = None::<(u16, u8)>;
    let mut pending_return_low = None::<u8>;
    let mut pushed_returns = Vec::<u16>::new();
    let mut popped_returns = Vec::<u16>::new();

    for event in events {
        match event.kind {
            StackDatapathKind::Push(_) => {
                byte_stack.push(event.data);
                validation.stack_bytes += 1;

                if let Some((instruction_pc, high)) = pending_call_high.take() {
                    if event.pc != instruction_pc {
                        return Err(format!(
                            "CALL push pair changed instruction PC: high from {instruction_pc:04X}, low from {:04X}",
                            event.pc
                        ));
                    }
                    let expected_return = instruction_pc.wrapping_add(3);
                    let expected_low = expected_return as u8;
                    if event.data != expected_low {
                        return Err(format!(
                            "CALL low return byte mismatch at {instruction_pc:04X}: pushed={:02X} expected={expected_low:02X}",
                            event.data
                        ));
                    }
                    let pushed_return = u16::from_be_bytes([high, event.data]);
                    if pushed_return != expected_return {
                        return Err(format!(
                            "CALL return address mismatch at {instruction_pc:04X}: pushed={pushed_return:04X} expected={expected_return:04X}"
                        ));
                    }
                    pushed_returns.push(pushed_return);
                    validation.call_pairs += 1;
                } else {
                    let expected_return = event.pc.wrapping_add(3);
                    let expected_high = (expected_return >> 8) as u8;
                    if event.data != expected_high {
                        return Err(format!(
                            "CALL high return byte mismatch at {:04X}: pushed={:02X} expected={expected_high:02X}",
                            event.pc, event.data
                        ));
                    }
                    pending_call_high = Some((event.pc, event.data));
                }
            }
            StackDatapathKind::Pop(_) => {
                let expected = byte_stack.pop().ok_or_else(|| {
                    format!("RET popped {:02X} from an empty modeled call stack", event.data)
                })?;
                if event.data != expected {
                    return Err(format!(
                        "RET stack byte mismatch: popped={:02X} expected={expected:02X}",
                        event.data
                    ));
                }
                validation.stack_bytes += 1;

                if let Some(low) = pending_return_low.take() {
                    let address = u16::from_le_bytes([low, event.data]);
                    popped_returns.push(address);
                    validation.return_pairs += 1;
                } else {
                    pending_return_low = Some(event.data);
                }
            }
        }
    }

    if pending_call_high.is_some() {
        return Err("trace ends with an incomplete CALL push pair".to_owned());
    }
    if pending_return_low.is_some() {
        return Err("trace ends with an incomplete RET pop pair".to_owned());
    }
    if !byte_stack.is_empty() {
        return Err(format!(
            "modeled call stack is not balanced: {} byte(s) remain",
            byte_stack.len()
        ));
    }
    if validation.call_pairs == 0 || validation.return_pairs == 0 {
        return Err("trace does not exercise both CALL and RET stack pairs".to_owned());
    }
    if validation.call_pairs != validation.return_pairs {
        return Err(format!(
            "CALL/RET pair count mismatch: calls={} returns={}",
            validation.call_pairs, validation.return_pairs
        ));
    }

    let call_pc_returns = trace
        .pc_events
        .iter()
        .filter_map(|event| match event.kind {
            PcEventKind::Load {
                before,
                source: PcSource::Call,
                ..
            } => Some(before),
            _ => None,
        })
        .collect::<Vec<_>>();
    let ret_pc_targets = trace
        .pc_events
        .iter()
        .filter_map(|event| match event.kind {
            PcEventKind::Load {
                after,
                source: PcSource::Return,
                ..
            } => Some(after),
            _ => None,
        })
        .collect::<Vec<_>>();

    if call_pc_returns != pushed_returns {
        return Err(format!(
            "CALL PC source and pushed return stream differ: pc={call_pc_returns:?} stack={pushed_returns:?}"
        ));
    }
    if ret_pc_targets != popped_returns {
        return Err(format!(
            "RET PC targets and popped return stream differ: pc={ret_pc_targets:?} stack={popped_returns:?}"
        ));
    }

    Ok(validation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Machine, SpEventKind};

    #[test]
    fn real_match_proves_pc_to_stack_combinational_return_path() {
        let trace = Machine::run_match("f3-call-stack-contract", 5000);
        let validation = validate_call_stack_contract(&trace).expect("valid CALL/RET path");
        assert!(validation.call_pairs > 0);
        assert_eq!(validation.call_pairs, validation.return_pairs);
        assert_eq!(validation.stack_bytes, validation.call_pairs * 4);
    }

    #[test]
    fn corrupting_an_authoritative_pushed_return_byte_is_detected() {
        let mut trace = Machine::run_match("f3-call-stack-negative", 120);
        let event = trace
            .sp_events
            .iter_mut()
            .find(|event| matches!(event.kind, SpEventKind::Push(_)))
            .expect("CPU-native stack push");
        event.data ^= 0x01;

        let error = validate_call_stack_contract(&trace).expect_err("corrupt return byte must fail");
        assert!(
            error.contains("CALL high return byte mismatch")
                || error.contains("CALL low return byte mismatch")
                || error.contains("CALL return address mismatch")
        );
    }

    #[test]
    fn bus_corruption_cannot_override_first_class_sp_authority() {
        let mut trace = Machine::run_match("f3-call-stack-bus-shadow", 120);
        let transaction = trace
            .bus_transactions
            .iter_mut()
            .find(|event| {
                event.kind == crate::BusTransactionKind::Write
                    && event.address.is_some_and(|address| (0x7f00..=0x7fff).contains(&address))
            })
            .expect("stack write");
        transaction.data = transaction.data.map(|value| value ^ 0x01);

        validate_call_stack_contract(&trace)
            .expect("first-class SP stream must remain authoritative over bus fallback");
    }
}
