use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::{Client, HyprBackend, Monitor};
use crate::binds::Bind;
use crate::store::Paths;
use crate::workspace;

/// Talks to Hyprland over its request socket (same protocol as `hyprctl`).
pub struct IpcBackend {
    socket: PathBuf,
}

impl IpcBackend {
    /// Returns `None` when not running inside a Hyprland session.
    pub fn from_env() -> Option<Self> {
        let signature = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
        let runtime = std::env::var("XDG_RUNTIME_DIR").ok()?;
        let socket = PathBuf::from(runtime)
            .join("hypr")
            .join(signature)
            .join(".socket.sock");
        socket.exists().then_some(Self { socket })
    }

    fn request(&self, command: &str) -> Result<String> {
        let mut stream = UnixStream::connect(&self.socket)
            .with_context(|| format!("connecting to {}", self.socket.display()))?;
        stream.write_all(command.as_bytes())?;
        let mut response = String::new();
        stream.read_to_string(&mut response)?;
        Ok(response)
    }

    fn request_json<T: for<'de> Deserialize<'de>>(&self, command: &str) -> Result<T> {
        let response = self.request(&format!("j/{command}"))?;
        serde_json::from_str(&response)
            .with_context(|| format!("unexpected answer to '{command}': {response}"))
    }
}

impl HyprBackend for IpcBackend {
    fn label(&self) -> String {
        #[derive(Deserialize)]
        struct Version {
            tag: String,
        }
        match self.request_json::<Version>("version") {
            Ok(v) => format!("Hyprland {}", v.tag),
            Err(_) => "Hyprland".into(),
        }
    }

    fn monitors(&self) -> Result<Vec<Monitor>> {
        self.request_json("monitors")
    }

    fn clients(&self) -> Result<Vec<Client>> {
        let mut clients: Vec<Client> = self.request_json("clients")?;
        clients.retain(|c| !c.class.is_empty());
        Ok(clients)
    }

    fn workspaces(&self) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Workspace {
            id: i64,
            name: String,
        }
        #[derive(Deserialize)]
        struct WorkspaceRule {
            #[serde(rename = "workspaceString")]
            workspace: String,
        }

        // Workspaces that currently exist.
        let mut found: Vec<String> = self
            .request_json::<Vec<Workspace>>("workspaces")?
            .iter()
            .filter_map(|w| workspace::from_ipc(w.id, &w.name))
            .collect();
        // Workspaces defined in the config, even if empty right now. Best
        // effort: the answer format may differ between Hyprland versions.
        if let Ok(rules) = self.request_json::<Vec<WorkspaceRule>>("workspacerules") {
            found.extend(rules.iter().filter_map(|r| workspace::from_config(&r.workspace)));
        }
        Ok(workspace::sorted(found))
    }

    fn binds(&self) -> Result<Vec<Bind>> {
        self.request_json("binds")
    }

    fn move_window(&self, address: &str, workspace: &str) -> Result<()> {
        let answer = self.request(&format!("dispatch movetoworkspacesilent {workspace},address:{address}"))?;
        if answer.trim() != "ok" {
            bail!("moving window failed: {answer}");
        }
        Ok(())
    }

    fn reload(&self) -> Result<()> {
        let answer = self.request("reload")?;
        if answer.trim() != "ok" {
            bail!("Hyprland reload failed: {answer}");
        }
        Ok(())
    }

    fn paths(&self) -> Paths {
        Paths::system()
    }
}
