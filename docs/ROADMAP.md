# Roadmap

## M0 — deterministic README pipeline ✅

- Rust workspace and CI.
- Stable detailed hardware topology.
- Seeded autonomous Space Invaders-like match.
- Real RAM state + 128×96 one-bit VRAM generation.
- Trace-driven long-form SVG.
- GitHub Actions regeneration from commit SHA.
- Cinematic director with subsystem close-ups and final framebuffer zoom.

## M1 — byte-addressed ISA / F2 ✅

The game control loop lives in assembled ROM bytecode interpreted by the CPU.

- real instruction fetch / decode / execute through ROM bytes;
- eight 8-bit registers, flags, PC and stack pointer;
- CALL / RET and conditional control flow;
- memory-mapped input, game-device and video registers;
- repository-owned two-pass assembler with labels/fixups;
- game program emitted as an 8 KiB-bounded ROM image;
- WAIT_VBLANK and HALT are real instructions;
- corrupting the ROM breaks the match causally.

## M2 — bit-accurate datapath / F3 ✅

The visible CPU/control path is production-native and physically authoritative.

### Control unit

- native T0/T1/T2 CPU microcycles;
- real µPC with fetch, sequential, dispatch, routine-call and routine-return transitions;
- `256 × 24` physical control-ROM representation;
- all 24 control bits persisted into the native trace;
- 24 distinct visible control outputs with a stable bit → node → label contract;
- full µROM scan proving all 24 physical bits are exercised;
- internal outputs wired to real physical consumers;
- visible address, condition, PC-select and register-select state with exact values.

### Datapath authority

- physically gated MAR/MDR/IR fetch latches and ripple PC increment;
- shared operand/read/write micro-routines;
- ripple-carry 8-bit ALU and 16-bit PC/SP increment/decrement networks;
- authoritative operand/op/flag/register/address/condition/PC-select state;
- CALL/RET stack path with direct CPU-native `SpEvent` mutations;
- native CPU bus/ALU/flag/control/register/PC/SP streams;
- production renderer independent of `MicroSample` reconstruction.

### Production contracts

- exact µROM/control validation;
- SP and CALL/RET contracts;
- final injected topology validation;
- native-only overlay pipeline;
- GitHub-safe declarative output validation;
- artifact size recorded as telemetry rather than used as a semantic validity ceiling.

### F3 acceptance — satisfied

Every visible critical CPU datapath/control node is backed by native same-tick state or a physically justified combinational path. Removing or corrupting authority is caught by tests/contracts.

## M3 — richer arcade hardware ✅

The F3 authority rule is now applied to the core game-specific hardware set. Simulation semantics remain complete and unsampled; only SVG presentation is bounded for readability.

### Original-arcade-inspired 16-bit shift register ✅

- explicit 16-bit device state with two-byte cascading load;
- 3-bit offset register and 8-bit shifted read window;
- memory-mapped `SHIFT_DATA`, `SHIFT_OFFSET`, `SHIFT_RESULT` ports;
- assembled ROM boot self-test proving `0x12`, `0x34`, offset `3` → `0xA0`;
- first-class shift-register events tied to same-tick CPU bus transactions;
- causal replay/corruption tests;
- visible `SHIFT HI / LO / OFFSET / WINDOW / OUT` hardware path;
- mandatory native SVG metadata/overlay.

### Hardware formation cadence ✅

- persistent hardware counter/divider is the sole fleet movement gate;
- divisor accelerates `3 → 2 → 1` as formation population falls;
- native clocks include alive count, divisor, counter before/after and tick;
- fleet RAM mutation without `tick=true` is invalid;
- visible alive/divider/counter/tick path;
- bounded overlay preserves all speed bands and tick states.

### Three-slot enemy projectile bank ✅

- explicit `EnemyShotBank` owns three independent projectile slots;
- round-robin allocator + hardware cooldown state;
- authoritative per-slot `X / Y / ACTIVE` RAM bytes;
- bot avoidance, collision, VRAM and gameplay replay consume all three slots;
- complete matches exercise all slots and concurrent projectiles;
- exact RAM replay across native frame checkpoints;
- invalid arm/clear ordering, missing writes and snapshot corruption fail validation;
- physical allocator/cooldown plus three visible X/Y/ACTIVE banks;
- shield-caused slot clear requires immediately preceding same-frame/same-PC `SHIELD_DAMAGE_ENEMY` authority;
- strict bounded overlay sampling preserves concurrency and slot use.

