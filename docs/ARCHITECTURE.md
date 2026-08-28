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
- native microcycle, bus, ALU, register and PC event streams;
- physical topology for all 24 µROM output lines and their consumers;
- CALL/RET combinational return-path contract;
- trace/control and final-topology validators;
- `MatchTrace`, the contract between simulation and presentation.

### `leader-svg`

Owns the reusable declarative base presentation.

- node-by-node construction;
- wire draw animation;
- cinematic camera substrate;
- vector gameplay replay and kill events;
- reduced-motion fallback;
- historical coarse `MicroSample` activity for backward-compatible library callers.

The production CLI intentionally clears `micro_samples` before invoking this base renderer, so the historical coarse activity layer is absent from the README artifact.

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
- control-state latches;
- T-state microcycle snapshots;
- ALU;
- register writes;
- bus transactions;
- stack/SP;
- timing.

It then validates the completed SVG before writing it. CI calls this exact path; there is no hidden renderer.

## Fidelity contract

**F1 — causal micro-architecture — complete.** Real game/memory/VRAM state transitions drive the replay.

**F2 — ISA execution — complete.** The autonomous game loop lives in assembled ROM bytecode. Fetch/decode/control flow, stack behavior, registers, flags and memory are execution state. Corrupting ROM breaks the match causally.

**F3 — physical datapath authority — advanced.** Critical visible control and datapath paths are same-tick authoritative rather than descriptive. The current implementation includes:

- native T0/T1/T2 microcycles;
- a real µPC with fetch, sequential, dispatch, routine-call and routine-return transitions;
- a physical `256 × 24` control-ROM model;
- 24 distinct visible control outputs, each with a unique bit/node/label contract;
- physical wiring from internal control outputs to MAR/MDR/IR, PC increment, ALU/select/flag paths, target/condition/PC/register state, bus enables and commit;
- explicit visible `ADDR LO`, `ADDR HI`, `COND`, `PC SEL` and `REG SEL` state nodes;
- a visible combinational `RETURN BYTE` mux connecting stable PC high/low bytes to CALL stack writes and the stack data path back to RET address latches;
- gated architectural commits for register writes, memory writes and PC loads;
- native bus ownership, exact ALU, register-write and PC streams;
- native decode from `DecodeLatch` microcycles;
- exact SP ripple reconstruction from native stack transactions;
- a production SVG whose high-fidelity hardware presentation is independent of `MicroSample`.

A visible critical control line is considered complete only when removing or mis-selecting that line changes or prevents the real machine result in the same instruction.

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

The trace uses several time-aligned native streams rather than treating `MicroSample` as universal truth.

1. `FrameState` — stable game/display checkpoints.
2. `MicroCycleEvent` — exact T-state snapshots with PC/MAR/MDR/IR and native `DecodeLatch` events.
3. `MicroAddressEvent` — µPC transitions and selected 24-bit control word.
4. `BusTransactionEvent` — address/data ownership and fetch/read/write/input/DMA/scanout transactions.
5. `AluEvent` — operation, operands, result and exact carry chain.
6. `RegisterWriteEvent` — architectural register mutation.
7. `PcEvent` — exact ripple increments and selected PC loads.
8. `MicroSample` — historical compatibility only in production architecture.

The production renderer clears `micro_samples` before base rendering, then consumes the native streams directly. A pipeline test requires byte-for-byte identical F3 output after `trace.micro_samples.clear()`.

## CPU authority

### Fetch and memory routines

The selected µROM row must physically enable each mutation:

```text
FETCH_T0: MAR_LOAD + BUS_ADDRESS_ENABLE
FETCH_T1: MEMR + MDR_LOAD + PC_INC + BUS_DATA_ENABLE
FETCH_T2: IR_LOAD
```

Shared operand/read/write routines follow the same rule. Native bus validation ties `ROM_FETCH`, `CPU_READ` and `CPU_WRITE` transactions back to their required physical rows.

### ALU

Five-row ALU instructions load explicit CPU latches for A, B and operation selection. Propagation consumes only those latches. Flags require `FLAGS_LOAD`; register mutation requires the final write-enable/commit row.

The SVG exposes operation, lhs, rhs, effective rhs, result and carry chain as data attributes while lighting the exact full-adder path.

### Register file

`REG_SELECT` loads real register-selection state. The selected A register is the authoritative architectural write-back destination. Native register events expose before/after values and are validated against `REGW + ARCH_COMMIT`.

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

`validate_call_stack_contract()` proves against the real native trace that:

- CALL pushes high then low byte of `PC + 3`;
- RET pops the exact bytes in LIFO order;
- reconstructed return addresses equal the actual `PcEvent::Return` targets;
- call/return pairs are balanced for the complete match.

This closes the return-address source path without inventing an unnecessary 25th control bit.

### Stack / SP

`STACK` gates PUSH/POP. SP movement uses exact 16-bit ripple decrement/increment logic. The stack SVG exposes push/pop kind, SP before/after, address and data byte.

A dedicated first-class `SpEvent` stream would improve trace explicitness but is not required for execution or rendering fidelity because all needed values are already derived from authoritative native stack transactions.

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

## Native SVG contract

The completed production SVG is validated before it is written.

Required native groups include PC, decoder, microcode, 24-bit control bank, control-state latches, microcycles, ALU, register file, bus, stack and timing.

The artifact must also contain structured metadata for the corresponding native values. Generation fails if:

- a required native group or metadata family is missing;
- legacy `class="hot …"` semantic activity is present;
- script or JavaScript URLs are present;
- the SVG exceeds the **5,000,000 byte** production budget.

The current artifact is roughly 4 MB, leaving explicit headroom while preserving inspectability.

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
- CALL/RET stack bytes disagree with the real PC stream;
- required native SVG overlays or metadata disappear;
- legacy semantic activity leaks back into production;
- the artifact exceeds the size budget;
- SVG safety/XML validation fails;
- deterministic tests diverge.
