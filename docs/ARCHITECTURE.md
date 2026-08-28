# Architecture

Leader Invader is a deterministic simulation + replay compiler, not a hand-authored animation.

GitHub renders README SVGs as images, so the simulation runs before publication and the generated SVG is a self-contained declarative replay.

```text
seed
  |
  v
leader-core ---------> MatchTrace ----------> leader-svg base
  |                        |                       |
  |                        |                       v
  |                        +---------------> leader-cli native F3 overlays
  |                                                |
  |                                                v
  +--> ROM + CPU + µROM + RAM + VRAM       generated/Leader.svg
       + validated physical topology
```

## Crates

### `leader-core`

Owns semantics, physical control contracts and determinism.

- repository-owned deterministic RNG;
- autonomous game state and bot;
- assembled byte-addressed ROM program;
- CPU state including PC, SP, MAR, MDR, IR, flags and register file;
- physical µPC / microsequencer and 24-bit control words;
- real operand, register-select, address, branch-condition and PC-mux latches;
- ripple-carry ALU, PC incrementer and SP increment/decrement networks;
- byte-addressed work RAM and stack window;
- real 128×96 1-bit VRAM generation;
- first-class microcycle, bus, ALU, flag, control-latch, register, PC and SP event streams;
- physical topology for all 24 µROM output lines and their consumers;
- CALL/RET combinational return-path contract;
- trace/control, SP and final-topology validators;
- `MatchTrace`, the contract between simulation and presentation.

### `leader-svg`

Owns the reusable declarative base presentation.

- node-by-node construction;
- wire draw animation;
- cinematic camera substrate;
- vector gameplay replay and kill events;
- reduced-motion fallback;
- historical coarse `MicroSample` activity for backward-compatible library callers.

The production CLI removes `micro_samples` from the base-render copy, so the historical coarse activity layer is absent from the README artifact.

### `leader-cli`

Owns the reproducible production boundary and all high-fidelity F3 presentation.

```bash
cargo run -p leader-cli -- render --seed <seed> --output generated/Leader.svg
```

Production layers native overlays for:

- PC;
- decoder;
- µPC / microcode;
- all 24 physical control outputs;
- exact control-state latch values;
- T-state microcycle snapshots;
- ALU;
- flags;
- register writes;
- bus transactions;
- stack/SP;
- timing.

`render`, `trace` and `stats` consume the first-class streams returned directly by `Machine::run_match()`. They do **not** materialize SP from the bus and do not depend on `MicroSample` semantic reconstruction.

The CLI validates the completed trace/topology/SVG before writing it. CI calls this exact path; there is no hidden renderer.

## Fidelity contract

**F1 — causal micro-architecture — complete.** Real game/memory/VRAM state transitions drive the replay.

**F2 — ISA execution — complete.** The autonomous game loop lives in assembled ROM bytecode. Fetch/decode/control flow, stack behavior, registers, flags and memory are execution state. Corrupting ROM breaks the match causally.

**F3 — physical datapath authority — complete.** Every visible critical CPU/control path is driven from native same-tick machine state or a physically justified combinational path. The completed implementation includes:

- native T0/T1/T2 microcycles;
- a real µPC with fetch, sequential, dispatch, routine-call and routine-return transitions;
- a physical `256 × 24` control-ROM model;
- 24 distinct visible control outputs, each with a unique bit/node/label contract;
- physical wiring from internal control outputs to MAR/MDR/IR, PC increment, ALU/select/flag paths, target/condition/PC/register state, bus enables and commit;
- explicit visible `ADDR LO`, `ADDR HI`, `COND`, `PC SEL` and `REG SEL` state nodes with exact native values;
- a visible combinational `RETURN BYTE` mux connecting stable PC high/low bytes to CALL stack writes and the stack data path back to RET address latches;
- gated architectural commits for register writes, memory writes and PC loads;
- first-class native bus ownership, exact ALU, flag, control-latch, register-write, PC and SP streams;
- native decode from `DecodeLatch` microcycles;
- direct CPU-native SP ripple transitions rather than production reconstruction from bus activity;
- a production SVG whose high-fidelity hardware presentation is independent of `MicroSample` and stack fallback materialization.

A visible critical control line is complete only when removing or mis-selecting that line changes, prevents or invalidates the real machine result in the same instruction.

No random decorative glow is allowed at any level.

## Physical control ROM

The execute space occupies `0x80..0xFC` as 25 opcode slots × 5 rows. Shared routines occupy dedicated regions for opcode fetch, operand fetch, memory read and memory write.

The eight external controls are:

```text
REGW ALU MEMR MEMW PCLD STACK WAIT HALT
```

The sixteen internal controls are:

