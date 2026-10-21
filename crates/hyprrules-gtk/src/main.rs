#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod binds_page;
mod desk;
mod monitors_page;
mod rule_row;
mod ui;
mod workspace_picker;

use hyprrules_core::backend::{self, HyprBackend};
use hyprrules_core::{Bind, Client, Monitor, Paths, RuleEdit, RuleSet, WindowRule, workspace};
use relm4::adw::prelude::*;
use relm4::gtk::gdk;
use relm4::{ComponentParts, ComponentSender, RelmApp, SimpleComponent, adw, gtk};

use desk::DeskState;
use ui::Ui;

const APP_ID: &str = "dev.felixkriechbaum.hyprrules";

fn main() {
    RelmApp::new(APP_ID).run::<App>(());
}

pub struct App {
    backend: Box<dyn HyprBackend>,
    paths: Paths,
    rules: RuleSet,
    monitors: Vec<Monitor>,
    /// Every open window.
    windows: Vec<Client>,
    /// Running apps, one entry per window class.
    clients: Vec<Client>,
    binds: Vec<Bind>,
    /// Workspaces Hyprland reports, as selectors.
    workspaces: Vec<String>,
    /// Rule shown in the desk's inspector.
    selected: Option<usize>,
    dirty: bool,
    included: bool,
    ui: Ui,
}

#[derive(Debug, Clone)]
pub enum Msg {
    ClientPicked(u32),
    AddWindowRule,
    ToggleWindowRule(usize, bool),
    EditWindowRule(usize, RuleEdit),
    RemoveWindowRule(usize),
    /// Drag & drop: move rule `from` to position `to`.
    MoveWindowRule(usize, usize),
    AddWorkspaceRule,
    RemoveWorkspaceRule(usize),
    // Desk
    SelectRule(Option<usize>),
    /// Running app (by class) dropped on a workspace.
    PlaceApp(String, String),
    PlaceRule(usize, String),
    /// Rule dragged back to the tray.
    UnplaceRule(usize),
    /// Workspace dropped on a monitor, or on the shelf (None).
    BindWorkspace(String, Option<String>),
    AddApp(String),
    /// Move open windows to the workspace their rule assigns.
    ApplyToWindows,
    Refresh,
    Save,
}

impl SimpleComponent for App {
    type Init = ();
    type Input = Msg;
    type Output = ();
    type Root = adw::ApplicationWindow;
    type Widgets = ();

    fn init_root() -> Self::Root {
        // Lets `cargo run` find the icon without installing it.
        if let Some(display) = gdk::Display::default() {
            gtk::IconTheme::for_display(&display).add_search_path(concat!(env!("CARGO_MANIFEST_DIR"), "/data/icons"));
        }
        gtk::Window::set_default_icon_name(APP_ID);
        adw::ApplicationWindow::builder()
            .title("hyprrules")
            .icon_name(APP_ID)
            .default_width(1180)
            .default_height(840)
            .build()
    }

    fn init(_: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        relm4::set_global_css(include_str!("style.css"));
        let backend = backend::detect();
        let paths = backend.paths();
        let ui = Ui::build(&root, &backend.label(), &sender);

        let mut errors = Vec::new();
        let rules = paths.load().unwrap_or_else(|e| {
            errors.push(format!("Could not load rules: {e:#}"));
            RuleSet::default()
        });

        let mut model = App {
            included: paths.is_included(),
            backend,
            paths,
            rules,
            monitors: Vec::new(),
            windows: Vec::new(),
            clients: Vec::new(),
            binds: Vec::new(),
            workspaces: Vec::new(),
            selected: None,
            dirty: false,
            ui,
        };
        model.refresh_from_hyprland();
        model.rebuild_lists(&sender);
        model.sync_state();
        for error in errors {
            model.toast(&error);
        }

        ComponentParts { model, widgets: () }
    }

