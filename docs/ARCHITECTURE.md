# Architecture

Leader Invader is a deterministic simulation + replay compiler, not a hand-authored animation.

GitHub renders the README SVG as an image, so the simulation runs before publication and the generated SVG is a self-contained declarative replay. The same physical machine model is also the authority for the future interactive explorer.

```text
seed
  |
  v
leader-core ----------------------> MatchTrace -----------------> leader-svg base
  |                                     |                              |
  |                                     |                              v
  |                                     +-----------------------> leader-cli native overlays
  |                                                                    |
  |                                                                    +--> cinematic CameraCue scenes
  |                                                                    +--> hierarchy metadata
  |                                                                    +--> interactive node targets
  |                                                                    |
  +--> physical topology --> NavigationModel                            v
       ROM / CPU / µROM / RAM / VRAM                           generated/Leader.svg
       arcade peripherals
       canonical memory map
       physical contracts
```

The central rule is simple:

> **There is one physical graph. Navigation, camera framing and interaction reference that graph; they never replace it with a second UI-only topology.**

## Crates

### `leader-core`

Owns semantics, determinism, physical authority and frontend-independent navigation structure.

- repository-owned deterministic RNG;
- assembled byte-addressed ROM program;
- CPU state including PC, SP, MAR, MDR, IR, flags and register file;
- physical µPC / microsequencer and 24-bit control words;
- real operand, register-select, address, branch-condition and PC-mux latches;
- ripple-carry ALU, PC incrementer and SP increment/decrement networks;
- canonical ROM/RAM/VRAM/MMIO ownership map;
- byte-addressed work RAM and stack window;
- real 128×96 1-bit VRAM generation;
- native CPU trace streams;
- shift-register and formation-cadence hardware/event streams;
- three-slot enemy projectile bank;
- 64-byte bit-addressed shield bank;
- topology injection for CPU and arcade peripherals;
- CPU, stack, topology, memory-map, video and M3 peripheral validators;
- hierarchical `NavigationModel` derived from the final physical topology;
- direct traversal queries for node → view resolution;
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

The production presentation chain is conceptually:

```text
base SVG
 -> hierarchical director / LOD
 -> interactive navigation targets
 -> PC -> decoder -> µcode -> 24-bit control -> control-state -> microcycles
 -> ALU -> flags -> registers -> bus -> video pipeline -> stack
 -> formation cadence -> shift register -> enemy-shot bank -> shield bank
 -> timing
 -> SVG contract validation
```

`render`, `trace` and `stats` consume the trace returned directly by `Machine::run_match()`. They do not depend on `MicroSample` reconstruction.

## Fidelity contract

**F1 — causal micro-architecture — complete.** Real game/memory/VRAM transitions drive the replay.

**F2 — ISA execution — complete.** The game loop lives in assembled ROM bytecode. Fetch/decode, stack, registers, flags, branches and memory are real execution state.

**F3 — physical CPU datapath authority — complete.** Every visible critical CPU/control path is driven from native same-tick state or a physically justified combinational path.

**M3 — arcade peripheral authority — core set complete.** The same rule covers shift hardware, formation timing, multiple enemy projectiles, destructible shields and the native video path.

A visible path is authoritative only if corrupting or removing its backing state/control causes validation to fail. Presentation sampling never changes simulation semantics.

## Physical topology vs navigation hierarchy

`Topology` is the physical source of truth:

```text
Topology
  groups[]
  nodes[]
  links[]
```

A node owns physical identity, kind, group and bounds. Links connect real topology node IDs. All later presentation systems reference these exact IDs.

`NavigationModel` is derived from the completed topology after CPU/M3/video layout injection:

```text
machine
  -> subsystem
       -> detail
```

Examples:

```text
machine
  -> decode
       -> decode.instruction
       -> decode.microcode

machine
  -> alu
       -> alu.ripple

machine
  -> io
       -> io.input_irq
       -> io.shift_register
       -> io.formation
       -> io.enemy_shots
       -> io.shields

machine
  -> gpu
       -> gpu.dma
       -> gpu.scanout
       -> gpu.timing
```

A `Module` contains references to physical `node_ids`, never copies of nodes. Its bounds are derived by unioning the bounds of those physical nodes.

A `CameraView` references one module and defines presentation framing plus detail density:

```text
Overview  -> whole machine
Native    -> subsystem
BitExact  -> dense detail module
```

### Hierarchy invariants

`navigation_violations()` makes hierarchy correctness a production contract:

- the default view exists;
- every physical node has exactly one subsystem owner matching `node.group`;
- detail modules reference only existing physical nodes;
- every detail module contains its referenced node bounds;
- a physical node has at most one detail owner;
- all parent/child module links close;
- all view parent links close;
- view/module IDs are unique.

This prevents a future UI from silently developing a topology different from the simulated machine.

## Direct node navigation

`NavigationModel` exposes frontend-independent traversal queries:

```text
child_views(view)
view_path_for_node(node)
deepest_view_for_node(node)
```

For example:

```text
microRom
  -> view-machine
  -> view-decode
  -> view-decode.microcode

shieldAddr
  -> view-machine
  -> view-io
  -> view-io.shields
```

A node with no dedicated detail module resolves to its subsystem view. An unknown node has no navigation target.

The production SVG compiles the same resolution directly into each rendered physical node:

```text
data-subsystem
data-detail-module
data-detail-density
data-target-view
data-parent-view
data-view-path
```

This means a WASM/DOM explorer can implement click, enter, back and breadcrumbs without inferring hierarchy from screen coordinates or CSS classes.

## Deterministic camera scenes and level of detail

The README cannot rely on JavaScript interaction, so the hierarchy already powers an automatic cinematic traversal.