```text
MAR_LOAD MDR_LOAD IR_LOAD PC_INC
OPERAND_A_LOAD OPERAND_B_LOAD ALU_OP_LOAD FLAGS_LOAD
ADDR_LO_LOAD ADDR_HI_LOAD CONDITION_LOAD PC_SELECT
REG_SELECT BUS_ADDRESS_ENABLE BUS_DATA_ENABLE ARCH_COMMIT
```

`ControlWord::bits24()` is the canonical packed representation. `MicroAddressEvent.control_bits` carries that exact word into the native trace.

`physical_control_lines()` defines the stable bit → node → label mapping for all 24 outputs. `physically_used_control_mask()` scans the complete µROM and tests require every bit to be exercised somewhere by a real microinstruction.

## Native trace model

The trace uses time-aligned first-class streams rather than treating `MicroSample` as universal truth.

1. `FrameState` — stable game/display checkpoints.
2. `MicroCycleEvent` — exact T-state snapshots with PC/MAR/MDR/IR and native `DecodeLatch` events.
3. `MicroAddressEvent` — µPC transitions and selected 24-bit control word.
4. `BusTransactionEvent` — address/data ownership and fetch/read/write/input/DMA/scanout transactions.
5. `AluEvent` — operation, operands, result and exact carry chain.
6. `FlagEvent` — exact architectural Z/C/L state after a physically enabled flag latch.
7. `ControlLatchEvent` — exact kind/value/validity for `ADDR LO`, `ADDR HI`, `COND`, `PC SEL` and `REG SEL`.
8. `RegisterWriteEvent` — architectural register mutation.
9. `PcEvent` — exact ripple increments and selected PC loads.
10. `SpEvent` — exact PUSH/POP mutation, stack address/data and 16-bit ripple chain.
11. `MicroSample` — historical compatibility only; not a production authority source.

`Machine::run_match()` emits all first-class streams directly. The production renderer clears `micro_samples` only from the reusable base-render copy, then consumes the native streams directly. Pipeline tests require byte-for-byte identical F3 output after `trace.micro_samples.clear()` and stack tests prove `SpEvent` remains authoritative even after bus transactions are removed.

Historical helpers may still reconstruct old traces from bus or `MicroSample` data when the newer stream is absent. Those compatibility branches are not invoked by the production CLI.

## CPU authority

### Fetch and memory routines

The selected µROM row physically enables each mutation:

```text
FETCH_T0: MAR_LOAD + BUS_ADDRESS_ENABLE
FETCH_T1: MEMR + MDR_LOAD + PC_INC + BUS_DATA_ENABLE
FETCH_T2: IR_LOAD
```

Shared operand/read/write routines follow the same rule. Native validation ties `ROM_FETCH`, `CPU_READ` and `CPU_WRITE` transactions back to their required physical rows.

Every native `PcEvent::Increment` is independently tied to the preceding `FETCH_T1`/`OPERAND_T1` row carrying `PC_INC`.

### ALU and flags

Five-row ALU instructions load explicit CPU latches for A, B and operation selection. Propagation consumes only those latches. Flags mutate only when `FLAGS_LOAD` is physically active; register mutation requires the final write-enable/commit row.

The SVG exposes operation, lhs, rhs, effective rhs, result and carry chain as data attributes while lighting the exact full-adder path. A separate flag overlay exposes the actual Z/C/L values latched by the CPU.

### Register file and control-state latches

`REG_SELECT` loads real register-selection state. The selected A register is the authoritative architectural write-back destination. Native register events expose before/after values and are validated against `REGW + ARCH_COMMIT`.

`ControlLatchEvent` is emitted at the real mutation point for each visible control-state latch. Each event is validated against its corresponding µROM enable:

```text
ADDR LO  <- ADDR_LO_LOAD
ADDR HI  <- ADDR_HI_LOAD
COND     <- CONDITION_LOAD
PC SEL   <- PC_SELECT
REG SEL  <- REG_SELECT
```

A not-taken branch records `PC SEL` with `valid=false`, explicitly proving stale target selection was cleared.

### Control flow

`ADDR_LO_LOAD` and `ADDR_HI_LOAD` build the target. Conditional branches latch their condition, then `PC_SELECT` chooses the target or clears stale selection. PC mutation requires `PCLD + ARCH_COMMIT`.

PC replay exposes before/after values, source and low-byte ripple carry.

### CALL / RET

A dedicated return-address latch is intentionally **not** part of the architecture. After CALL fetches its two-byte target, the PC already contains `instruction_pc + 3` and remains stable across both stack-write rows.

Therefore the return path is modeled as a combinational hardware path:

```text
PC LO ----\
           > RETURN BYTE MUX -> DATA BUS -> STACK RAM
PC HI ----/          ^
                     |
                   STACK

STACK RAM -> DATA BUS -> ADDR LO / ADDR HI -> PC SELECT -> PC
```

`validate_call_stack_contract()` consumes the first-class SP datapath stream and proves that:

- CALL pushes high then low byte of `PC + 3`;
- RET pops the exact bytes in LIFO order;
- reconstructed return addresses equal the actual `PcEvent::Return` targets;
- call/return pairs are balanced for the complete match.

Corrupting the authoritative `SpEvent` byte fails the contract. Corrupting only the older bus fallback cannot override the first-class SP stream.

This closes the return-address source path without inventing an unnecessary 25th control bit.

### Stack / SP

`STACK` gates PUSH/POP. SP movement uses exact 16-bit ripple decrement/increment logic.

The CPU emits `SpEvent` directly at the mutation boundary:

```text
PUSH: ripple_decrement16(SP) -> SP -> stack write -> SpEvent::Push
POP : stack read -> ripple_increment16(SP) -> SP -> SpEvent::Pop
```

Each event contains the complete ripple trace, address, byte, PC and control label. The central native-control validator requires a local execute µROM word carrying `STACK`, while `validate_sp_event_stream()` cross-checks each event against its exact stack-window bus transaction and ordering.

`materialize_sp_events()` remains only as a compatibility fallback for historical traces with no first-class SP stream. Production does not call it.

## Physical topology contract

The final injected topology is validated before production rendering.

`validate_final_topology()` requires:

- unique node IDs;
- unique link IDs;
- every node to belong to an existing group and stay inside that group;
- every link source and destination to exist;
- the complete 24-line control topology to be valid;
- required F3 state nodes and the `RETURN BYTE` mux to exist.

This validation runs on the actual final topology after visual-layout and F3 hardware injection, not only on the historical base topology.

## Native control-authority contract

`validate_native_control_authority()` ties the trace back to the selected physical µROM word.

It validates:

- every traced µROM word equals `control_word_at(µADDR, opcode).bits24()`;
- `DecodeLatch` requires `IR_LOAD`;
- ALU propagation requires `ALU`;
- `FlagEvent` requires `FLAGS_LOAD`;
- every visible `ControlLatchEvent` requires its physical load/select enable;
- register write requires `REGW + ARCH_COMMIT`;
- PC increment requires `PC_INC` on the exact fetch/operand row;
- PC load requires `PCLD + ARCH_COMMIT`;
- SP mutation requires `STACK`;
- ROM fetch, CPU read and CPU write require their corresponding shared micro-routine rows.

Negative tests remove each class of authority and require validation failure.

## Native SVG contract

The completed production SVG is validated before it is written.

Required native groups include PC, decoder, microcode, 24-bit control bank, control-state latches, microcycles, ALU, flags, register file, bus, stack and timing.

The artifact must also contain structured metadata for the corresponding native values. Generation fails if:

- a required native group or metadata family is missing;
- legacy `class="hot …"` semantic activity is present;
- script or JavaScript URLs are present;
- the SVG exceeds the **5,000,000 byte** production budget.

The current artifact is roughly **3.3 MB**, leaving substantial headroom while preserving inspectability.

## Determinism

Every source of simulation nondeterminism derives from the explicit seed.

- no wall clock inside `leader-core`;
- no OS RNG;
- no thread scheduling in semantic execution;
- stable text seed hash and SplitMix64 sequence owned by the repository;
- same source + same seed = same match semantics.

A commit SHA is therefore a natural production seed.

## Video path

Game state is mirrored into work RAM. Each frame is rasterized into a simulated 128×96 one-bit VRAM region, checksummed, then exposed through native DMA/scanout activity.

The SVG renders compact vector sprites from frame checkpoints to keep the artifact tractable. A later renderer can replay retained VRAM deltas directly without changing CPU semantics.

## Scaling strategy

Presentation is sampled, semantics are not. The renderer uses:

- one construction animation per node;
- one draw animation per wire;
- bounded sampling of native event streams;
- shared fleet/player transforms;
- sparse kill events;
- declarative camera motion;
- a hard 5 MB production size budget.

## Failure policy

Generation fails rather than publishing misleading output if:

- the bot cannot clear the match within the frame budget;
- the final injected topology is invalid;
- a traced µROM word differs from the physical word selected by µADDR/opcode;
- native architectural activity lacks the required control authority;
- native SP activity diverges from its physical `STACK` row or bus transaction;
- CALL/RET stack bytes disagree with the real PC stream;
- required native SVG overlays or metadata disappear;
- legacy semantic activity leaks back into production;
- the artifact exceeds the size budget;
- SVG safety/XML validation fails;
- deterministic tests diverge.