### Bit-addressed destructible shields ✅

- four explicit `16 × 8` shield bitmaps = 64 bytes total;
- shield RAM begins at `RAM_BASE + 0x40`;
- initial bunker silhouette is actual bit state, not presentation geometry;
- world coordinate → shield → byte → one-hot bit-mask addressing;
- player and enemy projectile sweeps prevent tunneling at 3 px / 2 px steps;
- each impact clears exactly one existing bit and writes the resulting RAM byte;
- source-specific `SHIELD_DAMAGE_PLAYER` / `SHIELD_DAMAGE_ENEMY` controls;
- replay contract rejects bit creation, duplicate/multi-bit destruction and wrong provenance;
- physical `ADDR -> MASK -> WRITE -> RAM0..3 -> VIDEO` path;
- CRT shield pixels disappear at exact native RAM-write timestamps;
- cross-device enemy-shot/shield ordering is validated.

### M3 acceptance — satisfied

The four core arcade systems are authoritative, physically represented and production-validated. No proprietary arcade ROM assets are used.

## Post-M3 hardware cleanup

### Canonical 8080-flavoured memory map ✅

The existing address layout was centralized **without changing any address or ROM semantics**:

```text
0000–1FFF  ROM
2000–7FFF  RAM
  2020–2028  three enemy-shot slots
  2040–207F  shield bitmap RAM
  7F00–7FFF  stack
8000–87FF  VRAM
A000–A1FF  MMIO
```

Completed contracts:

- `MemoryRegion` / `MemoryOwner` canonical definitions;
- program ports re-exported from the memory map for backward compatibility;
- projectile, shield and stack windows source their addresses from the canonical map;
- top-level region non-overlap tests;
- RAM subregion containment/non-overlap tests;
- all declared ports proven inside MMIO;
- 1536-byte framebuffer proven to fit the 2 KiB physical VRAM region;
- exact ownership-boundary tests;
- `validate_memory_map_contract()` rejects unmapped native bus accesses;
- fetch/read/write/input/DMA/scanout data ownership is validated against the mapped region;
- production `render`, `trace` and `stats` all require this contract.

Artifact size is intentionally observational. Native semantic completeness and inspectability take priority over a fixed byte ceiling; sampling is used only where it improves presentation readability, never to make validation pass.

### Remaining optional cleanup

- migrate remaining internal address-classification literals to `memory_map::owner()` where useful, while keeping the ownership contract as the safety net;
- explore richer original-arcade timing/peripheral quirks only where they improve inspectability;
- remove obsolete warning-only/helper code that is no longer needed by physical wrong-row tests;
- continue reducing presentation duplication while maintaining inspectable metadata;
- eventually deprecate historical `MicroSample`/bus reconstruction helper APIs when compatibility is no longer needed.

## M4 — live WASM explorer 🚧

The README remains a zero-JavaScript cinematic artifact, while the same Rust core now also powers a live browser explorer. The frontend is deliberately presentation-only: it may transform pointer coordinates and render native data, but it must not duplicate CPU, device, memory-map, topology, routing, hit-test or gameplay semantics.

### Shared navigation substrate ✅

- canonical hierarchy `machine → subsystem → detail` over the real physical topology;
- every topology group is a first-class subsystem view;
- dense CPU, memory, bus, M3 and video regions have dedicated bit-exact detail views;
- module bounds are derived from their real physical nodes rather than a duplicate UI graph;
- every node has a unique subsystem owner and at most one detail owner;
- hierarchy validation is a production gate;
- `child_views()`, `view_path_for_node()` and `deepest_view_for_node()` provide direct traversal queries;
- every rendered physical node carries subsystem/detail membership plus `target-view`, `parent-view` and complete `view-path` metadata;
- deterministic `CameraCue` scenes drive README framing and level-of-detail presentation;
- README replay already uses the hierarchy for CPU/M3/video close-ups without JavaScript.

### Live explorer foundation ✅

