#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod rule_row;
mod ui;

use hyprrules_core::backend::{self, HyprBackend};
use hyprrules_core::{Client, Monitor, Paths, RuleEdit, RuleSet, WindowRule};
use relm4::adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, RelmApp, SimpleComponent, adw};

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
    /// Running apps, one entry per window class.
    clients: Vec<Client>,
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
        adw::ApplicationWindow::builder()
            .title("hyprrules")
            .default_width(760)
            .default_height(860)
            .build()
    }

    fn init(_: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let backend = backend::detect();
        let paths = backend.paths();
        let ui = Ui::build(&root, &backend.label(), &sender);

        let mut errors = Vec::new();
        let rules = paths.load().unwrap_or_else(|e| {
            errors.push(format!("Regeln konnten nicht geladen werden: {e:#}"));
            RuleSet::default()
        });

        let mut model = App {
            included: paths.is_included(),
            backend,
            paths,
            rules,
            monitors: Vec::new(),
            clients: Vec::new(),
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
                    self.toast("Bitte eine Fensterklasse angeben oder eine laufende App wählen");
                    return;
                }
                let workspace = (self.ui.app_workspace.value() as u32).to_string();
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
                    self.dirty = true;
                    self.sync_state();
                }
            }
            Msg::EditWindowRule(index, edit) => {
                if let Some(rule) = self.rules.window_rules.get_mut(index) {
                    rule.apply(edit);
                    self.dirty = true;
                    self.ui.update_summary(index, &self.rules);
                    self.sync_state();
                }
            }
            Msg::RemoveWindowRule(index) => {
                if index < self.rules.window_rules.len() {
                    self.rules.window_rules.remove(index);
                    self.changed(&sender);
                }
            }
            Msg::MoveWindowRule(from, to) => {
                if from != to {
                    self.rules.move_window_rule(from, to);
                    self.changed(&sender);
                }
            }
            Msg::AddWorkspaceRule => {
                let Some(monitor) = self.monitors.get(self.ui.monitor_combo.selected() as usize) else {
                    self.toast("Kein Monitor ausgewählt");
                    return;
                };
                let workspace = (self.ui.ws_workspace.value() as u32).to_string();
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
            self.toast(&format!("Speichern fehlgeschlagen: {e:#}"));
            return;
        }
        self.dirty = false;
        self.included = self.paths.is_included();
        self.sync_state();

        let file = self.paths.lua_file();
        match self.backend.reload() {
            Ok(()) => self.toast(&format!("Gespeichert: {}", file.display())),
            Err(e) => self.toast(&format!("Gespeichert, aber Reload fehlgeschlagen: {e:#}")),
        }
    }

    fn refresh_from_hyprland(&mut self) {
        match self.backend.monitors() {
            Ok(monitors) => self.monitors = monitors,
            Err(e) => self.toast(&format!("Monitore nicht lesbar: {e:#}")),
        }
        match self.backend.clients() {
            Ok(mut clients) => {
                clients.sort_by_key(|c| c.class.to_lowercase());
                clients.dedup_by(|a, b| a.class == b.class);
                self.clients = clients;
            }
            Err(e) => self.toast(&format!("Laufende Apps nicht lesbar: {e:#}")),
        }
        self.ui.set_pickers(&self.clients, &self.monitors);
    }

    fn rebuild_lists(&self, sender: &ComponentSender<Self>) {
        self.ui.set_rules(&self.rules, &self.monitors, sender);
    }

    fn sync_state(&self) {
        self.ui.save.set_sensitive(self.dirty);
        self.ui.banner.set_revealed(!self.included);
    }

    fn toast(&self, text: &str) {
        self.ui
            .toasts
            .add_toast(adw::Toast::builder().title(text).use_markup(false).build());
    }
}
