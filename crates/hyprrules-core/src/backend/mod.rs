//! Access to the running compositor.
//!
//! On Linux inside a Hyprland session the real IPC socket is used; everywhere
//! else (e.g. Windows during development) a fake backend with sample data.

mod fake;
#[cfg(unix)]
mod ipc;

use anyhow::Result;
use serde::Deserialize;

use crate::store::Paths;

pub use fake::FakeBackend;
#[cfg(unix)]
pub use ipc::IpcBackend;

/// Set to `fake` to force the fake backend, even inside Hyprland.
pub const BACKEND_ENV: &str = "HYPRRULES_BACKEND";

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Monitor {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    #[serde(default = "one")]
    pub scale: f64,
    #[serde(default)]
    pub focused: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Client {
    pub class: String,
    #[serde(default)]
    pub title: String,
}

fn one() -> f64 {
    1.0
}

pub trait HyprBackend {
    /// Short label shown in the UI, e.g. "Hyprland 0.55" or "Fake".
    fn label(&self) -> String;
    fn monitors(&self) -> Result<Vec<Monitor>>;
    fn clients(&self) -> Result<Vec<Client>>;
    /// Known workspaces as selectors (`"2"`, `"name:G"`), sorted.
    fn workspaces(&self) -> Result<Vec<String>>;
    /// Asks Hyprland to re-read its config.
    fn reload(&self) -> Result<()>;
    fn paths(&self) -> Paths;
}

/// Picks the real backend inside a Hyprland session, the fake one otherwise.
pub fn detect() -> Box<dyn HyprBackend> {
    if std::env::var(BACKEND_ENV).is_ok_and(|v| v == "fake") {
        return Box::new(FakeBackend::new());
    }
    #[cfg(unix)]
    if let Some(ipc) = IpcBackend::from_env() {
        return Box::new(ipc);
    }
    Box::new(FakeBackend::new())
}
