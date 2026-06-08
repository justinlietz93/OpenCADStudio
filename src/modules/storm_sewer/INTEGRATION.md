# Storm Sewer module — integration plan

The ribbon tab (`mod.rs`) and the engine bridge (`analysis.rs`) are in place.
What remains is wiring each `SS_*` ribbon command to a handler in the host
command system.

## Architecture

```
storm_sewer/mod.rs      ribbon tab  →  emits ModuleEvent::Command("SS_*")
        │
host command dispatch   (src/app/commands.rs / src/command/)
        │
storm_sewer/analysis.rs builds a stormsewer::Network and runs the engine
        │
stormsewer crate        Rational + Manning + HGL  →  Analysis + report text
```

The network/hydraulics data is **not** a native `acadrust` entity, so it is
carried as side data keyed to drawing entities:

| Network object | Drawing representation | XDATA / fields |
|----------------|------------------------|----------------|
| Node (inlet/junction/outfall) | block insert at the structure point | invert, rim, area_ac, C, tc_inlet, kind |
| Pipe (link) | LINE/LWPOLYLINE between two structures | diameter, n (length from geometry) |

## Command handlers — status

All `SS_*` commands are wired in `src/app/commands.rs::dispatch_command`:

| Command   | Status | Action |
|-----------|--------|--------|
| `SS_INLET`/`SS_JUNCTION`/`SS_OUTFALL` | ✅ done | interactive `PlaceStructure` — pick points, drop circle markers (repeatable) |
| `SS_PIPE` | ✅ done | interactive `PlacePipe` — chain line segments between structures |
| `SS_ANALYZE` | ✅ done | open a `.ssn` file → run engine → draw plan (pipes/markers/labels) + print report |
| `SS_REPORT` | ✅ done | open a `.ssn` file → print `report::format_analysis()` to the command line |
| `SS_PROFILE` | ✅ done | open a `.ssn` file → draw the HGL/invert/ground long-section of the main stem |

Drafting (`structures.rs`) and analysis/drawing (`analysis.rs`) are both live and
unit-tested. `SS_ANALYZE`/`SS_REPORT`/`SS_PROFILE` read a user-authored `.ssn`
network file (format in `stormsewer::parse`); the engine runs on real data.

## Remaining enhancement: drive analysis from canvas geometry

The placement commands (`SS_INLET`/`PIPE`/…) draw markers and pipe lines but do
not yet carry hydraulic data, so analysis is driven by the `.ssn` file rather
than by the drawn entities. To let users analyze what they draw directly, attach
data via **XDATA** when `PlaceStructure`/`PlacePipe` commit (invert/rim/area/C on
structures, diameter/n on pipes), then have `SS_ANALYZE` walk the document
entities into a `stormsewer::Network`. The `.ssn` path remains the simple,
file-based workflow.

## Build

`build.rs` auto-discovers this directory (`storm_sewer/` → `StormSewerModule`)
and regenerates `src/modules/registry.rs`, so the tab appears on `cargo build`.
