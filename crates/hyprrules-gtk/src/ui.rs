//! Widget construction. The [`App`] owns a [`Ui`] and reads form values
//! from it directly; rule lists are rebuilt whenever the rules change.

use hyprrules_core::{Client, Monitor, RuleSet};
use relm4::adw::prelude::*;
use relm4::gtk::gdk;
use relm4::{ComponentSender, adw, gtk};

use crate::{App, Msg, rule_row};

pub const MAX_WORKSPACE: f64 = 10.0;

pub struct Ui {
    pub toasts: adw::ToastOverlay,
    pub banner: adw::Banner,
    pub save: gtk::Button,
    window_rules: gtk::ListBox,
    workspace_rules: gtk::ListBox,
    pub client_combo: adw::ComboRow,
    pub class_entry: adw::EntryRow,
    pub app_workspace: adw::SpinRow,
    pub silent: adw::SwitchRow,
    pub ws_workspace: adw::SpinRow,
    pub monitor_combo: adw::ComboRow,
}

impl Ui {
    pub fn build(window: &adw::ApplicationWindow, backend: &str, sender: &ComponentSender<App>) -> Self {
        let save = gtk::Button::builder()
            .label("Speichern & neu laden")
            .css_classes(["suggested-action"])
            .sensitive(false)
            .build();
        connect(&save, sender, Msg::Save);

        let refresh = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Laufende Apps und Monitore neu einlesen")
            .build();
        connect(&refresh, sender, Msg::Refresh);

        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&adw::WindowTitle::new("hyprrules", backend)));
        header.pack_start(&refresh);
        header.pack_end(&save);

        let banner = adw::Banner::new(
            "hyprrules.lua wird noch nicht geladen – füge require(\"hyprrules\") in deine hyprland.lua ein",
        );
        banner.set_use_markup(false);

        let page = adw::PreferencesPage::new();

        // Apps -> workspaces
        let window_rules = boxed_list("Noch keine App-Regeln");
        page.add(&group("Apps → Workspaces", "Welche App auf welchem Workspace startet", &[window_rules.upcast_ref()]));

        let client_combo = adw::ComboRow::builder().title("Laufende App übernehmen").build();
        let s = sender.clone();
        client_combo.connect_selected_notify(move |row| s.input(Msg::ClientPicked(row.selected())));
        let class_entry = adw::EntryRow::builder().title("Fensterklasse").build();
        let app_workspace = workspace_spin();
        let silent = adw::SwitchRow::builder()
            .title("Im Hintergrund öffnen")
            .subtitle("Nicht automatisch zum Workspace wechseln")
            .active(true)
            .build();
        let new_app = group(
            "Neue App-Regel",
            "Weitere Eigenschaften (schwebend, Größe, Transparenz …) nach dem Hinzufügen per Aufklappen",
            &[client_combo.upcast_ref(), class_entry.upcast_ref(), app_workspace.upcast_ref(), silent.upcast_ref()],
        );
        new_app.set_header_suffix(Some(&add_button(sender, Msg::AddWindowRule)));
        page.add(&new_app);

        // Workspaces -> monitors
        let workspace_rules = boxed_list("Noch keine Workspace-Zuordnungen");
        page.add(&group(
            "Workspaces → Monitore",
            "Auf welchem Bildschirm ein Workspace liegt",
            &[workspace_rules.upcast_ref()],
        ));

        let ws_workspace = workspace_spin();
        let monitor_combo = adw::ComboRow::builder().title("Monitor").build();
        let new_ws = group("Neue Workspace-Zuordnung", "", &[ws_workspace.upcast_ref(), monitor_combo.upcast_ref()]);
        new_ws.set_header_suffix(Some(&add_button(sender, Msg::AddWorkspaceRule)));
        page.add(&new_ws);

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.add_top_bar(&banner);
        toolbar.set_content(Some(&page));

        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&toolbar));
        window.set_content(Some(&toasts));

        Self {
            toasts,
            banner,
            save,
            window_rules,
            workspace_rules,
            client_combo,
            class_entry,
            app_workspace,
            silent,
            ws_workspace,
            monitor_combo,
        }
    }

    pub fn set_pickers(&self, clients: &[Client], monitors: &[Monitor]) {
        let mut apps = vec!["— manuell eingeben —".to_owned()];
        apps.extend(clients.iter().map(|c| ellipsize(&format!("{} — {}", c.class, c.title), 48)));
        self.client_combo.set_model(Some(&string_list(&apps)));

        let screens: Vec<String> = monitors.iter().map(monitor_label).collect();
        self.monitor_combo.set_model(Some(&string_list(&screens)));
    }

    pub fn set_rules(&self, rules: &RuleSet, monitors: &[Monitor], sender: &ComponentSender<App>) {
        self.window_rules.remove_all();
        for (i, rule) in rules.window_rules.iter().enumerate() {
            let row = rule_row::window_rule_row(i, rule, rules, sender);
            make_reorderable(&row, i, sender);
            self.window_rules.append(&row);
        }

        self.workspace_rules.remove_all();
        for (i, rule) in rules.workspace_rules.iter().enumerate() {
            let monitor = monitors
                .iter()
                .find(|m| m.name == rule.monitor)
                .map(monitor_label)
                .unwrap_or_else(|| format!("{} (nicht verbunden)", rule.monitor));
            let row = action_row(&format!("Workspace {}", rule.workspace), &monitor);
            row.add_suffix(&delete_button(sender, Msg::RemoveWorkspaceRule(i)));
            self.workspace_rules.append(&row);
        }
    }

    /// Refreshes the collapsed summary of one rule without rebuilding the list.
    pub fn update_summary(&self, index: usize, rules: &RuleSet) {
        let row = self
            .window_rules
            .row_at_index(index as i32)
            .and_then(|row| row.downcast::<adw::ExpanderRow>().ok());
        if let (Some(row), Some(rule)) = (row, rules.window_rules.get(index)) {
            row.set_subtitle(&rule_row::summary(rule, rules));
        }
    }
}

