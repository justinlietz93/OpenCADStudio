# Wire Pipeline — Cold-Open Optimization Plan

Goal: reduce first-file-open latency (tessellate → upload → GPU buffer
creation) for wire entities. Steady-state draw cost is already cheap; the
bottleneck on cold open is **upload volume** and **vertex emission**.

## Baseline

Each wire segment currently produces **6 vertices × 96 B = 576 B**. The same
`pos_a / pos_b / color / half_width / pattern_length / pat0 / pat1` values
are duplicated across all 6 corners of the quad. For a drawing with 1 M
segments this is **~576 MB** of CPU-side `Vec<WireVertex>` plus the same
amount uploaded over PCIe.

Build path: `WireGpu::from_batch` (single-threaded loop) → `Vec<WireVertex>`
→ `create_buffer_init` (one alloc + one memcpy into a staging buffer the
driver then DMAs to VRAM).

## Step 1 — `mapped_at_creation` upload

Replace `create_buffer_init(contents)` with `create_buffer(mapped: true)` +
write the vertex stream directly into the mapped slice. Skips the
intermediate `Vec<WireVertex>` → staging-buffer memcpy.

- **Files**: `src/scene/pipeline/wire_gpu.rs`
- **Risk**: low (size known ahead of time; no semantic change).
- **Expected win**: 1 alloc + 1 memcpy per buffer dropped. Modest but free.

## Step 2 — Rayon parallel vertex emission

`from_batch` walks wires serially. Each wire's vertex generation is
independent (no shared state, no order dependency — wires are already
keyed by style upstream in `block_cache`). Use `rayon::par_iter().flat_map`
to emit per-wire vertex slices in parallel, then concatenate.

- **Files**: `src/scene/pipeline/wire_gpu.rs`
- **Risk**: low. Output ordering does not matter for correctness.
- **Expected win**: N-core speedup on multi-wire drawings; meaningful on
  files with many short wires (typical CAD geometry).

## Step 3 — Instanced rendering (the big win)

Replace 6-vertex quad expansion with **1 instance per segment** + 4-vertex
unit quad shared across all instances. Per-instance struct (~88 B) holds:
`pos_a, pos_b, color, distance_a, distance_b, half_width, pattern_length,
pat0, pat1`. `which_end` and `side` are derived from `@builtin(vertex_index)`
inside the vertex shader.

- **Files**: `src/scene/pipeline/wire_gpu.rs`, `src/shaders/wire.wgsl`,
  `src/scene/pipeline/mod.rs` (pipeline `VertexBufferLayout`).
- **Risk**: medium. Two vertex buffers (per-vertex + per-instance), shader
  derives `which_end` / `side` from `vertex_index`. `WireGpu::from_batch`
  emits 1 instance per segment instead of 6 verts.
- **Expected win**: **~6.5× less GPU memory & upload bandwidth per
  segment** (576 B → 88 B). Biggest cold-open lever.

## Step 4 — Tighter per-instance packing

After Step 3 the instance struct is the new hot path. Pack:

- `color: [f32; 4]` → `[u8; 4]` (`Unorm8x4`): 16 B → 4 B, no visible
  quality loss at 8-bit display.
- `which_end / side` already eliminated by Step 3.
- `pat0 / pat1` stay (encoder relies on up to 8 dash elements).

Net: ~88 B → ~76 B per instance. ~15 % further reduction on top of Step 3.

- **Files**: `src/scene/pipeline/wire_gpu.rs`, `src/shaders/wire.wgsl`.
- **Risk**: low (vertex-attribute format change only).
- **Expected win**: ~15 % less upload after Step 3.

## Ordering rationale

1-2 first — independent, low risk, immediate wins.
3 next — restructures the pipeline; depends on no prior changes but is the
single biggest lever.
4 last — only pays off after Step 3 turns the per-instance struct into the
hot path.

Each step ends with a `cargo check` (warnings-free) and a commit.
