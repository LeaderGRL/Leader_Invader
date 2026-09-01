# Editor architecture

Leader Invader is both a machine simulator and an editor, but those responsibilities must remain separate.

## Non-negotiable authority boundary

`leader-core` owns the physical machine:

- CPU, microcode and architectural state;
- RAM, ROM, VRAM, MMIO and device semantics;
- canonical physical node/link topology;
- canonical hierarchy, routing and hardware hit-test semantics;
- deterministic native traces and framebuffer representation.

The editor must never rewrite those semantics in JavaScript or silently mutate the canonical topology used to execute a match.

## Workspace overlay

`leader-explorer::WorkspaceLayout` is the first editor-state layer. It is intentionally presentation-only.

It currently owns:

- per-node `(dx, dy)` offsets relative to canonical physical bounds;
- user-created groups with stable ids and labels;
- exclusive user-group membership;
- group movement implemented as member offsets;
- deterministic JSON snapshots;
- a monotonic workspace revision.

A reset discards the overlay and reveals the original physical layout exactly.

### Why offsets instead of replacing bounds?

The physical layout remains a contract checked by `leader-core`. Storing offsets makes the distinction explicit:

```text
canonical physical bounds  +  user workspace offset  =  presentation bounds
```

This prevents an editor drag from accidentally changing execution topology, hardware ownership or validation.

## Next editor layers

The following features should build on the same separation:

1. **Browser manipulation** — drag selected nodes, multi-select, move groups, box-select and snap-to-grid by calling `WorkspaceLayout`; JavaScript performs only pointer-coordinate transforms.
2. **Workspace persistence** — export/import a versioned editor document containing offsets, groups, camera state and editor preferences. Loading must validate every referenced canonical node id in Rust.
3. **Workspace routing** — render presentation routes between moved nodes without replacing the canonical hardware route. The UI must make the distinction between physical/canonical and workspace presentation routes inspectable.
4. **User circuits** — custom gates, nodes and links must live in a separate editable circuit model with an explicit compilation/validation step before they can become executable hardware. They must not be inserted ad hoc into the running canonical machine.
5. **Reusable components** — user circuits may later become component definitions/instances, again through a Rust-owned typed netlist and deterministic compile step.

## Proposed editable-circuit pipeline

```text
Editor document
    ↓ validate ids/types/ports
Rust editable netlist
    ↓ elaborate components
Typed primitive netlist
    ↓ validate widths + drivers + cycles
Executable machine definition
    ↓ instantiate
Simulator / trace / topology
```

The browser edits the document. Rust validates and compiles it. Only the compiled Rust representation may affect simulation.

## Acceptance criteria for the editor milestone

- moving/grouping canonical nodes cannot change native match traces;
- reset restores byte-for-byte canonical presentation coordinates;
- exported workspace state is deterministic and versioned;
- invalid node ids, coordinates, group ids and malformed documents are rejected in Rust;
- user-created executable circuits use typed ports and bit widths;
- no JavaScript ALU, memory-map, bus, device or game semantics;
- custom-circuit compilation has corruption/negative tests before it is allowed to drive the simulator.
