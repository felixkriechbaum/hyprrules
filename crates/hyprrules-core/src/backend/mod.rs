//! Access to the running compositor.
//!
//! On Linux inside a Hyprland session the real IPC socket is used; everywhere
//! else (e.g. Windows during development) a fake backend with sample data.

mod fake;
#[cfg(unix)]
mod ipc;

use anyhow::Result;
use serde::Deserialize;

use crate::binds::Bind;
use crate::store::Paths;
use crate::workspace;

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
    #[serde(default, rename = "refreshRate")]
    pub refresh_rate: f64,
    /// wl_output transform: odd values are rotated by 90° or 270°.
    #[serde(default)]
    pub transform: u8,
    #[serde(default, rename = "activeWorkspace")]
    pub active_workspace: Option<WorkspaceRef>,
}

impl Monitor {
    /// Size in layout coordinates, the space `x` and `y` live in:
    /// scaled down and rotated the way Hyprland places the monitor.
    pub fn logical_size(&self) -> (f64, f64) {
        let scale = if self.scale > 0.0 { self.scale } else { 1.0 };
        let (w, h) = (self.width as f64 / scale, self.height as f64 / scale);
        if self.transform % 2 == 1 { (h, w) } else { (w, h) }
    }

    /// Selector of the workspace the monitor shows right now.
    pub fn shown_workspace(&self) -> Option<String> {
        self.active_workspace.as_ref().and_then(WorkspaceRef::selector)
    }
}

/// A workspace as Hyprland references it inside other objects.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WorkspaceRef {
    pub id: i64,
    #[serde(default)]
    pub name: String,
}

impl WorkspaceRef {
    pub fn selector(&self) -> Option<String> {
        workspace::from_ipc(self.id, &self.name)
    }
}

/// One open window.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Client {
    pub class: String,
    #[serde(default)]
    pub title: String,
    /// Window address (`0x…`), used to move this exact window.
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub workspace: Option<WorkspaceRef>,
}

impl Client {
    /// Selector of the workspace the window is on; `None` for special ones.
    pub fn workspace(&self) -> Option<String> {
        self.workspace.as_ref().and_then(WorkspaceRef::selector)
    }
}

fn one() -> f64 {
    1.0
}

pub trait HyprBackend {
    /// Short label shown in the UI, e.g. "Hyprland 0.55" or "Fake".
    fn label(&self) -> String;
    fn monitors(&self) -> Result<Vec<Monitor>>;
    /// All open windows, one entry per window.
    fn clients(&self) -> Result<Vec<Client>>;
    /// Known workspaces as selectors (`"2"`, `"name:G"`), sorted.
    fn workspaces(&self) -> Result<Vec<String>>;
    /// Key and mouse bindings in config order.
    fn binds(&self) -> Result<Vec<Bind>>;
    /// Moves a window (by address) to a workspace without following it.
    fn move_window(&self, address: &str, workspace: &str) -> Result<()>;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotated_monitors_swap_their_sides() {
        let json = r#"{"name": "DP-2", "width": 2560, "height": 1440, "x": 0, "y": 0,
            "scale": 2.0, "transform": 1, "refreshRate": 143.97,
            "activeWorkspace": {"id": 4, "name": "4"}}"#;
        let monitor: Monitor = serde_json::from_str(json).unwrap();
        assert_eq!(monitor.logical_size(), (720.0, 1280.0));
        assert_eq!(monitor.shown_workspace().as_deref(), Some("4"));
    }

    #[test]
    fn clients_on_special_workspaces_have_none() {
        let json = r#"{"class": "kitty", "address": "0x1", "workspace": {"id": -98, "name": "special:term"}}"#;
        let client: Client = serde_json::from_str(json).unwrap();
        assert_eq!(client.workspace(), None);
    }
}
