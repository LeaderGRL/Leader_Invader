#![forbid(unsafe_code)]
pub mod assembler;
pub mod datapath;
pub mod game;
pub mod isa;
pub mod layout;
pub mod logic;
pub mod machine;
pub mod pc_datapath;
pub mod program;
pub mod rng;
pub mod topology;
pub mod trace;

pub use datapath::{
    bit16, bit8, derive_alu_datapath, derive_bus_datapath, derive_datapath,
    derive_decoder_datapath, derive_register_datapath, AluDatapathEvent, BusAddressOwner,
    BusCycle, BusDataOwner, BusDatapathEvent, DatapathEvent, DatapathState,
    DecoderDatapathEvent, RegisterDatapathEvent,
};
pub use isa::{Cpu, Flags, PcSource, Reg, StepOutcome};
pub use logic::{
    logic_trace, ripple_add, ripple_increment16, ripple_sub, AluOp, AluTrace, PcIncrementTrace,
};
pub use machine::Machine;
pub use pc_datapath::{derive_pc_datapath, PcDatapathEvent, PcDatapathKind};
pub use topology::{Group, Link, Node, Rect, SignalKind, Topology};
pub use trace::{
    AluEvent, FrameState, KillEvent, MatchTrace, MicroSample, PcEvent, PcEventKind, PhaseKind,
    ProjectileSnapshot, RegisterWriteEvent,
};

#[must_use]
pub fn build_topology() -> Topology {
    let mut topology = topology::build_topology();
    layout::apply_visual_layout(&mut topology);
    topology
}
