use anyhow::Result;

use super::{Client, HyprBackend, Monitor};
use crate::store::Paths;

/// Pretends to be a two-monitor Hyprland setup with a few apps open.
/// Writes only into a sandbox directory (see [`Paths::fake`]).
pub struct FakeBackend {
    monitors: Vec<Monitor>,
    clients: Vec<Client>,
}

impl FakeBackend {
    pub fn new() -> Self {
        let monitor = |name: &str, description: &str, width, height, x, focused| Monitor {
            name: name.into(),
            description: description.into(),
            width,
            height,
            x,
            y: 0,
            scale: 1.0,
            focused,
        };
        let client = |class: &str, title: &str| Client {
            class: class.into(),
            title: title.into(),
        };
        Self {
            monitors: vec![
                monitor("DP-1", "Dell U2723QE", 2560, 1440, 0, true),
                monitor("HDMI-A-1", "LG 24GL600F", 1920, 1080, 2560, false),
            ],
            clients: vec![
                client("discord", "#general - Discord"),
                client("Spotify", "Spotify Premium"),
                client("firefox", "Mozilla Firefox"),
                client("kitty", "~/dev/hyprrules"),
                client("code", "Visual Studio Code"),
            ],
        }
    }
}

impl Default for FakeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl HyprBackend for FakeBackend {
    fn label(&self) -> String {
        "Fake-Hyprland".into()
    }

    fn monitors(&self) -> Result<Vec<Monitor>> {
        Ok(self.monitors.clone())
    }

    fn clients(&self) -> Result<Vec<Client>> {
        Ok(self.clients.clone())
    }

    fn workspaces(&self) -> Result<Vec<String>> {
        Ok(["1", "2", "3", "name:G"].map(String::from).to_vec())
    }

    fn reload(&self) -> Result<()> {
        Ok(())
    }

    fn paths(&self) -> Paths {
        Paths::fake()
    }
}
