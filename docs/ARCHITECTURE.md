# Architecture

Leader Invader is a deterministic simulation + replay compiler, not a hand-authored animation.

GitHub renders README SVGs as images, so the simulation runs before publication and the generated SVG is a self-contained declarative replay.

```text
seed
  |
  v
leader-core ----------> MatchTrace ------------> leader-svg base
  |                         |                         |
  |                         |                         v
  |                         +-----------------> leader-cli native overlays
  |                                                   |
  |                                                   v
  +--> ROM + CPU + µROM + RAM + VRAM          generated/Leader.svg
       + arcade peripherals
       + validated physical topology
```

## Crates

### `leader-core`

Owns semantics, determinism and physical/causal contracts.

- repository-owned deterministic RNG;
- assembled byte-addressed ROM program;
- CPU state including PC, SP, MAR, MDR, IR, flags and register file;
- physical µPC / microsequencer and 24-bit control words;
- real operand, register-select, address, branch-condition and PC-mux latches;
- ripple-carry ALU, PC incrementer and SP increment/decrement networks;
- byte-addressed work RAM and stack window;
- real 128×96 1-bit VRAM generation;
- native CPU trace streams;
- shift-register and formation-cadence hardware/event streams;
- three-slot enemy projectile bank;
- 64-byte bit-addressed shield bank;
- topology injection for CPU and arcade peripherals;
- CPU, stack, topology and M3 peripheral validators;
- `MatchTrace`, the contract between simulation and presentation.

### `leader-svg`

Owns the reusable declarative base presentation.

- node-by-node construction;
- wire draw animation;
- cinematic camera substrate;
- compact vector gameplay replay;
- reduced-motion fallback;
- historical coarse `MicroSample` activity for backward-compatible library callers.

The production CLI clears `micro_samples` from the base-render copy. Historical semantic activity therefore never reaches the README artifact.

### `leader-cli`

Owns the reproducible production boundary and high-fidelity native presentation.

```bash
cargo run -p leader-cli -- render --seed <seed> --output generated/Leader.svg
```

The production overlay chain is:

```text
director
-> PC -> decoder -> µcode -> 24-bit control -> control-state -> microcycles
-> ALU -> flags -> registers -> bus -> stack
-> formation cadence -> shift register -> enemy-shot bank -> shield bank
-> timing
```

`render`, `trace` and `stats` consume the trace returned directly by `Machine::run_match()`. They do not materialize SP and do not depend on `MicroSample` reconstruction.

## Fidelity contract

**F1 — causal micro-architecture — complete.** Real game/memory/VRAM transitions drive the replay.

**F2 — ISA execution — complete.** The game loop lives in assembled ROM bytecode. Fetch/decode, stack, registers, flags, branches and memory are real execution state.

**F3 — physical CPU datapath authority — complete.** Every visible critical CPU/control path is driven from native same-tick state or a physically justified combinational path.

**M3 — arcade peripheral authority — core set complete.** The same rule now covers shift hardware, formation timing, multiple enemy projectiles and destructible shields.

A visible path is authoritative only if corrupting or removing its backing state/control causes validation to fail. Presentation sampling never changes simulation semantics.

## Physical CPU control ROM

The execute space occupies `0x80..0xFC` as 25 opcode slots × 5 rows. Shared routines occupy dedicated fetch/operand/read/write regions.

External controls:

```text
REGW ALU MEMR MEMW PCLD STACK WAIT HALT
```

Internal controls:

```text
MAR_LOAD MDR_LOAD IR_LOAD PC_INC
OPERAND_A_LOAD OPERAND_B_LOAD ALU_OP_LOAD FLAGS_LOAD
ADDR_LO_LOAD ADDR_HI_LOAD CONDITION_LOAD PC_SELECT
REG_SELECT BUS_ADDRESS_ENABLE BUS_DATA_ENABLE ARCH_COMMIT
```

`ControlWord::bits24()` is canonical. `MicroAddressEvent.control_bits` carries that exact word into the native trace. `physical_control_lines()` defines the stable bit → node → label mapping and tests prove every bit is exercised and physically connected.

