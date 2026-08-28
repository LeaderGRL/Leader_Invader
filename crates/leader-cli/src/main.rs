#![forbid(unsafe_code)]

mod alu_overlay;
mod bus_overlay;
mod control_state_overlay;
mod control_word_overlay;
mod decoder_overlay;
mod director;
mod enemy_shot_overlay;
mod flags_overlay;
mod formation_cadence_overlay;
mod microcode_overlay;
mod microcycle_overlay;
#[cfg(test)]
mod native_pipeline_tests;
mod pc_overlay;
mod register_overlay;
mod render_contract;
mod shield_overlay;
mod shift_register_overlay;
mod stack_overlay;
mod timing_overlay;

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use leader_core::{
    build_topology, validate_call_stack_contract, validate_enemy_shot_bank_contract,
    validate_final_topology, validate_formation_cadence_contract, validate_memory_map_contract,
    validate_native_control_authority, validate_shield_bank_contract,
    validate_shift_register_contract, validate_sp_event_stream, MatchTrace, MicroCycleKind, Machine,
    Topology,
};
use leader_svg::{render, RenderConfig};

fn main() {
    if let Err(error) = run() {
        eprintln!("leader-cli: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "render".to_owned());
    let options = Options::parse(args.collect())?;
    match command.as_str() {
        "render" => render_cmd(options),
        "trace" => trace_cmd(options),
        "stats" => stats_cmd(options),
        "help" | "--help" | "-h" => {
            help();
            Ok(())
        }
        other => Err(format!("unknown command '{other}'")),
    }
}

#[derive(Debug, Clone)]
struct Options {
    seed: String,
    output: PathBuf,
    max_frames: u32,
}

impl Options {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let (mut seed, mut output, mut max_frames) = (
            "leader-invader-dev".to_owned(),
            PathBuf::from("generated/Leader.svg"),
            5000_u32,
        );
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--seed" => {
                    index += 1;
                    seed = args.get(index).ok_or("--seed requires value")?.clone();
                }
                "--output" | "-o" => {
                    index += 1;
                    output = PathBuf::from(args.get(index).ok_or("--output requires path")?);
                }
                "--max-frames" => {
                    index += 1;
                    max_frames = args
                        .get(index)
                        .ok_or("--max-frames requires value")?
                        .parse()
                        .map_err(|error| format!("invalid frame count: {error}"))?;
                }
                other => return Err(format!("unknown option '{other}'")),
            }
            index += 1;
        }
        Ok(Self {
            seed,
            output,
            max_frames,
        })
    }
}

fn run_native_trace(seed: &str, max_frames: u32) -> MatchTrace {
    Machine::run_match(seed, max_frames)
}

fn render_native_base(topology: &Topology, trace: &MatchTrace, config: RenderConfig) -> String {
    let mut native_base = trace.clone();
    native_base.micro_samples.clear();
    render(topology, &native_base, config)
}

fn validate_native_trace(
    trace: &MatchTrace,
) -> Result<
    (
        leader_core::NativeTraceValidation,
        leader_core::CallStackValidation,
        usize,
        leader_core::ShiftRegisterValidation,
        leader_core::FormationCadenceValidation,
        leader_core::EnemyShotValidation,
        leader_core::ShieldValidation,
        leader_core::MemoryMapValidation,
    ),
    String,
> {
    let native = validate_native_control_authority(trace)
        .map_err(|error| format!("native F3 trace invalid: {error}"))?;
    let sp_events = validate_sp_event_stream(trace)
        .map_err(|error| format!("native SP trace invalid: {error}"))?;
    let call_stack = validate_call_stack_contract(trace)
        .map_err(|error| format!("CALL/RET stack contract invalid: {error}"))?;
    let shift = validate_shift_register_contract(trace)
        .map_err(|error| format!("native M3 shift-register trace invalid: {error}"))?;
    let cadence = validate_formation_cadence_contract(trace)
        .map_err(|error| format!("native M3 formation cadence trace invalid: {error}"))?;
    let enemy_shots = validate_enemy_shot_bank_contract(trace)
        .map_err(|error| format!("native M3 enemy-shot bank invalid: {error}"))?;
    let shields = validate_shield_bank_contract(trace)
        .map_err(|error| format!("native M3 shield bank invalid: {error}"))?;
    let memory_map = validate_memory_map_contract(trace)
        .map_err(|error| format!("native memory-map contract invalid: {error}"))?;
    Ok((
        native,
        call_stack,
        sp_events,
        shift,
        cadence,
        enemy_shots,
        shields,
        memory_map,
    ))
}

