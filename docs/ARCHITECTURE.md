# Architecture

Leader Invader is a deterministic simulation + replay compiler, not a hand-authored animation.

GitHub renders README SVGs as images, so the simulation runs before publication and the generated SVG is a self-contained declarative replay.

```text
seed
  |
  v
leader-core ---------> MatchTrace ----------> leader-svg
  |                        |                     |
  |                        |                     +--> generated/Leader.svg
  |                        +------------------------> generated/trace.json
  |
  +--> ROM + CPU + µROM + RAM + VRAM + devices + bot
```

## Crates

### `leader-core`

Owns semantics and determinism.

- repository-owned deterministic RNG;
- autonomous game state and bot;
- assembled byte-addressed ROM program;
- semantic CPU state including PC, SP, MAR, MDR, IR, flags and register file;
- physical µPC / microsequencer and 24-bit control words;
- real operand, register-select, address, branch-condition and PC-mux latches;
- ripple-carry ALU, PC incrementer and SP increment/decrement networks;
- byte-addressed work RAM and stack window;
- real 128×96 1-bit VRAM generation;
- native bus, ALU, register, PC and microcycle event streams;
- stable 300+ node topology;
- `MatchTrace`, the contract between simulation and presentation.

### `leader-svg`

Owns presentation only.

- node-by-node construction;
- wire draw animation;
- trace-driven activity;
- cinematic camera through animated `viewBox`;
- vector gameplay replay and kill events;
- reduced-motion fallback.

The renderer may compress or sample a real trace. It may not invent machine activity.

### `leader-cli`

Owns the reproducible build boundary and higher-fidelity overlays.

```bash
cargo run -p leader-cli -- render --seed <seed> --output generated/Leader.svg
```

The CLI layers native PC, decoder, microcode, stack and timing overlays onto the base SVG. CI calls the same path. There is no hidden production renderer.

## Fidelity contract

The project evolves through explicit levels rather than pretending all visible gates are already electrically simulated.

**F1 — causal micro-architecture — complete.** Real game/memory/VRAM state transitions emit causal machine events; visible nodes light from those events.

**F2 — ISA execution — complete.** The autonomous game loop lives in assembled ROM bytecode. Fetch/decode/control flow, stack behavior, registers, flags and memory are semantic execution state. Corrupting ROM breaks the match causally.

**F3 — bit-accurate datapath — active.** Critical visible control and datapath paths are being converted from descriptive replay into same-tick authority. The current implementation already includes:

- native T0/T1/T2 microcycles;
- a real µPC with fetch, sequential, dispatch, routine-call and routine-return transitions;
- a physical `256 × 24` control ROM model;
- `MAR_LOAD`, `MDR_LOAD`, `IR_LOAD` and `PC_INC` gating fetch/shared routines;
- authoritative `OPERAND_A_LOAD`, `OPERAND_B_LOAD`, `ALU_OP_LOAD` and `FLAGS_LOAD`;
- authoritative `REG_SELECT` with latched register sources/destination;
- authoritative `ADDR_LO_LOAD`, `ADDR_HI_LOAD`, `CONDITION_LOAD` and `PC_SELECT`;
- gated architectural commits for register writes, memory writes and PC loads;
- native bus ownership, exact ALU, register-write and PC event streams;
- native decode visualization from `DecodeLatch` microcycles;
- stack visualization driven from native stack-window bus transactions.

A visible critical control line is considered complete only when removing or mis-selecting that line changes or prevents the real machine result in the same instruction.

No random decorative glow is allowed at any level.

## Control ROM

The execute space occupies `0x80..0xFC` as 25 opcode slots × 5 physical rows. Shared routines occupy dedicated regions for opcode fetch, operand fetch, memory read and memory write.

The physical word contains eight external controls:

```text
REGW ALU MEMR MEMW PCLD STACK WAIT HALT
```

and sixteen internal controls:

```text
MAR_LOAD MDR_LOAD IR_LOAD PC_INC
OPERAND_A_LOAD OPERAND_B_LOAD ALU_OP_LOAD FLAGS_LOAD
ADDR_LO_LOAD ADDR_HI_LOAD CONDITION_LOAD PC_SELECT
REG_SELECT BUS_ADDRESS_ENABLE BUS_DATA_ENABLE ARCH_COMMIT
```

