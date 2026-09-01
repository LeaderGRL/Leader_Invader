use std::collections::HashMap;

use crate::{
    control_word_at, microcode::decode as decode_microcode, microcode::internal, microcode::uaddr,
    BusTransactionKind, ControlLatchKind, MatchTrace, MicroAddressEvent, MicroCycleKind,
    PcEventKind,
};

const REGW_BIT: u32 = 1 << 0;
const ALU_BIT: u32 = 1 << 1;
const MEMR_BIT: u32 = 1 << 2;
const MEMW_BIT: u32 = 1 << 3;
const PCLD_BIT: u32 = 1 << 4;
const STACK_BIT: u32 = 1 << 5;

type MicroIndex<'a> = HashMap<(u32, u16), Vec<&'a MicroAddressEvent>>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeTraceValidation {
    pub micro_words: usize,
    pub decode_latches: usize,
    pub alu_events: usize,
    pub flag_events: usize,
    pub control_latches: usize,
    pub register_writes: usize,
    pub pc_increments: usize,
    pub pc_loads: usize,
    pub sp_events: usize,
    pub rom_fetches: usize,
    pub cpu_reads: usize,
    pub cpu_writes: usize,
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

    let micro_index = index_micro_words(&trace.micro_addresses);
    let mut validated = NativeTraceValidation::default();

    for cycle in trace
        .micro_cycles
        .iter()
        .filter(|event| event.kind == MicroCycleKind::DecodeLatch)
    {
        require_micro_authority(
            &micro_index,
            cycle.frame,
            cycle.ordinal,
            0,
            None,
            |bits| internal_on(bits, internal::IR_LOAD),
            "IR_LOAD for native DecodeLatch",
        )?;
        validated.decode_latches += 1;
    }

    for event in &trace.alu_events {
        require_micro_authority(
            &micro_index,
            event.frame,
            event.ordinal,
            0,
            Some(event.control),
            |bits| bits & ALU_BIT != 0,
            "ALU enable for native AluEvent",
        )?;
        validated.alu_events += 1;
    }

    for event in &trace.flag_events {
        require_micro_authority(
            &micro_index,
            event.frame,
            event.ordinal,
            0,
            Some(event.control),
            |bits| internal_on(bits, internal::FLAGS_LOAD),
            "FLAGS_LOAD for native FlagEvent",
        )?;
        validated.flag_events += 1;
    }

    for event in &trace.control_latch_events {
        let signal = signal_for_control_latch(event.kind);
        require_micro_authority(
            &micro_index,
            event.frame,
            event.ordinal,
            0,
            Some(event.control),
            |bits| internal_on(bits, signal),
            "physical load enable for native ControlLatchEvent",
        )?;
        validated.control_latches += 1;
    }

    for event in &trace.register_writes {
        // Register write-back follows the ALU trace sample, so its local ordinal
        // is one step after the execute row that carries REGW + ARCH_COMMIT.
        require_micro_authority(
            &micro_index,
            event.frame,
            event.ordinal,
            1,
            Some(event.control),
            |bits| bits & REGW_BIT != 0 && internal_on(bits, internal::ARCH_COMMIT),
            "REGW + ARCH_COMMIT for native RegisterWriteEvent",
        )?;
        validated.register_writes += 1;
    }

    for event in &trace.pc_events {
        match event.kind {
            PcEventKind::Increment(_) => {
                // ROM fetch sampling increments the trace ordinal before the
                // controlled PC ripple increment is emitted. The physical
                // FETCH_T1/OPERAND_T1 word is therefore exactly one ordinal back.
                require_shared_micro_row(
                    &micro_index,
                    event.frame,
                    event.ordinal.saturating_sub(1),
                    &[uaddr::FETCH_T1, uaddr::OPERAND_T1],
                    |bits| internal_on(bits, internal::PC_INC),
                    "PC_INC authority for native PcEvent::Increment",
                )?;
                validated.pc_increments += 1;
            }
            PcEventKind::Load { control, .. } => {
                require_micro_authority(
                    &micro_index,
                    event.frame,
                    event.ordinal,
                    0,
                    Some(control),
                    |bits| bits & PCLD_BIT != 0 && internal_on(bits, internal::ARCH_COMMIT),
                    "PCLD + ARCH_COMMIT for native PcEvent::Load",
                )?;
                validated.pc_loads += 1;
            }
        }
    }

    for event in &trace.sp_events {
        require_micro_authority(
            &micro_index,
            event.frame,
            event.ordinal,
            0,
            None,
            |bits| bits & STACK_BIT != 0,
            "STACK enable for native SpEvent",
        )?;
        validated.sp_events += 1;
    }

    for event in &trace.bus_transactions {
        match (event.kind, event.control) {
            (BusTransactionKind::Fetch, "ROM_FETCH") => {
                require_shared_micro_row(
                    &micro_index,
                    event.frame,
                    event.ordinal,
                    &[uaddr::FETCH_T1, uaddr::OPERAND_T1],
                    |bits| {
                        bits & MEMR_BIT != 0
                            && internal_on(bits, internal::MDR_LOAD)
                            && internal_on(bits, internal::PC_INC)
                            && internal_on(bits, internal::BUS_DATA_ENABLE)
                    },
                    "FETCH_T1/OPERAND_T1 authority for native ROM fetch",
                )?;
                validated.rom_fetches += 1;
            }
            (BusTransactionKind::Read, "CPU_READ") => {
                require_shared_micro_row(
                    &micro_index,
                    event.frame,
                    event.ordinal,
                    &[uaddr::READ_T1],
                    |bits| {
                        bits & MEMR_BIT != 0
                            && internal_on(bits, internal::MDR_LOAD)
                            && internal_on(bits, internal::BUS_DATA_ENABLE)
                    },
                    "READ_T1 authority for native CPU read",
                )?;
                validated.cpu_reads += 1;
            }
            (BusTransactionKind::Write, "CPU_WRITE") => {
                require_shared_micro_row(
                    &micro_index,
                    event.frame,
                    event.ordinal,
                    &[uaddr::WRITE_T2],
                    |bits| {
                        bits & MEMW_BIT != 0
                            && internal_on(bits, internal::BUS_ADDRESS_ENABLE)
                            && internal_on(bits, internal::BUS_DATA_ENABLE)
                            && internal_on(bits, internal::ARCH_COMMIT)
                    },
                    "WRITE_T2 authority for native CPU write",
                )?;
                validated.cpu_writes += 1;
            }
            _ => {}
        }
    }

    for event in &trace.micro_addresses {
        let expected = control_word_at(event.address, event.opcode).bits24();
        if event.control_bits != expected {
            return Err(format!(
                "microcode trace mismatch at frame={} ordinal={} address={:02X} opcode={:02X}: traced={:06X} expected={:06X}",
                event.frame,
                event.ordinal,
                event.address,
                event.opcode,
                event.control_bits,
                expected
            ));
        }
        validated.micro_words += 1;
    }

    if validated.micro_words == 0 {
        return Err("native trace contains no validated microcode words".to_owned());
    }
    if validated.decode_latches == 0 {
        return Err("native trace contains no validated decode latches".to_owned());
    }
    if validated.alu_events == 0 {
        return Err("native trace contains no validated ALU events".to_owned());
    }
    if validated.flag_events == 0 {
        return Err("native trace contains no validated flag latch events".to_owned());
    }
    if validated.control_latches == 0 {
        return Err("native trace contains no validated control latch events".to_owned());
    }
    if validated.register_writes == 0 {
        return Err("native trace contains no validated register writes".to_owned());
    }
    if validated.pc_increments == 0 {
        return Err("native trace contains no validated PC increments".to_owned());
    }
    if validated.pc_loads == 0 {
        return Err("native trace contains no validated PC loads".to_owned());
    }
    if validated.sp_events == 0 {
        return Err("native trace contains no validated SP events".to_owned());
    }
    if validated.rom_fetches == 0 {
        return Err("native trace contains no validated ROM fetches".to_owned());
    }
    if validated.cpu_reads == 0 {
        return Err("native trace contains no validated CPU reads".to_owned());
    }
    if validated.cpu_writes == 0 {
        return Err("native trace contains no validated CPU writes".to_owned());
    }

    Ok(validated)
}

