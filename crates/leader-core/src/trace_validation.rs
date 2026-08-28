use crate::{microcode::internal, MatchTrace, MicroCycleKind, PcEventKind};

const REGW_BIT: u32 = 1 << 0;
const ALU_BIT: u32 = 1 << 1;
const PCLD_BIT: u32 = 1 << 4;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeTraceValidation {
    pub decode_latches: usize,
    pub alu_events: usize,
    pub register_writes: usize,
    pub pc_loads: usize,
}

pub fn validate_native_control_authority(
    trace: &MatchTrace,
) -> Result<NativeTraceValidation, String> {
    if trace.micro_addresses.is_empty() {
        return Err("native trace contains no micro-address events".to_owned());
    }
    if trace.micro_cycles.is_empty() {
        return Err("native trace contains no microcycle events".to_owned());
    }

    let mut validated = NativeTraceValidation::default();

    for cycle in trace
        .micro_cycles
        .iter()
        .filter(|event| event.kind == MicroCycleKind::DecodeLatch)
    {
        require_micro_word(trace, cycle.frame, cycle.ordinal, |bits| {
            internal_on(bits, internal::IR_LOAD)
        }, "IR_LOAD for native DecodeLatch")?;
        validated.decode_latches += 1;
    }

    for event in &trace.alu_events {
        require_micro_word(
            trace,
            event.frame,
            event.ordinal,
            |bits| bits & ALU_BIT != 0,
            "ALU enable for native AluEvent",
        )?;
        validated.alu_events += 1;
    }

    for event in &trace.register_writes {
        require_micro_word(
            trace,
            event.frame,
            event.ordinal,
            |bits| bits & REGW_BIT != 0 && internal_on(bits, internal::ARCH_COMMIT),
            "REGW + ARCH_COMMIT for native RegisterWriteEvent",
        )?;
        validated.register_writes += 1;
    }

    for event in &trace.pc_events {
        let PcEventKind::Load { .. } = event.kind else {
            continue;
        };
        require_micro_word(
            trace,
            event.frame,
            event.ordinal,
            |bits| bits & PCLD_BIT != 0 && internal_on(bits, internal::ARCH_COMMIT),
            "PCLD + ARCH_COMMIT for native PcEvent::Load",
        )?;
        validated.pc_loads += 1;
    }

    if validated.decode_latches == 0 {
        return Err("native trace contains no validated decode latches".to_owned());
    }
    if validated.alu_events == 0 {
        return Err("native trace contains no validated ALU events".to_owned());
    }
    if validated.register_writes == 0 {
        return Err("native trace contains no validated register writes".to_owned());
    }
    if validated.pc_loads == 0 {
        return Err("native trace contains no validated PC loads".to_owned());
    }

    Ok(validated)
}

fn require_micro_word<F>(
    trace: &MatchTrace,
    frame: u32,
    ordinal: u16,
    predicate: F,
    authority: &str,
) -> Result<(), String>
where
    F: Fn(u32) -> bool,
{
    let candidates = trace
        .micro_addresses
        .iter()
        .filter(|event| event.frame == frame && event.ordinal == ordinal)
        .collect::<Vec<_>>();

    if candidates.iter().any(|event| predicate(event.control_bits)) {
        return Ok(());
    }

    let observed = candidates
        .iter()
        .map(|event| format!("{:02X}:{:06X}", event.address, event.control_bits))
        .collect::<Vec<_>>()
        .join(",");
    Err(format!(
        "missing {authority} at frame={frame} ordinal={ordinal}; micro_words=[{observed}]"
    ))
}

const fn internal_on(control_bits: u32, signal: u16) -> bool {
    control_bits & ((signal as u32) << 8) != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Machine;

    #[test]
    fn complete_match_native_events_have_same_tick_microcode_authority() {
        let trace = Machine::run_match("f3-native-authority", 5000);
        let validation = validate_native_control_authority(&trace).expect("valid F3 trace");
        assert!(validation.decode_latches > 0);
        assert!(validation.alu_events > 0);
        assert!(validation.register_writes > 0);
        assert!(validation.pc_loads > 0);
    }

    #[test]
    fn removing_commit_authority_is_detected() {
        let mut trace = Machine::run_match("f3-authority-negative", 5000);
        let register = trace.register_writes.first().expect("register write");
        let event = trace
            .micro_addresses
            .iter_mut()
            .find(|event| event.frame == register.frame && event.ordinal == register.ordinal)
            .expect("matching micro word");
        event.control_bits &= !REGW_BIT;
        event.control_bits &= !((internal::ARCH_COMMIT as u32) << 8);
        let error = validate_native_control_authority(&trace).expect_err("authority corruption must fail");
        assert!(error.contains("REGW + ARCH_COMMIT"));
    }
}