    fn update(&mut self, msg: Msg, sender: ComponentSender<Self>) {
        debug(&format!("{msg:?}"));
        match msg {
            Msg::ClientPicked(index) => {
                // Entry 0 is "enter manually".
                if let Some(client) = index.checked_sub(1).and_then(|i| self.clients.get(i as usize)) {
                    self.ui.class_entry.set_text(&client.class);
                }
            }
            Msg::AddWindowRule => {
                let class = self.ui.class_entry.text().trim().to_owned();
                if class.is_empty() {
                    self.toast("Enter a window class or pick a running app");
                    return;
                }
                let Some(workspace) = self.ui.app_workspace.selected() else {
                    self.toast("Pick a workspace");
                    return;
                };
                let mut rule = WindowRule::for_class(&class, workspace);
                rule.silent = self.ui.silent.is_active();
                self.rules.add_window_rule(rule);
                self.ui.class_entry.set_text("");
                self.ui.client_combo.set_selected(0);
                self.changed(&sender);
            }
            Msg::ToggleWindowRule(index, enabled) => {
                if let Some(rule) = self.rules.window_rules.get_mut(index) {
                    rule.enabled = enabled;
                    self.edited(index, &sender);
                }
            }
            Msg::EditWindowRule(index, edit) => {
                if let Some(rule) = self.rules.window_rules.get_mut(index) {
                    rule.apply(edit);
                    self.edited(index, &sender);
                }
            }
            Msg::SelectRule(index) => {
                self.selected = index;
                self.rebuild_desk(&sender);
            }
            Msg::PlaceApp(class, workspace) => {
                self.selected = Some(self.rules.place_app(&class, &workspace));
                self.changed(&sender);
            }
            Msg::PlaceRule(index, workspace) => {
                if let Some(rule) = self.rules.window_rules.get_mut(index) {
                    rule.workspace = Some(workspace);
                    self.changed(&sender);
                }
            }
            Msg::UnplaceRule(index) => {
                if self.rules.unplace_rule(index) {
                    self.selected = None;
                }
                self.changed(&sender);
            }
            Msg::BindWorkspace(workspace, monitor) => {
                match monitor {
                    Some(monitor) => self.rules.set_workspace_monitor(&workspace, &monitor),
                    None => self.rules.unbind_workspace(&workspace),
                }
                self.changed(&sender);
            }
            Msg::AddApp(class) => {
                self.selected = Some(match self.rules.rule_for_class(&class) {
                    Some(index) => index,
                    None => {
                        let mut rule = WindowRule::for_class(&class, "");
                        rule.workspace = None;
                        self.rules.add_window_rule(rule);
                        self.rules.window_rules.len() - 1
                    }
                });
                self.changed(&sender);
            }
            Msg::RemoveWindowRule(index) => {
                if index < self.rules.window_rules.len() {
                    self.rules.window_rules.remove(index);
                    self.selected = None;
                    self.changed(&sender);
                }
            }
            Msg::MoveWindowRule(from, to) => {
                if from != to {
                    self.rules.move_window_rule(from, to);
                    self.selected = None;
                    self.changed(&sender);
                }
            }
            Msg::AddWorkspaceRule => {
                let Some(monitor) = self.monitors.get(self.ui.monitor_combo.selected() as usize) else {
                    self.toast("No monitor selected");
                    return;
                };
                let Some(workspace) = self.ui.ws_workspace.selected() else {
                    self.toast("Pick a workspace");
                    return;
                };
                let monitor = monitor.name.clone();
                self.rules.set_workspace_monitor(&workspace, &monitor);
                self.changed(&sender);
            }
            Msg::RemoveWorkspaceRule(index) => {
                if index < self.rules.workspace_rules.len() {
                    self.rules.workspace_rules.remove(index);
                    self.changed(&sender);
                }
            }
            Msg::ApplyToWindows => {
                let moves = self.rules.moves(&self.windows);
                let mut moved = 0;
                for (address, workspace) in &moves {
                    match self.backend.move_window(address, workspace) {
                        Ok(()) => moved += 1,
                        Err(e) => {
                            self.toast(&format!("Could not move a window: {e:#}"));
                            break;
                        }
                    }
                }
                if moved > 0 {
                    self.toast(&format!("Moved {}", windows(moved)));
                }
                self.refresh_from_hyprland();
                self.rebuild_desk(&sender);
                self.sync_state();
            }
            Msg::Refresh => {
                self.refresh_from_hyprland();
                self.included = self.paths.is_included();
                self.rebuild_lists(&sender);
                self.sync_state();
            }
            Msg::Save => self.save(),
        }
    }
}

impl App {
    fn changed(&mut self, sender: &ComponentSender<Self>) {
        self.dirty = true;
        self.rebuild_lists(sender);
        self.sync_state();
    }

    fn save(&mut self) {
        if let Err(e) = self.paths.save(&self.rules) {
            self.toast(&format!("Saving failed: {e:#}"));
            return;
        }
        self.dirty = false;
        self.included = self.paths.is_included();
        self.sync_state();

        let file = self.paths.lua_file();
        match self.backend.reload() {
            Ok(()) => self.toast(&format!("Saved: {}", file.display())),
            Err(e) => self.toast(&format!("Saved, but reload failed: {e:#}")),
        }
    }