const fn signal_for_control_latch(kind: ControlLatchKind) -> u16 {
    match kind {
        ControlLatchKind::AddressLo => internal::ADDR_LO_LOAD,
        ControlLatchKind::AddressHi => internal::ADDR_HI_LOAD,
        ControlLatchKind::Condition => internal::CONDITION_LOAD,
        ControlLatchKind::PcSelect => internal::PC_SELECT,
        ControlLatchKind::RegSelect => internal::REG_SELECT,
    }
}

fn index_micro_words(events: &[MicroAddressEvent]) -> MicroIndex<'_> {
    let mut index = HashMap::with_capacity(events.len().min(16_384));
    for event in events {
        index
            .entry((event.frame, event.ordinal))
            .or_insert_with(Vec::new)
            .push(event);
    }
    index
}

fn require_micro_authority<F>(
    index: &MicroIndex<'_>,
    frame: u32,
    ordinal: u16,
    max_ordinal_gap: u16,
    control: Option<&str>,
    predicate: F,
    authority: &str,
) -> Result<(), String>
where
    F: Fn(u32) -> bool,
{
    let start = ordinal.saturating_sub(max_ordinal_gap);
    let end = ordinal.saturating_add(max_ordinal_gap);
    let mut observed = Vec::new();

    for nearby_ordinal in start..=end {
        let Some(events) = index.get(&(frame, nearby_ordinal)) else {
            continue;
        };
        for event in events {
            if control.is_some_and(|expected| {
                !decode_microcode(event.opcode)
                    .is_some_and(|instruction| instruction.mnemonic == expected)
            }) {
                continue;
            }
            if predicate(event.control_bits) {
                return Ok(());
            }
            observed.push(format!(
                "{:02X}/{:02X}:{:06X}",
                event.opcode, event.address, event.control_bits
            ));
        }
    }

    Err(format!(
        "missing {authority} near frame={frame} ordinal={ordinal} gap={max_ordinal_gap}; micro_words=[{}]",
        observed.join(",")
    ))
}