- `leader-explorer` is a thin WASM adapter over `leader-core`;
- the complete canonical topology and view-scoped physical graphs are exposed to WASM;
- view-scoped hit testing is performed by Rust against real node bounds;
- drag-pan, wheel zoom and two-pointer pinch/pan use the Rust camera state;
- click-to-enter uses `deepest_view_for_node()` rather than frontend heuristics;
- breadcrumb, parent, back, home and child-view navigation use canonical hierarchy edges;
- hover inspection exposes real node id, kind, subsystem, bounds and target path;
- canonical orthogonal routing now lives in renderer-independent `leader-core::routing`;
- live navigation consumes that shared router and serializes canonical paths to WASM;
- the browser renders those routes rather than inventing center-to-center wiring.

### Native deterministic playback ✅

- browser playback is generated by `Machine::run_match()` and consumes native trace streams;
- pause / play / micro-step / instruction-step;
- exact microcycle cursor seek and continuous scrubber;
- exact event focus is retained for bus, DMA and VBlank seeks even when the nearest microcycle key differs;
- follow PC / bus / DMA / VBlank navigates to canonical physical views;
- PC, MAR, MDR, IR, bus address/data/source and frame state are inspectable in the browser;
- seed and frame-count selection regenerate deterministic native traces;
- CI compiles the explorer for `wasm32-unknown-unknown`, checks browser JavaScript syntax, and runs native workspace tests + Clippy.

### Live physical activity ✅

- `leader-core` owns `phase → physical node ids` activity mapping;
- addressed ROM/RAM/VRAM activity selects the exact canonical physical page;
- WASM exposes the current native physical-activity snapshot;
- `leader-core` derives a canonical conservative active-link subgraph from the physical topology, so the frontend no longer infers wire activity from active endpoints;
- address/data values are attached to core-owned active links and rendered as hexadecimal or binary at deep zoom;
- native register/flag/PC/SP mutations are mapped to exact physical bit nodes and exposed by playback;
- the browser renders `0→1` and `1→0` mutations independently from generic phase activity, including transition/source inspection;
- `leader-core` resolves the native `AluTrace` into gate values for every visible XOR/SUM/GEN/PROP/CARRY/RES node across all eight slices;
- `Playback` exposes those exact 48 gate states for the current microcycle;
- the browser displays ALU gate `0/1` state, bit/stage metadata and deep-zoom values without reimplementing ALU semantics in JavaScript;
- the final topology now materializes previously sampled full-adder internal wiring and complete ROM/RAM/VRAM/system-bus page read/write paths;
- production topology validation requires every full-adder slice and every memory page path to remain connected;
- `prefers-reduced-motion` disables signal-flow animation without hiding activity state;
- source commit `0e7bea520277c7aeb0eb3fb49a68300bedbee8e6` passed workspace tests, WASM compilation, JS syntax, Clippy, smoke render and SVG validation in CI #1098.

### Remaining M4 work

- complete the physical ALU result-selection network for every operation (`PASS`, `AND`, `OR`, `XOR`, arithmetic SUM/COMPARE) before claiming exact dependency propagation for all opcodes;
- refine the conservative active-link subgraph into dependency-ordered per-stage propagation after those missing physical result paths exist;
- animate ordered address → decoder/page → data/control propagation from same-tick native bus events;
- expose carry/control/per-gate values on exact active links in addition to address/data values;
- add native VRAM checkpoints to `MatchTrace`: frame records currently contain only `vram_checksum`, so the live CRT must not be reconstructed from gameplay in JavaScript;
- expose checkpointed framebuffer/CRT state through `Playback` and synchronize it with DMA/scan timing;
- replay/scrub deterministic camera scenes in addition to raw microcycle time;
- add bot-policy selection once multiple deterministic policies are first-class core inputs;
- migrate the README renderer from its historical local `orthogonal_path` helper to `leader-core::routing` so both renderers share one route implementation;
- add browser-level smoke tests for generated WASM package + interactions;
- optional user-created groups/layout state layered above, never replacing physical topology authority.

## M5 — generated match seasons

- seed from commit SHA on source changes;
- optional scheduled matches;
- archive winning traces;
- multiple deterministic bot strategies;
- match metadata embedded in the SVG footer.