    fn refresh_from_hyprland(&mut self) {
        match self.backend.monitors() {
            Ok(monitors) => self.monitors = monitors,
            Err(e) => self.toast(&format!("Could not read monitors: {e:#}")),
        }
        match self.backend.clients() {
            Ok(windows) => {
                let mut clients = windows.clone();
                clients.sort_by_key(|c| c.class.to_lowercase());
                clients.dedup_by(|a, b| a.class == b.class);
                self.clients = clients;
                self.windows = windows;
            }
            Err(e) => self.toast(&format!("Could not read open windows: {e:#}")),
        }
        match self.backend.workspaces() {
            Ok(workspaces) => self.workspaces = workspaces,
            Err(e) => self.toast(&format!("Could not read workspaces: {e:#}")),
        }
        match self.backend.binds() {
            Ok(binds) => self.binds = binds,
            Err(e) => self.toast(&format!("Could not read keybindings: {e:#}")),
        }
        debug(&format!(
            "monitors {:?}, workspaces {:?}, {} windows, {} binds",
            self.monitors.iter().map(|m| (&m.name, m.x, m.y, m.width, m.height)).collect::<Vec<_>>(),
            self.workspaces,
            self.windows.len(),
            self.binds.len()
        ));
        self.ui.set_pickers(&self.clients, &self.monitors);
        self.ui.binds.set(&self.binds);
        let monitors_file = self
            .paths
            .monitors_file()
            .and_then(|path| std::fs::read_to_string(&path).ok().map(|text| (path, text)));
        self.ui.monitors.set(&self.monitors, monitors_file, &self.paths.hypr_dir);
    }

    /// Workspaces from Hyprland plus those already used in rules, so a
    /// rule's workspace is always selectable.
    fn known_workspaces(&self) -> Vec<String> {
        let mut known = self.workspaces.clone();
        known.extend(self.rules.window_rules.iter().filter_map(|r| r.workspace.clone()));
        known.extend(self.rules.workspace_rules.iter().map(|r| r.workspace.clone()));
        workspace::sorted(known)
    }

    /// Rebuilds every view: desk, inspector and rule list.
    fn rebuild_lists(&self, sender: &ComponentSender<Self>) {
        let workspaces = self.known_workspaces();
        self.ui.set_workspace_choices(&workspaces);
        self.ui.set_rules(&self.rules, &self.monitors, &workspaces, sender);
        self.ui.set_inspector(self.selected, &self.rules, &workspaces, sender);
        self.rebuild_desk(sender);
    }

    fn rebuild_desk(&self, sender: &ComponentSender<Self>) {
        let workspaces = self.known_workspaces();
        let state = DeskState {
            rules: &self.rules,
            monitors: &self.monitors,
            workspaces: &workspaces,
            windows: &self.windows,
            selected: self.selected,
        };
        self.ui.desk.rebuild(&state, sender);
        if self.selected.is_some() {
            self.ui.set_inspector(self.selected, &self.rules, &workspaces, sender);
        }
    }

    /// After an edit inside an open editor: refresh everything except the
    /// editor being used, so it doesn't collapse or lose focus.
    fn edited(&mut self, index: usize, sender: &ComponentSender<Self>) {
        self.dirty = true;
        let workspaces = self.known_workspaces();
        let state = DeskState {
            rules: &self.rules,
            monitors: &self.monitors,
            workspaces: &workspaces,
            windows: &self.windows,
            selected: self.selected,
        };
        self.ui.desk.rebuild(&state, sender);
        if self.ui.desk_visible() {
            self.ui.set_rules(&self.rules, &self.monitors, &workspaces, sender);
        } else {
            self.ui.update_summary(index, &self.rules);
            self.ui.set_inspector(self.selected, &self.rules, &workspaces, sender);
        }
        self.sync_state();
    }

    fn sync_state(&self) {
        self.ui.save.set_sensitive(self.dirty);
        self.ui.banner.set_revealed(!self.included);
        let moves = self.rules.moves(&self.windows).len();
        self.ui.apply.set_sensitive(moves > 0);
        self.ui.apply.set_tooltip_text(Some(&if moves == 0 {
            "Every open window is where its rule wants it".to_owned()
        } else {
            format!("Move {} to the workspace their rule assigns", windows(moves))
        }));
    }

    fn toast(&self, text: &str) {
        self.ui
            .toasts
            .add_toast(adw::Toast::builder().title(text).use_markup(false).build());
    }
}

/// Prints to stderr when `HYPRRULES_DEBUG` is set.
fn debug(text: &str) {
    if std::env::var_os("HYPRRULES_DEBUG").is_some() {
        eprintln!("hyprrules: {text}");
    }
}

fn windows(n: usize) -> String {
    if n == 1 { "1 window".into() } else { format!("{n} windows") }
}