The director builds one ordered `CameraCue` timeline. Each cue contains:

```text
time
camera rectangle
view ID
detail LOD state
```

That single source drives all of the following:

- camera matrix animation;
- active hierarchy boundary emphasis;
- node kind visibility;
- node title visibility;
- base wire opacity;
- subsystem group opacity;
- serialized scene metadata for later replay/scrubbing.

The SVG includes a hidden `navigation-scenes` group with deterministic metadata:

```text
data-scene-time
data-scene-progress
data-scene-view
data-scene-detail
data-scene-x/y/w/h
```

The future explorer can therefore reuse the README's cinematic sequence, scrub it, pause it, or replace it with user-controlled navigation without changing simulation semantics.

### Readability policy

The diagram does not use a single visual density everywhere.

- **Machine overview:** topology remains visible, but wires and secondary node text are subdued.
- **Subsystem view:** local wiring and labels gain contrast.
- **Bit-exact detail:** wire, node-title and node-kind contrast rises while broad subsystem framing recedes.
- **Gameplay CRT:** technical framing recedes again so the game remains legible.

Native activity overlays are not attenuated by this readability LOD. Recorded execution remains visually authoritative even when static diagram scaffolding is softened.

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

“Native” means the data comes from the real simulation mutation path, not that every subsystem must allocate a bespoke event type.

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

`EnemyShotBank` owns three independent projectile slots plus allocator/cooldown state. Per-slot RAM is `X / Y / ACTIVE`.

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

There are four `16 × 8` shield bitmaps: 512 bits / 64 bytes total. The initial silhouette is stored as actual bits.

World coordinates resolve to:

```text
world x/y
 -> shield index
 -> local x/y
 -> byte offset
 -> one-hot bit mask
```

Both projectile directions sweep every intermediate pixel to prevent tunneling. On impact, `ShieldBank::damage` clears exactly one existing bit and returns the byte-level `before / mask / after` mutation.

A valid shield mutation satisfies:

```text
after == before & !mask
mask.count_ones() == 1
before & mask != 0
```

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

The CRT overlay starts from the same initial bitmap and attaches disappearance animation to each damaged bit at the timestamp of its native RAM write. There is no separate decorative shield state.

## Canonical memory ownership

The project has one 8080-flavoured address map:

```text
0000–1FFF  PROGRAM ROM    8 KiB
2000–7FFF  WORK RAM      24 KiB
  2020–2028  enemy-shot slot RAM
  2040–207F  shield bitmap RAM
  7F00–7FFF  stack window
8000–87FF  VIDEO RAM      2 KiB
A000–A1FF  MMIO           input / shift / game device
```

`memory_map.rs` owns `MemoryRegion`, `MemoryOwner`, top-level ranges, M3 RAM subregions and all public MMIO ports. `program.rs` re-exports historical constants from this module.

Static map tests prove region/subregion containment and non-overlap. `validate_memory_map_contract()` then validates the runtime trace: mapped owner, fetch ownership, read data source, CPU writes, MMIO input and VRAM DMA/scanout ownership.

## Physical topology contract

`validate_final_topology()` runs on the fully injected runtime topology. It requires:

- unique node and link IDs;
- closed link endpoints;
- group containment;
- all 24 CPU control outputs and required CPU state nodes;
- shift-register hardware;
- formation cadence hardware;
- three complete enemy-shot X/Y/ACTIVE banks;
- shield address/mask/write/RAM/video nodes;
- physical video timing and scanout nodes.

Generation fails if a required subsystem disappears or escapes its physical group.

## Production validation boundary

Before writing `Leader.svg`, the CLI validates:

1. final physical topology;
2. hierarchical navigation closure and unique node ownership;
3. native CPU/control authority;
4. first-class SP vs stack bus;
5. CALL/RET stack contract;
6. shift-register MMIO authority;
7. formation cadence and movement gating;
8. enemy-shot RAM/snapshot replay;
9. shield one-bit RAM replay;
10. enemy-shot ↔ shield cross-device ordering;
11. canonical memory ownership for every addressed native bus transaction;
12. ordered raster/DMA/scanout/WAIT video authority;
13. required native SVG groups and metadata;
14. concrete hierarchy, scene and interactive node-target metadata;
15. absence of legacy semantic activity and JavaScript;
16. XML/GitHub-safe SVG validation.

Artifact size is telemetry, not a semantic validity condition. A large artifact is valid when its causal and inspectability contracts are complete.

## Video path

Game state is rasterized into simulated 128×96 one-bit VRAM, checksummed and exposed through DMA/scanout activity. Invaders, player, all enemy shots and current shield bits affect the native VRAM image.

The SVG uses compact vector gameplay elements and bounded sampling only for high-frequency presentation families where exhaustive drawing would reduce readability. Underlying simulation and validation remain exhaustive.

## Determinism

Every semantic source of nondeterminism derives from the explicit seed:

- no wall clock in `leader-core`;
- no OS RNG;
- no semantic thread scheduling;
- stable repository-owned text hash + SplitMix64;
- same source + same seed = same match semantics.

A commit SHA is therefore a natural production seed.

## Failure policy

Generation fails rather than publishing misleading output if the match misses its frame budget, topology is invalid, navigation ownership is ambiguous, CPU physical authority is broken, stack/return bytes disagree, a peripheral event lacks bus/RAM authority, formation movement bypasses cadence, projectile snapshots diverge, a shield write is not a valid one-bit erase, a shield-caused shot clear is orphaned, a native bus transaction violates mapped ownership, required hierarchy/native metadata disappears, an interactive node target becomes invalid, or SVG safety/XML validation fails.
