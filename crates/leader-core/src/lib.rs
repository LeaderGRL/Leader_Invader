#![forbid(unsafe_code)]
pub mod assembler;
pub mod call_stack_contract;
pub mod control_contract;
pub mod control_layout;
pub mod datapath;
pub mod decoder_datapath;
pub mod game;
pub mod isa;
pub mod layout;
pub mod logic;
pub mod machine;
pub mod microcode;
pub mod pc_datapath;
pub mod program;
pub mod rng;
pub mod shift_register;
pub mod sp_trace;
pub mod stack_datapath;
pub mod topology;
pub mod topology_contract;
pub mod trace;
pub mod trace_validation;

pub use call_stack_contract::{validate_call_stack_contract, CallStackValidation};
pub use control_contract::{
    control_topology_violations, physical_control_lines, physically_used_control_mask,
    PhysicalControlLine, EXTERNAL_CONTROL_NODES,
};
pub use control_layout::INTERNAL_CONTROL_NODES;
pub use datapath::{
    bit16, bit8, derive_alu_datapath, derive_bus_datapath, derive_datapath,
    derive_register_datapath, AluDatapathEvent, BusAddressOwner, BusCycle, BusDataOwner,
    BusDatapathEvent, DatapathEvent, DatapathState, RegisterDatapathEvent,
};
pub use decoder_datapath::{derive_decoder_datapath, DecoderDatapathEvent};
pub use isa::{Cpu, Flags, MicroCycleKind, MicroPhase, PcSource, Reg, StepOutcome};
pub use logic::{
    logic_trace, ripple_add, ripple_decrement16, ripple_increment16, ripple_sub, AluOp, AluTrace,
    Decrement16Trace, PcIncrementTrace,
};
pub use machine::Machine;
pub use microcode::{
    control_word, control_word_at, decode as decode_microcode, execute_address,
    execute_control_step, execute_row_kind, execute_step_address, opcode_slot, uaddr, ControlWord,
    ExecuteRowKind, MicroAddressSource, MicroAddressTransition, MicroInstruction, MicroOp,
    MicroSequencer,
};
pub use pc_datapath::{derive_pc_datapath, PcDatapathEvent, PcDatapathKind};
pub use shift_register::{ShiftRegister16, ShiftRegisterEventKind};
pub use sp_trace::{materialize_sp_events, validate_sp_event_stream};
pub use stack_datapath::{derive_stack_datapath, StackDatapathEvent, StackDatapathKind};
pub use topology::{Group, Link, Node, Rect, SignalKind, Topology};
pub use topology_contract::{validate_final_topology, TopologyValidation};
pub use trace::{
    AluEvent, BusAddressSource, BusDataSource, BusTransactionEvent, BusTransactionKind,
    ControlLatchEvent, ControlLatchKind, FlagEvent, FrameState, KillEvent, MatchTrace,
    MicroAddressEvent, MicroCycleEvent, MicroSample, PcEvent, PcEventKind, PhaseKind,
    ProjectileSnapshot, RegisterWriteEvent, ShiftRegisterEvent, SpEvent, SpEventKind,
};
pub use trace_validation::{validate_native_control_authority, NativeTraceValidation};

#[must_use]
pub fn build_topology() -> Topology {
    let mut topology = topology::build_topology();
    layout::apply_visual_layout(&mut topology);
    control_layout::inject_internal_control_lines(&mut topology);
    topology
}