fn render_cmd(options: Options) -> Result<(), String> {
    let topology = build_topology();
    let topology_validation = validate_final_topology(&topology)
        .map_err(|error| format!("final topology invalid: {error}"))?;
    let trace = run_native_trace(&options.seed, options.max_frames);
    if !trace.finished {
        return Err(format!(
            "match did not clear within {} frames",
            options.max_frames
        ));
    }
    let (validation, call_stack, sp_events, shift, cadence, enemy_shots, shields, memory_map) =
        validate_native_trace(&trace)?;
    let config = RenderConfig::default();
    let svg = render_native_base(&topology, &trace, config);
    let svg = director::apply_camera(svg, &topology, &trace, config);
    let svg = pc_overlay::apply(svg, &topology, &trace, config);
    let svg = decoder_overlay::apply(svg, &topology, &trace, config);
    let svg = microcode_overlay::apply(svg, &topology, &trace, config);
    let svg = control_word_overlay::apply(svg, &topology, &trace, config);
    let svg = control_state_overlay::apply(svg, &topology, &trace, config);
    let svg = microcycle_overlay::apply(svg, &topology, &trace, config);
    let svg = alu_overlay::apply(svg, &topology, &trace, config);
    let svg = flags_overlay::apply(svg, &topology, &trace, config);
    let svg = register_overlay::apply(svg, &topology, &trace, config);
    let svg = bus_overlay::apply(svg, &topology, &trace, config);
    let svg = stack_overlay::apply(svg, &topology, &trace, config);
    let svg = formation_cadence_overlay::apply(svg, &topology, &trace, config);
    let svg = shift_register_overlay::apply(svg, &topology, &trace, config);
    let svg = enemy_shot_overlay::apply(svg, &topology, &trace, config);
    let svg = shield_overlay::apply(svg, &topology, &trace, config);
    let svg = timing_overlay::apply(svg, &topology, &trace, config);
    let svg_validation = render_contract::validate_native_svg_contract(&svg)
        .map_err(|error| format!("native SVG contract invalid: {error}"))?;
    write(&options.output, svg.as_bytes())?;
    let trace_path = options.output.with_file_name("trace.json");
    write(&trace_path, trace.to_json().as_bytes())?;
    println!(
        "rendered {} nodes / {} links / {} frames / {} kills / {} verified µwords / {} PC increments / {} flag latches / {} control latches / {} SP events / {} CALL pairs / {} shift events / {} cadence clocks / {} cadence ticks / {} enemy-shot writes / {} max concurrent shots / {} shield-caused shot clears / {} shield damages / {} shield pixels left / {} mapped bus transactions / {} native overlays / {} bytes -> {}",
        topology_validation.nodes,
        topology_validation.links,
        trace.total_frames,
        trace.kills.len(),
        validation.micro_words,
        validation.pc_increments,
        validation.flag_events,
        validation.control_latches,
        sp_events,
        call_stack.call_pairs,
        shift.data_writes + shift.offset_writes + shift.reads,
        cadence.clocks,
        cadence.ticks,
        enemy_shots.ram_writes,
        enemy_shots.max_active,
        enemy_shots.shield_clears,
        shields.damages,
        shields.pixels_after,
        memory_map.addressed_transactions,
        svg_validation.overlay_groups,
        svg_validation.bytes,
        options.output.display()
    );
    Ok(())
}

fn trace_cmd(mut options: Options) -> Result<(), String> {
    if options.output == PathBuf::from("generated/Leader.svg") {
        options.output = PathBuf::from("generated/trace.json");
    }
    let trace = run_native_trace(&options.seed, options.max_frames);
    let (_, _, _, shift, cadence, enemy_shots, shields, memory_map) =
        validate_native_trace(&trace)?;
    write(&options.output, trace.to_json().as_bytes())?;
    println!(
        "frames={} kills={} flag_events={} control_latch_events={} sp_events={} shift_events={} cadence_clocks={} cadence_ticks={} enemy_shot_spawns={} enemy_shot_moves={} enemy_shot_clears={} enemy_shot_shield_clears={} max_enemy_shots={} shield_damages={} shield_player={} shield_enemy={} mapped_bus_transactions={} clear={}",
        trace.total_frames,
        trace.kills.len(),
        trace.flag_events.len(),
        trace.control_latch_events.len(),
        trace.sp_events.len(),
        shift.data_writes + shift.offset_writes + shift.reads,
        cadence.clocks,
        cadence.ticks,
        enemy_shots.spawns,
        enemy_shots.moves,
        enemy_shots.clears,
        enemy_shots.shield_clears,
        enemy_shots.max_active,
        shields.damages,
        shields.player_damages,
        shields.enemy_damages,
        memory_map.addressed_transactions,
        trace.finished
    );
    if trace.finished {
        Ok(())
    } else {
        Err("trace hit frame limit".to_owned())
    }
}

