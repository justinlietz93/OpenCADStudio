# Add-on plugin template

Copy this folder to `src/modules/<your_module>/` and rename placeholders.

## Quick start

1. Copy files into `src/modules/my_plugin/`.
2. Replace `MY_PLUGIN`, `my_plugin`, `opencad.my_plugin`, `MP_` throughout.
3. Add `pub mod my_plugin;` to `src/modules/mod.rs`.
4. `cargo build` — ribbon tab and commands register automatically.

## Required files

| File | Purpose |
|------|---------|
| `plugin.toml` | Metadata (QGIS-style); excludes dir from `build.rs` ribbon scan |
| `manifest.rs` | Compile-time `PluginManifest` — keep in sync with `plugin.toml` |
| `register.rs` | `inventory::submit!(PluginRegistration { … })` only |
| `plugin.rs` | Thin `BuiltinPlugin` impl |
| `dispatch.rs` | All command handlers |
| `mod.rs` | `CadModule` ribbon + `CommandRegistration` for autocomplete |
| `PLUGIN.md` | XDATA schemas and command reference |

## Optional

| File | Purpose |
|------|---------|
| `state.rs` | Per-document tab state via `host.ensure_plugin_state(PLUGIN_ID, …)` |
| `crates/my_engine/` | Headless domain logic (no iced/acadrust) |

See `docs/plugin-architecture.md` for the full spec.