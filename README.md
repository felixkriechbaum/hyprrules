# hyprrules

GTK4/libadwaita-Oberfläche für Hyprland Window- und Workspace-Rules (Lua-Config, Hyprland ≥ 0.55).

Beispiel: „Discord und Spotify öffnen immer auf Workspace 2, und Workspace 2 liegt auf Monitor `HDMI-A-1`“.
Daraus erzeugt hyprrules:

```lua
hl.workspace_rule({ workspace = "2", monitor = "HDMI-A-1" })

hl.window_rule({
    name      = "hyprrules-discord",
    match     = { class = "^(discord)$" },
    workspace = "2 silent",
})
```

## So funktioniert es

- Die Regeln liegen in `~/.config/hyprrules/rules.toml` (Quelle der Wahrheit).
- Beim Speichern wird `~/.config/hypr/hyprrules.lua` generiert und `hyprctl reload` ausgelöst.
- Deine eigene Config wird **nie** angefasst. Einmalig in `hyprland.lua` einbinden:

  ```lua
  require("hyprrules")
  ```

## Backends

| Umgebung | Backend |
|---|---|
| Linux in einer Hyprland-Session (`HYPRLAND_INSTANCE_SIGNATURE` gesetzt) | echte IPC über `$XDG_RUNTIME_DIR/hypr/<sig>/.socket.sock` |
| alles andere (z. B. Windows) | Fake-Hyprland mit zwei Monitoren und Beispiel-Apps |
| `HYPRRULES_BACKEND=fake` | erzwingt Fake, auch unter Hyprland |

Das Fake-Backend schreibt nur in eine Sandbox (`<config_dir>/hyprrules/fake/`).

## Projektstruktur

- `crates/hyprrules-core`: Modell, Lua-Generator, Speicherung, Backends (ohne GTK, testbar mit `cargo test -p hyprrules-core`)
- `crates/hyprrules-gtk`: Oberfläche (relm4 + libadwaita)

## Bauen

### Linux (Arch)

```sh
sudo pacman -S gtk4 libadwaita
cargo run -p hyprrules-gtk
```

### Windows (nur zum Entwickeln, Fake-Backend)

1. Fertiges GTK4-Bundle von [gvsbuild](https://github.com/wingtk/gvsbuild/releases) (`GTK4_Gvsbuild_*_x64.zip`) nach `C:\gtk` entpacken.
2. Umgebungsvariablen setzen: `PKG_CONFIG_PATH=C:\gtk\lib\pkgconfig`, `LIB=C:\gtk\lib`, `C:\gtk\bin` zum `PATH` hinzufügen.
3. `cargo run -p hyprrules-gtk`