## Native trace model

The system deliberately uses the **smallest authoritative representation** for each subsystem rather than forcing every device into one event abstraction.

### First-class CPU streams

1. `MicroCycleEvent` — T-state snapshots with PC/MAR/MDR/IR.
2. `MicroAddressEvent` — µPC transition and exact 24-bit word.
3. `BusTransactionEvent` — bus owner/address/data/kind/control.
4. `AluEvent` — operation, operands, result and carry chain.
5. `FlagEvent` — exact Z/C/L latch result.
6. `ControlLatchEvent` — exact ADDR/COND/PC-SEL/REG-SEL values.
7. `RegisterWriteEvent` — architectural register mutation.
8. `PcEvent` — exact ripple increments and selected loads.
9. `SpEvent` — exact PUSH/POP mutation and ripple chain.

`MicroSample` remains compatibility-only.

### M3 authority sources

- **Shift register:** first-class device event + same-tick MMIO bus transaction.
- **Formation cadence:** first-class clock event + fleet RAM movement cross-check.
- **Enemy shots:** persistent `EnemyShotBank`, exact `X/Y/ACTIVE` RAM writes and native `FrameState` snapshots.
- **Shields:** persistent `ShieldBank` plus exact 64-byte RAM mutation stream. Shield bytes are intentionally not duplicated into every `FrameState`.

This distinction matters: “native” means the data comes from the real simulation mutation path, not that every subsystem must allocate a bespoke event type.

## CPU authority

### Fetch / memory

```text
FETCH_T0: MAR_LOAD + BUS_ADDRESS_ENABLE
FETCH_T1: MEMR + MDR_LOAD + PC_INC + BUS_DATA_ENABLE
FETCH_T2: IR_LOAD
```

Shared operand/read/write routines follow the same rule. PC increments are independently tied to the exact preceding `PC_INC` row.

### ALU / flags / registers

Five-row ALU instructions load A/B/op state before propagation. Flags mutate only with `FLAGS_LOAD`. Register writes require selected destination plus `REGW + ARCH_COMMIT`.

### Branches / PC

`ADDR_LO_LOAD` and `ADDR_HI_LOAD` build targets. Conditional branches latch condition state, then `PC_SELECT` selects or invalidates a target. PC mutation requires `PCLD + ARCH_COMMIT`.

### CALL / RET / SP

CALL uses a visible combinational return-byte path instead of a hidden return-address latch:

```text
PC LO ----\
           > RETURN BYTE MUX -> DATA BUS -> STACK RAM
PC HI ----/

STACK RAM -> DATA BUS -> ADDR LO / ADDR HI -> PC SELECT -> PC
```

SP movement is exact 16-bit ripple decrement/increment. `validate_call_stack_contract()` proves CALL pushes `instruction_pc + 3`, RET pops the same bytes in LIFO order and actual return PC events agree.

## M3 arcade hardware

### 16-bit shift register

The memory-mapped peripheral owns a 16-bit shift state, a 3-bit offset and an 8-bit read window. Two data writes cascade bytes through the register. The ROM boot self-test exercises `0x12`, `0x34`, offset `3`, result `0xA0`.

`validate_shift_register_contract()` replays the device events and requires same-tick bus authority. The topology exposes `SHIFT HI`, `SHIFT LO`, `SHIFT OFFSET`, window mux and output latch.

### Formation cadence

`FormationCadence` is the sole gate for formation movement. Its divisor depends on remaining invaders and accelerates `3 → 2 → 1`.

Each native event records alive count, divisor, counter before/after and tick. `validate_formation_cadence_contract()` rejects any fleet position/direction RAM mutation that occurs without a corresponding hardware tick.

### Three-slot enemy projectile bank

`EnemyShotBank` owns three independent projectile slots plus allocator/cooldown state.

Per slot RAM:

```text
X / Y / ACTIVE
```

The validator replays all writes between native frame checkpoints. It rejects arm-while-active, clear-while-inactive, wrong component controls, missing writes and snapshot divergence. All three slots and concurrent projectile states must be exercised in a complete match.

