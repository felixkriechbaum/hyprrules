//! Core logic for hyprrules: rule model, Lua generation, storage and
//! access to Hyprland (real IPC on Linux, fake backend everywhere else).

pub mod backend;
pub mod lua;
pub mod model;
pub mod store;
pub mod workspace;

pub use backend::{Client, HyprBackend, Monitor};
pub use model::{RuleEdit, RuleSet, WindowRule, WorkspaceRule};
pub use store::Paths;
