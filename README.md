# hyprrules

A GTK4/libadwaita UI for Hyprland window and workspace rules (Lua config, Hyprland ≥ 0.55).

Example: "Discord and Spotify always open on workspace 2, and workspace 2 lives on monitor `HDMI-A-1`".
hyprrules turns that into:

```lua
hl.workspace_rule({ workspace = "2", monitor = "HDMI-A-1" })

hl.window_rule({
    name      = "hyprrules-discord",
    match     = { class = "^(discord)$" },
    workspace = "2 silent",
})
```

## Features

- **App rules** per window class: workspace (optionally silent), floating, fixed size, opacity, pin.
  Pick the class from running windows instead of writing regex by hand.
- **Workspace → monitor** bindings, using the monitors Hyprland actually reports.
- **Workspace detection**: numbered and named workspaces (e.g. `name:G`) are read from Hyprland
  (existing workspaces plus those defined in the config); anything else can be typed in.
- **Drag & drop ordering**: later rules take precedence, just like in Hyprland.

## How it works

- Rules are stored in `~/.config/hyprrules/rules.toml` (the source of truth).
- On save, `~/.config/hypr/hyprrules.lua` is generated and Hyprland is asked to reload.
- Your own config is **never** modified. Include the generated module once in `hyprland.lua`:

  ```lua
  require("hyprrules")
  ```

## Backends

| Environment | Backend |
|---|---|
| Linux inside a Hyprland session (`HYPRLAND_INSTANCE_SIGNATURE` set) | real IPC via `$XDG_RUNTIME_DIR/hypr/<sig>/.socket.sock` |
| anything else (e.g. Windows) | fake Hyprland with two monitors and sample apps |
| `HYPRRULES_BACKEND=fake` | forces the fake backend, even inside Hyprland |

The fake backend only writes into a sandbox (`<config_dir>/hyprrules/fake/`).

## Project layout

- `crates/hyprrules-core`: model, Lua generator, storage, backends (no GTK; test with `cargo test -p hyprrules-core`)
- `crates/hyprrules-gtk`: the UI (relm4 + libadwaita)

## Building

### Linux (Arch)

```sh
sudo pacman -S gtk4 libadwaita
cargo run -p hyprrules-gtk
```

### Windows (development only, fake backend)

1. Extract the prebuilt GTK4 bundle from [gvsbuild](https://github.com/wingtk/gvsbuild/releases) (`GTK4_Gvsbuild_*_x64.zip`) to `C:\gtk`.
2. Set environment variables: `PKG_CONFIG_PATH=C:\gtk\lib\pkgconfig`, `LIB=C:\gtk\lib`, and add `C:\gtk\bin` to `PATH`.
3. `cargo run -p hyprrules-gtk`
