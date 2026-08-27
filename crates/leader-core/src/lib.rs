#![forbid(unsafe_code)]
pub mod assembler;
pub mod game;
pub mod isa;
pub mod layout;
pub mod machine;
pub mod program;
pub mod rng;
pub mod topology;
pub mod trace;

pub use isa::{Cpu, Flags, Reg, StepOutcome};
pub use machine::Machine;
pub use topology::{Group, Link, Node, Rect, SignalKind, Topology};
pub use trace::{FrameState, KillEvent, MatchTrace, MicroSample, PhaseKind, ProjectileSnapshot};

#[must_use]
pub fn build_topology() -> Topology {
    let mut topology = topology::build_topology();
    layout::apply_visual_layout(&mut topology);
    topology
}
