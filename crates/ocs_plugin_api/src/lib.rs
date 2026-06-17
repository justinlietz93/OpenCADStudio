//! # Open CAD Studio plugin API
//!
//! The stable, semver-versioned contract an add-on package targets instead of
//! the `OpenCADStudio` binary internals. It is intentionally **dependency
//! free** (no `iced`, no `acadrust`) so engine crates and external tooling can
//! depend on it cheaply.
//!
//! Two pieces live here:
//!
//! - [`manifest`] — plugin identity ([`PluginManifest`]) and the host ABI
//!   version handshake ([`ApiVersion`]).
//! - [`ribbon`] — the [`CadModule`] trait and the plain-data ribbon types
//!   ([`RibbonGroup`], [`ToolDef`], …) a plugin uses to describe its tab.
//!
//! The runtime host surface a plugin uses at *dispatch* time (document access,
//! command line, undo) is `acadrust`-typed and therefore lives in the host
//! binary for now; see `docs/plugin-architecture.md` (phase 1b).

pub mod manifest;
pub mod ribbon;

/// Runtime host surface — only built with the `host` feature (pulls `acadrust`).
#[cfg(feature = "host")]
pub mod host;

pub use manifest::{ApiVersion, PluginManifest, API_VERSION};
pub use ribbon::{
    CadModule, IconKind, ModuleEvent, RibbonGroup, RibbonItem, StyleKey, ToolDef,
};