`ControlWord::bits24()` is the canonical packed representation. The complete 24-bit word is persisted in `MicroAddressEvent.control_bits` and consumed by the SVG microcode overlay.

## Determinism

Every source of simulation nondeterminism derives from the explicit seed.

- no wall clock inside `leader-core`;
- no OS RNG;
- no thread scheduling in semantic execution;
- stable text seed hash and SplitMix64 sequence owned by the repository;
- same source + same seed = same match semantics.

This makes a commit SHA a natural production seed.

## Trace model

The trace now records several native time-aligned streams rather than treating `MicroSample` as universal truth.

1. `FrameState` — stable game/display checkpoints.
2. `MicroCycleEvent` — exact CPU T-state latch snapshots, including native `DecodeLatch` events.
3. `MicroAddressEvent` — µPC transitions and the selected 24-bit control word.
4. `BusTransactionEvent` — address/data ownership and native fetch/read/write/input/DMA/scanout transactions.
5. `AluEvent` — exact ALU operation, operands, result and carry chain.
6. `RegisterWriteEvent` — actual architectural register mutation.
7. `PcEvent` — exact ripple increment and selected PC loads.
8. `MicroSample` — legacy semantic samples retained for historical compatibility and coarse cinematic activity.

Renderer policy is **native-first**. A renderer uses the dedicated native stream whenever one exists and falls back to semantic reconstruction only for traces produced before that native stream existed.

Tests deliberately clear `micro_samples` and prove native bus, decoder, ALU, register, PC and stack visual derivations remain valid.

## CPU authority

### Fetch

The selected µROM row must physically enable each mutation:

```text
FETCH_T0: MAR_LOAD + BUS_ADDRESS_ENABLE
FETCH_T1: MEMR + MDR_LOAD + PC_INC + BUS_DATA_ENABLE
FETCH_T2: IR_LOAD
```

The same discipline applies to shared operand, read and write routines.

### ALU

Five-row ALU instructions load explicit CPU latches for operand A, operand B and operation selection. Propagation consumes only those latches. Flags require `FLAGS_LOAD`; register mutation requires the final write-enable/commit row.

Compact `LDI/ADDI` retain three execute rows while following the same rule: A latch → B/op latch → ALU/flags/write commit.

### Register file

`REG_SELECT` loads real register-selection latches. The selected A register is also the architectural write-back destination; the executor does not retain a hidden destination register local through the instruction.

### Control flow

`ADDR_LO_LOAD` and `ADDR_HI_LOAD` build the target address. Conditional branches first latch their condition, then `PC_SELECT` selects the target or explicitly clears the selection. Final PC mutation requires `PCLD + ARCH_COMMIT`.

RET restores its low/high return bytes through the same address latches before selecting the PC input.

### Stack

`STACK` gates PUSH/POP. SP movement uses exact 16-bit ripple decrement/increment logic and the shared memory routines perform the corresponding stack transactions.

## Video path

The game state is mirrored into work RAM. Each frame is rasterized into a simulated 128×96 one-bit VRAM region (1536 bytes), checksummed, then exposed through native DMA/scanout/VBlank activity.

The SVG renders compact vector sprites from frame checkpoints to keep the artifact small. A later renderer can replay retained VRAM deltas directly without changing CPU semantics.

## Camera

The README cannot offer interactive mouse pan/zoom because GitHub embeds the SVG as an image. Camera motion is generated declaratively by animating the nested SVG `viewBox`.

The camera sequence follows construction subsystem-by-subsystem, reveals the completed machine, follows boot activity, then moves into the display for the autonomous match before returning to the whole machine.

A later WASM explorer can reuse the same topology and deterministic machine with real drag-pan, wheel zoom and node inspection.

## Scaling strategy

A naïve keyframe for every gate on every cycle would create an unusably large SVG. The renderer therefore compresses only presentation, never semantics:

- one construction animation per node;
- one draw animation per wire;
- bounded sampling of native micro-events;
- shared fleet/player transforms;
- sparse alien kill events;
- long-lived declarative camera track.

## Failure policy

Generation fails rather than publishing misleading output if:

- the bot cannot clear the match within the frame budget;
- topology links reference missing nodes;
- generated SVG contains script/event-handler/JavaScript URLs;
- generated XML is malformed;
- deterministic tests diverge;
- a causal row-selection test observes mutation without the corresponding physical control enable.