fn stats_cmd(options: Options) -> Result<(), String> {
    let topology = build_topology();
    let topology_validation = validate_final_topology(&topology)
        .map_err(|error| format!("final topology invalid: {error}"))?;
    let trace = run_native_trace(&options.seed, options.max_frames);
    let (validation, call_stack, sp_events, shift, cadence, enemy_shots, shields, memory_map) =
        validate_native_trace(&trace)?;
    let decode_latches = trace
        .micro_cycles
        .iter()
        .filter(|event| event.kind == MicroCycleKind::DecodeLatch)
        .count();
    println!("topology.nodes={}", topology_validation.nodes);
    println!("topology.links={}", topology_validation.links);
    println!("trace.frames={}", trace.frames.len());
    println!("trace.micro_samples={}", trace.micro_samples.len());
    println!("trace.micro_cycles={}", trace.micro_cycles.len());
    println!("trace.decode_latches={decode_latches}");
    println!("trace.micro_addresses={}", trace.micro_addresses.len());
    println!("trace.bus_transactions={}", trace.bus_transactions.len());
    println!("trace.alu_events={}", trace.alu_events.len());
    println!("trace.flag_events={}", trace.flag_events.len());
    println!("trace.control_latch_events={}", trace.control_latch_events.len());
    println!("trace.formation_cadence_events={}", trace.formation_cadence_events.len());
    println!("trace.shift_register_events={}", trace.shift_register_events.len());
    println!("trace.register_writes={}", trace.register_writes.len());
    println!("trace.pc_events={}", trace.pc_events.len());
    println!("trace.sp_events={}", trace.sp_events.len());
    println!("trace.native_verified_micro_words={}", validation.micro_words);
    println!("trace.native_verified_decode_latches={}", validation.decode_latches);
    println!("trace.native_verified_alu_events={}", validation.alu_events);
    println!("trace.native_verified_flag_events={}", validation.flag_events);
    println!("trace.native_verified_control_latches={}", validation.control_latches);
    println!("trace.native_verified_register_writes={}", validation.register_writes);
    println!("trace.native_verified_pc_increments={}", validation.pc_increments);
    println!("trace.native_verified_pc_loads={}", validation.pc_loads);
    println!("trace.native_verified_sp_events={}", validation.sp_events);
    println!("trace.native_verified_rom_fetches={}", validation.rom_fetches);
    println!("trace.native_verified_cpu_reads={}", validation.cpu_reads);
    println!("trace.native_verified_cpu_writes={}", validation.cpu_writes);
    println!("trace.native_verified_sp_bus_contract={sp_events}");
    println!("trace.shift_data_writes={}", shift.data_writes);
    println!("trace.shift_offset_writes={}", shift.offset_writes);
    println!("trace.shift_reads={}", shift.reads);
    println!("trace.cadence_clocks={}", cadence.clocks);
    println!("trace.cadence_ticks={}", cadence.ticks);
    println!("trace.cadence_divisor3={}", cadence.divisor3);
    println!("trace.cadence_divisor2={}", cadence.divisor2);
    println!("trace.cadence_divisor1={}", cadence.divisor1);
    println!("trace.cadence_movement_transactions={}", cadence.movement_transactions);
    println!("trace.enemy_shot_transitions={}", enemy_shots.transitions);
    println!("trace.enemy_shot_ram_writes={}", enemy_shots.ram_writes);
    println!("trace.enemy_shot_spawns={}", enemy_shots.spawns);
    println!("trace.enemy_shot_moves={}", enemy_shots.moves);
    println!("trace.enemy_shot_clears={}", enemy_shots.clears);
    println!("trace.enemy_shot_shield_clears={}", enemy_shots.shield_clears);
    println!("trace.enemy_shot_max_active={}", enemy_shots.max_active);
    println!("trace.enemy_shot_slots_used={}", enemy_shots.slots_used);
    println!("trace.shield_damages={}", shields.damages);
    println!("trace.shield_player_damages={}", shields.player_damages);
    println!("trace.shield_enemy_damages={}", shields.enemy_damages);
    println!("trace.shields_damaged={}", shields.shields_damaged);
    println!("trace.shield_pixels_before={}", shields.pixels_before);
    println!("trace.shield_pixels_after={}", shields.pixels_after);
    println!("trace.memory_map_addressed={}", memory_map.addressed_transactions);
    println!("trace.memory_map_rom={}", memory_map.rom);
    println!("trace.memory_map_ram={}", memory_map.ram);
    println!("trace.memory_map_vram={}", memory_map.vram);
    println!("trace.memory_map_mmio={}", memory_map.mmio);
    println!("trace.call_pairs={}", call_stack.call_pairs);
    println!("trace.return_pairs={}", call_stack.return_pairs);
    println!("trace.call_stack_bytes={}", call_stack.stack_bytes);
    println!("trace.kills={}", trace.kills.len());
    println!("trace.finished={}", trace.finished);
    println!("trace.final_score={}", trace.final_score);
    Ok(())
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    fs::write(path, bytes).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn help() {
    println!("leader-cli\n\nrender [--seed TEXT] [--output PATH] [--max-frames N]\ntrace  [--seed TEXT] [--output PATH]\nstats  [--seed TEXT]\n\nSame source + same seed => same deterministic replay.");
}