A shield-induced clear has a stronger cross-device rule:

```text
SHIELD_DAMAGE_ENEMY write
        |
        | same frame + same PC + immediately preceding ordinal
        v
ENEMY_SHOT_SHIELD_CLEAR
```

An orphan shield clear is invalid even if the final projectile snapshot would otherwise look plausible.

### Bit-addressed destructible shields

There are four `16 × 8` shield bitmaps: 512 bits / 64 bytes total. The shield RAM window begins at `RAM_BASE + 0x40`.

The initial silhouette is stored as actual bits. World coordinates resolve to:

```text
world x/y
 -> shield index
 -> local x/y
 -> byte offset
 -> one-hot bit mask
```

Both projectile directions sweep every intermediate pixel to prevent tunneling. On impact, `ShieldBank::damage` clears exactly one existing bit and returns the byte-level `before / mask / after` mutation. Machine writes the resulting byte to shield RAM with a source-specific control:

```text
SHIELD_DAMAGE_PLAYER
SHIELD_DAMAGE_ENEMY
```

`validate_shield_bank_contract()` reconstructs the full bank from the initial bitmap and replays only those RAM writes. A valid mutation must satisfy:

```text
after == before & !mask
mask.count_ones() == 1
before & mask != 0
```

It therefore rejects bit creation, multi-bit destruction, duplicate destruction and invalid control provenance.

Physical path:

```text
SHIELD ADDR
    |
DAMAGE MASK
    |
SHIELD WRITE ENABLE
    |
+---+---+---+---+
|   |   |   |   |
RAM0 RAM1 RAM2 RAM3
 \   |   |   /
  SHIELD VIDEO MUX
         |
      scanout
```

The CRT overlay starts from the same initial bitmap and attaches a disappearance animation to each damaged bit at the timestamp of its native RAM write. There is no separate decorative shield state.

## Physical topology contract

`validate_final_topology()` runs on the fully injected runtime topology. It requires:

- unique node and link IDs;
- closed link endpoints;
- group containment;
- all 24 CPU control outputs and required CPU state nodes;
- shift-register hardware;
- formation cadence hardware;
- three complete enemy-shot X/Y/ACTIVE banks;
- shield address/mask/write/RAM/video nodes.

Generation fails if a required subsystem disappears or escapes its physical group.

## Production validation boundary

Before writing `Leader.svg`, the CLI validates:

1. final topology;
2. native CPU/control authority;
3. first-class SP vs stack bus;
4. CALL/RET stack contract;
5. shift-register MMIO authority;
6. formation cadence and movement gating;
7. enemy-shot RAM/snapshot replay;
8. shield one-bit RAM replay;
9. enemy-shot ↔ shield cross-device ordering;
10. required native SVG groups/metadata;
11. absence of legacy semantic activity / JavaScript;
12. hard **5,000,000-byte** SVG budget.

The shield-complete generated artifact measured **3,856,346 bytes**, leaving roughly **1.14 MB** of margin.

## Video path

Game state is rasterized into simulated 128×96 one-bit VRAM, checksummed and exposed through DMA/scanout activity. Invaders, player, all enemy shots and current shield bits affect the native VRAM image.

For tractable README size, the SVG uses compact vector gameplay elements and bounded native overlay sampling. Shield destruction is still timestamped from exact RAM mutations rather than approximated from frame interpolation.

## Determinism

Every semantic source of nondeterminism derives from the explicit seed:

- no wall clock in `leader-core`;
- no OS RNG;
- no semantic thread scheduling;
- stable repository-owned text hash + SplitMix64;
- same source + same seed = same match semantics.

A commit SHA is therefore a natural production seed.

## Failure policy

Generation fails rather than publishing misleading output if the match misses its frame budget, topology is invalid, CPU physical authority is broken, stack/return bytes disagree, a peripheral event lacks bus/RAM authority, formation movement bypasses cadence, projectile snapshots diverge, a shield write is not a valid one-bit erase, a shield-caused shot clear is orphaned, required native SVG metadata disappears, the artifact exceeds budget, or SVG safety/XML validation fails.
