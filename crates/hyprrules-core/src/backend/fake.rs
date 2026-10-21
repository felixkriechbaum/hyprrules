use std::cell::RefCell;

use anyhow::{Result, bail};

use super::{Client, HyprBackend, Monitor, WorkspaceRef};
use crate::binds::Bind;
use crate::store::Paths;

/// Pretends to be a two-monitor Hyprland setup with a few apps open.
/// Writes only into a sandbox directory (see [`Paths::fake`]).
pub struct FakeBackend {
    monitors: Vec<Monitor>,
    /// Interior mutability so moving windows shows up on the next refresh.
    clients: RefCell<Vec<Client>>,
    binds: Vec<Bind>,
}

fn workspace_ref(id: i64) -> WorkspaceRef {
    WorkspaceRef {
        id,
        name: id.to_string(),
    }
}

impl FakeBackend {
    pub fn new() -> Self {
        let monitor = |name: &str, description: &str, width, height, x, refresh_rate, shown, focused| Monitor {
            name: name.into(),
            description: description.into(),
            width,
            height,
            x,
            y: 0,
            scale: 1.0,
            focused,
            refresh_rate,
            transform: 0,
            active_workspace: Some(workspace_ref(shown)),
        };
        let client = |n: usize, class: &str, title: &str, workspace| Client {
            class: class.into(),
            title: title.into(),
            address: format!("0xfake{n}"),
            workspace: Some(workspace_ref(workspace)),
        };
        let bind = |modmask, key: &str, dispatcher: &str, arg: &str| Bind {
            modmask,
            key: key.into(),
            dispatcher: dispatcher.into(),
            arg: arg.into(),
            ..Bind::default()
        };
        let mut binds = vec![
            bind(64, "Return", "exec", "kitty"),
            bind(64, "Q", "killactive", ""),
            bind(64, "E", "exec", "nautilus"),
            bind(64, "V", "togglefloating", ""),
            bind(64, "F", "fullscreen", "0"),
        ];
        for n in 1..=3 {
            binds.push(bind(64, &n.to_string(), "workspace", &n.to_string()));
            binds.push(bind(65, &n.to_string(), "movetoworkspace", &n.to_string()));
        }
        binds.push(Bind {
            mouse: true,
            ..bind(64, "mouse:272", "movewindow", "")
        });
        binds.push(Bind {
            locked: true,
            repeat: true,
            description: "Volume up".into(),
            ..bind(0, "XF86AudioRaiseVolume", "exec", "wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%+")
        });
        binds.push(bind(64, "R", "submap", "resize"));
        binds.push(Bind {
            submap: "resize".into(),
            repeat: true,
            ..bind(0, "right", "resizeactive", "10 0")
        });
        binds.push(Bind {
            submap: "resize".into(),
            ..bind(0, "escape", "submap", "reset")
        });

        Self {
            monitors: vec![
                monitor("DP-1", "Dell U2723QE", 2560, 1440, 0, 59.95, 1, true),
                monitor("HDMI-A-1", "LG 24GL600F", 1920, 1080, 2560, 144.0, 2, false),
            ],
            clients: RefCell::new(vec![
                client(1, "discord", "#general - Discord", 3),
                client(2, "Spotify", "Spotify Premium", 2),
                client(3, "firefox", "Mozilla Firefox", 1),
                client(4, "kitty", "~/dev/hyprrules", 1),
                client(5, "kitty", "htop", 3),
                client(6, "code", "Visual Studio Code", 1),
            ]),
            binds,
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
        "Fake Hyprland".into()
    }

    fn monitors(&self) -> Result<Vec<Monitor>> {
        Ok(self.monitors.clone())
    }

    fn clients(&self) -> Result<Vec<Client>> {
        Ok(self.clients.borrow().clone())
    }

    fn workspaces(&self) -> Result<Vec<String>> {
        Ok(["1", "2", "3"].map(String::from).to_vec())
    }

    fn binds(&self) -> Result<Vec<Bind>> {
        Ok(self.binds.clone())
    }

    fn move_window(&self, address: &str, workspace: &str) -> Result<()> {
        let mut clients = self.clients.borrow_mut();
        let Some(client) = clients.iter_mut().find(|c| c.address == address) else {
            bail!("no window {address}");
        };
        let name = crate::workspace::label(workspace).to_owned();
        let id = workspace.parse().unwrap_or(-1);
        client.workspace = Some(WorkspaceRef { id, name });
        Ok(())
    }

    fn reload(&self) -> Result<()> {
        Ok(())
    }

    fn paths(&self) -> Paths {
        Paths::fake()
    }
}
