#![forbid(unsafe_code)]
pub mod assembler;
pub mod call_stack_contract;
pub mod control_contract;
pub mod control_layout;
pub mod datapath;
pub mod decoder_datapath;
pub mod enemy_shot_bank;
pub mod enemy_shot_contract;
pub mod enemy_shot_layout;
pub mod formation_cadence;
pub mod formation_cadence_contract;
pub mod formation_cadence_layout;
pub mod game;
pub mod isa;
pub mod layout;
pub mod logic;
pub mod machine;
pub mod microcode;
pub mod pc_datapath;
pub mod program;
pub mod rng;
pub mod shield_bank;
pub mod shield_contract;
pub mod shield_layout;
pub mod shift_register;
pub mod shift_register_contract;
pub mod shift_register_layout;
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
pub use enemy_shot_bank::{EnemyShotBank, ENEMY_SHOT_SLOTS};
pub use enemy_shot_contract::{
    enemy_shot_ram, validate_enemy_shot_bank_contract, EnemyShotValidation, ENEMY_SHOT_RAM_BASE,
};
pub use enemy_shot_layout::ENEMY_SHOT_NODES;
pub use formation_cadence::{FormationCadence, FormationCadenceEvent};
pub use formation_cadence_contract::{
    validate_formation_cadence_contract, FormationCadenceValidation,
};
pub use formation_cadence_layout::FORMATION_CADENCE_NODES;
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
pub use shield_bank::{
    bit_address as shield_bit_address, bit_mask as shield_bit_mask,
    byte_offset as shield_byte_offset, locate_world as locate_shield_world, ShieldBank, ShieldDamage,
    SHIELD_BYTES_PER, SHIELD_BYTES_PER_ROW, SHIELD_COUNT, SHIELD_H, SHIELD_RAM_BASE,
    SHIELD_TOTAL_BYTES, SHIELD_W, SHIELD_X, SHIELD_Y,
};
pub use shield_contract::{validate_shield_bank_contract, ShieldValidation};
pub use shield_layout::SHIELD_NODES;
pub use shift_register::{ShiftRegister16, ShiftRegisterEventKind};
pub use shift_register_contract::{validate_shift_register_contract, ShiftRegisterValidation};
pub use shift_register_layout::SHIFT_REGISTER_NODES;
pub use sp_trace::{materialize_sp_events, validate_sp_event_stream};
pub use stack_datapath::{derive_stack_datapath, StackDatapathEvent, StackDatapathKind};
pub use topology::{Group, Link, Node, Rect, SignalKind, Topology};
pub use topology_contract::{validate_final_topology, TopologyValidation};
pub use trace::{
    AluEvent, BusAddressSource, BusDataSource, BusTransactionEvent, BusTransactionKind,
    ControlLatchEvent, ControlLatchKind, FlagEvent, FormationCadenceTraceEvent, FrameState,
    KillEvent, MatchTrace, MicroAddressEvent, MicroCycleEvent, MicroSample, PcEvent, PcEventKind,
    PhaseKind, ProjectileSnapshot, RegisterWriteEvent, ShiftRegisterEvent, SpEvent, SpEventKind,
};
pub use trace_validation::{validate_native_control_authority, NativeTraceValidation};

impl PartialEq for FrameState {
    fn eq(&self, other: &Self) -> bool {
        self.frame == other.frame
            && self.player_x == other.player_x
            && self.fleet_x == other.fleet_x
            && self.fleet_y == other.fleet_y
            && self.fleet_dir == other.fleet_dir
            && self.player_shot == other.player_shot
            && self.enemy_shots == other.enemy_shots
            && self.alive_rows == other.alive_rows
            && self.score == other.score
            && self.lives == other.lives
            && self.pc == other.pc
            && self.vram_checksum == other.vram_checksum
    }
}

impl Eq for FrameState {}

#[must_use]
pub fn build_topology() -> Topology {
    let mut topology = topology::build_topology();
    layout::apply_visual_layout(&mut topology);
    control_layout::inject_internal_control_lines(&mut topology);
    shift_register_layout::inject_shift_register(&mut topology);
    formation_cadence_layout::inject_formation_cadence(&mut topology);
    enemy_shot_layout::inject_enemy_shot_bank(&mut topology);
    shield_layout::inject_shield_bank(&mut topology);
    topology
}