/// Adds a drag handle to `row` and lets other rows be dropped onto it.
/// The dragged payload is the rule index.
fn make_reorderable(row: &adw::ExpanderRow, index: usize, sender: &ComponentSender<App>) {
    let handle = gtk::Image::builder()
        .icon_name("list-drag-handle-symbolic")
        .tooltip_text("Ziehen zum Umsortieren – spätere Regeln haben Vorrang")
        .css_classes(["dim-label"])
        .build();
    row.add_prefix(&handle);

    let source = gtk::DragSource::builder()
        .actions(gdk::DragAction::MOVE)
        .content(&gdk::ContentProvider::for_value(&(index as u32).to_value()))
        .build();
    let dragged = row.clone();
    source.connect_drag_begin(move |source, _| {
        let icon = gtk::WidgetPaintable::new(Some(&dragged));
        source.set_icon(Some(&icon), 0, 0);
    });
    handle.add_controller(source);

    let target = gtk::DropTarget::new(u32::static_type(), gdk::DragAction::MOVE);
    let sender = sender.clone();
    target.connect_drop(move |_, value, _, _| match value.get::<u32>() {
        Ok(from) => {
            sender.input(Msg::MoveWindowRule(from as usize, index));
            true
        }
        Err(_) => false,
    });
    row.add_controller(target);
}

fn monitor_label(m: &Monitor) -> String {
    let mut label = format!("{} ({}×{})", m.name, m.width, m.height);
    if !m.description.is_empty() {
        label.push_str(&format!(" — {}", m.description));
    }
    ellipsize(&label, 48)
}

fn group(title: &str, description: &str, children: &[&gtk::Widget]) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title(title).build();
    if !description.is_empty() {
        group.set_description(Some(description));
    }
    for child in children {
        group.add(*child);
    }
    group
}

fn boxed_list(placeholder: &str) -> gtk::ListBox {
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    let label = gtk::Label::builder()
        .label(placeholder)
        .css_classes(["dim-label"])
        .margin_top(18)
        .margin_bottom(18)
        .build();
    list.set_placeholder(Some(&label));
    list
}

fn action_row(title: &str, subtitle: &str) -> adw::ActionRow {
    adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .use_markup(false)
        .build()
}

fn workspace_spin() -> adw::SpinRow {
    let spin = adw::SpinRow::with_range(1.0, MAX_WORKSPACE, 1.0);
    spin.set_title("Workspace");
    spin
}

fn add_button(sender: &ComponentSender<App>, msg: Msg) -> gtk::Button {
    let button = gtk::Button::builder()
        .label("Hinzufügen")
        .css_classes(["flat"])
        .build();
    connect(&button, sender, msg);
    button
}

pub fn delete_button(sender: &ComponentSender<App>, msg: Msg) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name("user-trash-symbolic")
        .tooltip_text("Entfernen")
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    connect(&button, sender, msg);
    button
}

fn connect(button: &gtk::Button, sender: &ComponentSender<App>, msg: Msg) {
    let sender = sender.clone();
    button.connect_clicked(move |_| sender.input(msg.clone()));
}

fn string_list(items: &[String]) -> gtk::StringList {
    let items: Vec<&str> = items.iter().map(String::as_str).collect();
    gtk::StringList::new(&items)
}

fn ellipsize(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let cut: String = text.chars().take(max - 1).collect();
    format!("{cut}…")
}