fn require_shared_micro_row<F>(
    index: &MicroIndex<'_>,
    frame: u32,
    ordinal: u16,
    allowed_addresses: &[u8],
    predicate: F,
    authority: &str,
) -> Result<(), String>
where
    F: Fn(u32) -> bool,
{
    let Some(events) = index.get(&(frame, ordinal)) else {
        return Err(format!(
            "missing {authority} at frame={frame} ordinal={ordinal}; no micro word"
        ));
    };

    if events.iter().any(|event| {
        allowed_addresses.contains(&event.address) && predicate(event.control_bits)
    }) {
        return Ok(());
    }

    let observed = events
        .iter()
        .map(|event| {
            format!(
                "{:02X}/{:02X}:{:06X}",
                event.opcode, event.address, event.control_bits
            )
        })
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
    fn native_events_have_local_microcode_authority() {
        let trace = Machine::run_match("f3-native-authority", 120);
        let validation = validate_native_control_authority(&trace).expect("valid F3 trace");
        assert!(validation.micro_words > 0);
        assert!(validation.decode_latches > 0);
        assert!(validation.alu_events > 0);
        assert!(validation.flag_events > 0);
        assert!(validation.control_latches > 0);
        assert!(validation.register_writes > 0);
        assert!(validation.pc_increments > 0);
        assert!(validation.pc_loads > 0);
        assert!(validation.sp_events > 0);
        assert!(validation.rom_fetches > 0);
        assert!(validation.cpu_reads > 0);
        assert!(validation.cpu_writes > 0);
    }

    #[test]
    fn removing_commit_authority_is_detected() {
        let mut trace = Machine::run_match("f3-authority-negative", 120);
        let register = trace.register_writes.first().expect("register write").clone();
        for event in trace.micro_addresses.iter_mut().filter(|event| {
            event.frame == register.frame
                && event.ordinal.abs_diff(register.ordinal) <= 1
                && decode_microcode(event.opcode)
                    .is_some_and(|instruction| instruction.mnemonic == register.control)
        }) {
            event.control_bits &= !REGW_BIT;
            event.control_bits &= !((internal::ARCH_COMMIT as u32) << 8);
        }
        let error = validate_native_control_authority(&trace).expect_err("authority corruption must fail");
        assert!(error.contains("REGW + ARCH_COMMIT"));
    }

    #[test]
    fn removing_pc_increment_authority_is_detected() {
        let mut trace = Machine::run_match("f3-pc-inc-authority-negative", 120);
        let pc = trace
            .pc_events
            .iter()
            .find(|event| matches!(event.kind, PcEventKind::Increment(_)))
            .copied()
            .expect("PC increment");
        let control_ordinal = pc.ordinal.saturating_sub(1);
        for event in trace.micro_addresses.iter_mut().filter(|event| {
            event.frame == pc.frame
                && event.ordinal == control_ordinal
                && matches!(event.address, uaddr::FETCH_T1 | uaddr::OPERAND_T1)
        }) {
            event.control_bits &= !((internal::PC_INC as u32) << 8);
        }
        let error = validate_native_control_authority(&trace)
            .expect_err("PC_INC authority corruption must fail");
        assert!(error.contains("PC_INC authority for native PcEvent::Increment"));
    }

    #[test]
    fn removing_stack_authority_is_detected() {
        let mut trace = Machine::run_match("f3-stack-authority-negative", 120);
        let sp = *trace.sp_events.first().expect("SP event");
        for event in trace.micro_addresses.iter_mut().filter(|event| {
            event.frame == sp.frame && event.ordinal == sp.ordinal
        }) {
            event.control_bits &= !STACK_BIT;
        }
        let error = validate_native_control_authority(&trace)
            .expect_err("STACK authority corruption must fail");
        assert!(error.contains("STACK enable for native SpEvent"));
    }

    #[test]
    fn removing_flags_load_authority_is_detected() {
        let mut trace = Machine::run_match("f3-flags-authority-negative", 120);
        let flags = *trace.flag_events.first().expect("flag event");
        for event in trace.micro_addresses.iter_mut().filter(|event| {
            event.frame == flags.frame
                && event.ordinal == flags.ordinal
                && decode_microcode(event.opcode)
                    .is_some_and(|instruction| instruction.mnemonic == flags.control)
        }) {
            event.control_bits &= !((internal::FLAGS_LOAD as u32) << 8);
        }
        let error = validate_native_control_authority(&trace)
            .expect_err("FLAGS_LOAD corruption must fail");
        assert!(error.contains("FLAGS_LOAD for native FlagEvent"));
    }

    #[test]
    fn removing_control_latch_authority_is_detected() {
        let mut trace = Machine::run_match("f3-control-latch-authority-negative", 120);
        let latch = *trace
            .control_latch_events
            .first()
            .expect("control latch event");
        let signal = signal_for_control_latch(latch.kind);
        for event in trace.micro_addresses.iter_mut().filter(|event| {
            event.frame == latch.frame
                && event.ordinal == latch.ordinal
                && decode_microcode(event.opcode)
                    .is_some_and(|instruction| instruction.mnemonic == latch.control)
        }) {
            event.control_bits &= !((signal as u32) << 8);
        }
        let error = validate_native_control_authority(&trace)
            .expect_err("control latch authority corruption must fail");
        assert!(error.contains("ControlLatchEvent"));
    }

    #[test]
    fn corrupting_traced_microcode_word_is_detected() {
        let mut trace = Machine::run_match("f3-micro-word-negative", 120);
        let event = trace.micro_addresses.first_mut().expect("micro word");
        event.control_bits ^= 1 << 23;
        let error = validate_native_control_authority(&trace).expect_err("microcode corruption must fail");
        assert!(error.contains("microcode trace mismatch"));
    }

    #[test]
    fn removing_cpu_write_bus_authority_is_detected() {
        let mut trace = Machine::run_match("f3-write-authority-negative", 120);
        let write = trace
            .bus_transactions
            .iter()
            .find(|event| event.kind == BusTransactionKind::Write && event.control == "CPU_WRITE")
            .expect("CPU write")
            .clone();
        for event in trace.micro_addresses.iter_mut().filter(|event| {
            event.frame == write.frame
                && event.ordinal == write.ordinal
                && event.address == uaddr::WRITE_T2
        }) {
            event.control_bits &= !MEMW_BIT;
            event.control_bits &= !((internal::ARCH_COMMIT as u32) << 8);
        }
        let error = validate_native_control_authority(&trace).expect_err("bus authority corruption must fail");
        assert!(error.contains("WRITE_T2 authority"));
    }
}
