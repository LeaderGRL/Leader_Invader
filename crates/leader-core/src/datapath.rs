use crate::trace::{MatchTrace, PhaseKind};

/// Bit-accurate state for the first F3 critical path.
///
/// These are not presentation guesses: MAR/MDR are latched from actual ROM fetch
/// bus transactions, IR is latched from the actual decode event, and PC is the
/// semantic program-counter value that initiated that fetch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DatapathState {
    pub pc: u16,
    pub mar: u16,
    pub mdr: u8,
    pub ir: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatapathEvent {
    pub frame: u32,
    pub ordinal: u16,
    pub phase: PhaseKind,
    pub state: DatapathState,
}

#[must_use]
pub fn derive_datapath(trace: &MatchTrace) -> Vec<DatapathEvent> {
    let mut state = DatapathState::default();
    let mut events = Vec::with_capacity(trace.micro_samples.len());

    for sample in &trace.micro_samples {
        match sample.phase {
            PhaseKind::Fetch => {
                state.pc = sample.pc;
                if let Some(address) = sample.address {
                    state.mar = address;
                }
                if let Some(data) = sample.data {
                    state.mdr = data;
                }
            }
            PhaseKind::Decode => {
                // Only decode events carrying an opcode latch IR. Control-flow
                // annotations such as CALL/RET intentionally leave IR unchanged.
                if sample.address == Some(sample.pc) {
                    if let Some(opcode) = sample.data {
                        state.ir = opcode;
                    }
                }
            }
            _ => {}
        }

        events.push(DatapathEvent {
            frame: sample.frame,
            ordinal: sample.ordinal,
            phase: sample.phase,
            state,
        });
    }
    events
}

#[must_use]
pub const fn bit16(value: u16, bit: usize) -> bool {
    value & (1_u16 << bit) != 0
}

#[must_use]
pub const fn bit8(value: u8, bit: usize) -> bool {
    value & (1_u8 << bit) != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Machine;

    #[test]
    fn fetch_latches_real_pc_mar_mdr_and_decode_latches_ir() {
        let trace = Machine::run_match("f3-fetch", 5000);
        let events = derive_datapath(&trace);
        let fetch = trace
            .micro_samples
            .iter()
            .position(|sample| sample.phase == PhaseKind::Fetch)
            .expect("fetch sample");
        let sample = &trace.micro_samples[fetch];
        assert_eq!(events[fetch].state.pc, sample.pc);
        assert_eq!(events[fetch].state.mar, sample.address.expect("fetch address"));
        assert_eq!(events[fetch].state.mdr, sample.data.expect("fetch byte"));

        let decode = trace
            .micro_samples
            .iter()
            .position(|sample| sample.phase == PhaseKind::Decode && sample.data.is_some())
            .expect("decode sample");
        assert_eq!(events[decode].state.ir, trace.micro_samples[decode].data.unwrap());
    }
}
