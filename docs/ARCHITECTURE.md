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
  +--> topology + machine + game + bot + VRAM
```

## Crates

### `leader-core`

Owns semantics and determinism.

- repository-owned deterministic RNG;
- autonomous game state and bot;
- byte-addressed work RAM;
- real 128×96 1-bit VRAM generation;
- PC/decode/ALU/memory/DMA/scanout/VBlank causal phases;
- stable 300+ node topology derived from the visual CPU prototype;
- `MatchTrace`, the contract between simulation and presentation.

### `leader-svg`

Owns presentation only.

- node-by-node construction;
- wire draw animation;
- trace-driven activity overlays;
- cinematic camera through animated `viewBox`;
- vector gameplay replay and kill events;
- reduced-motion fallback.

The renderer may compress or interpolate a real trace. It may not invent machine activity.

### `leader-cli`

Owns the reproducible build boundary.

```bash
cargo run -p leader-cli -- render --seed <seed> --output generated/Leader.svg
```

CI calls the same command. There is no hidden renderer path.

## Fidelity contract

The project evolves through explicit levels rather than pretending all visible gates are already electrically simulated.

**F1 — causal micro-architecture (current milestone).** Real game/memory/VRAM state transitions emit causal machine events; visible nodes light from those events.

**F2 — ISA execution.** The game loop moves into ROM bytecode interpreted by the machine. Fetch/decode/control flow become first-class program execution.

**F3 — bit-accurate datapath.** PC/MAR/MDR/IR/register DFFs, ALU carry slices, decoders and buses become first-class logical state. At this point every visible datapath light is computed directly in the same tick.

No random decorative glow is allowed at any level.

## Determinism

Every source of simulation nondeterminism derives from the explicit seed.

- no wall clock inside `leader-core`;
- no OS RNG;
- no thread scheduling in semantic execution;
- stable text seed hash and SplitMix64 sequence owned by the repository;
- same source + same seed = same match semantics.

This makes a commit SHA a natural production seed.

## Trace model

Two time scales are recorded:

1. `FrameState` — stable game/display checkpoints.
2. `MicroSample` — causal PC/address/data/control activity.

The renderer maps logical simulation time onto cinematic time. A game can execute thousands of state transitions while the README stays around two minutes.

## Video path

The game state is mirrored into work RAM. Each frame is rasterized into a simulated 128×96 one-bit VRAM region (1536 bytes), checksummed, then exposed through DMA/scanout/VBlank trace phases.

The SVG currently renders compact vector sprites from frame checkpoints to keep the artifact small. The architecture allows a later renderer to replay retained VRAM deltas directly without changing the simulation API.

## Camera

The README cannot offer interactive mouse pan/zoom because GitHub embeds the SVG as an image. Camera motion is therefore generated declaratively by animating the nested SVG `viewBox`.

The camera sequence follows construction subsystem-by-subsystem, reveals the completed machine, follows boot activity, then moves into the display for the autonomous match before returning to the whole machine.

A later WASM explorer will reuse the exact same topology and simulation with real drag-pan, wheel zoom and node inspection.

## Scaling strategy

A naïve keyframe for every gate on every cycle would create an unusably large SVG. The renderer uses semantic compression:

- one construction animation per node;
- one draw animation per wire;
- bounded sampling of real micro-events;
- shared fleet/player transforms;
- sparse alien kill events;
- long-lived declarative camera track.

## Failure policy

Generation fails rather than publishing misleading output if:

- the bot cannot clear the match within the frame budget;
- topology links reference missing nodes;
- generated SVG contains script/event-handler/JavaScript URLs;
- generated XML is malformed;
- deterministic tests diverge.
