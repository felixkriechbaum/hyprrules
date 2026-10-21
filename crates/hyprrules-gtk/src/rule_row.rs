//! Expandable row for one window rule: a summary in the header, an editor
//! for every effect inside. Edits are sent as [`Msg::EditWindowRule`] and do
//! not rebuild the list, so the row stays open while editing.

use std::rc::Rc;

use hyprrules_core::{RuleEdit, RuleSet, WindowRule, workspace};
use relm4::adw::prelude::*;
use relm4::{ComponentSender, adw, gtk};

use crate::ui::delete_button;
use crate::workspace_picker::WorkspacePicker;
use crate::{App, Msg};

const MAX_SIZE: f64 = 7680.0;

pub fn window_rule_row(
    index: usize,
    rule: &WindowRule,
    rules: &RuleSet,
    workspaces: &[String],
    sender: &ComponentSender<App>,
) -> adw::ExpanderRow {
    let row = adw::ExpanderRow::builder()
        .title(&rule.name)
        .subtitle(summary(rule, rules))
        .build();
    row.set_use_markup(false);

    let enabled = gtk::Switch::builder()
        .active(rule.enabled)
        .valign(gtk::Align::Center)
        .tooltip_text("Regel aktiv")
        .build();
    let s = sender.clone();
    enabled.connect_active_notify(move |sw| s.input(Msg::ToggleWindowRule(index, sw.is_active())));
    row.add_suffix(&enabled);
    row.add_suffix(&delete_button(sender, Msg::RemoveWindowRule(index)));

    let emit: Rc<dyn Fn(RuleEdit)> = {
        let sender = sender.clone();
        Rc::new(move |edit| sender.input(Msg::EditWindowRule(index, edit)))
    };

    // Workspace
    let ws_on = switch_row("Workspace zuweisen", "", rule.workspace.is_some());
    let ws = WorkspacePicker::new("Workspace");
    ws.set_choices(workspaces);
    if let Some(selector) = &rule.workspace {
        ws.select(selector);
    }
    let silent = switch_row("Im Hintergrund öffnen", "Nicht automatisch zum Workspace wechseln", rule.silent);
    let [ws_combo, ws_entry] = ws.rows();
    enables(&ws_on, &[ws_combo, ws_entry, silent.upcast_ref()]);
    let send_ws = {
        let (ws_on, ws, emit) = (ws_on.clone(), ws.clone(), emit.clone());
        move || emit(RuleEdit::Workspace(ws_on.is_active().then(|| ws.selected()).flatten()))
    };
    let send_toggle = send_ws.clone();
    ws_on.connect_active_notify(move |_| send_toggle());
    ws.connect_changed(send_ws);
    let e = emit.clone();
    silent.connect_active_notify(move |sw| e(RuleEdit::Silent(sw.is_active())));

    // Float
    let float = switch_row("Schwebend", "Fenster nicht kacheln", rule.float);
    let e = emit.clone();
    float.connect_active_notify(move |sw| e(RuleEdit::Float(sw.is_active())));

    // Size
    let [w, h] = rule.size.unwrap_or([1280, 720]);
    let size_on = switch_row("Feste Größe", "Startgröße in Pixeln, macht das Fenster schwebend", rule.size.is_some());
    let width = spin_row("Breite", 100.0, MAX_SIZE, 10.0, 0, w as f64);
    let height = spin_row("Höhe", 100.0, MAX_SIZE, 10.0, 0, h as f64);
    enables(&size_on, &[width.upcast_ref(), height.upcast_ref()]);
    let send_size = {
        let (size_on, width, height, emit) = (size_on.clone(), width.clone(), height.clone(), emit.clone());
        move || {
            let size = [width.value() as u32, height.value() as u32];
            emit(RuleEdit::Size(size_on.is_active().then_some(size)))
        }
    };
    on_change(&size_on, &width, send_size.clone());
    height.connect_value_notify(move |_| send_size());

    // Opacity
    let opacity_on = switch_row("Transparenz", "", rule.opacity.is_some());
    let opacity = spin_row("Deckkraft", 0.1, 1.0, 0.05, 2, rule.opacity.unwrap_or(0.9).into());
    enables(&opacity_on, &[opacity.upcast_ref()]);
    let send_opacity = {
        let (opacity_on, opacity, emit) = (opacity_on.clone(), opacity.clone(), emit.clone());
        move || emit(RuleEdit::Opacity(opacity_on.is_active().then(|| opacity.value() as f32)))
    };
    on_change(&opacity_on, &opacity, send_opacity);

    // Pin
    let pin = switch_row("Anheften", "Auf allen Workspaces sichtbar, macht das Fenster schwebend", rule.pin);
    let e = emit;
    pin.connect_active_notify(move |sw| e(RuleEdit::Pin(sw.is_active())));

    let class = class_row(rule);
    let editors: [&gtk::Widget; 12] = [
        ws_on.upcast_ref(),
        ws_combo,
        ws_entry,
        silent.upcast_ref(),
        float.upcast_ref(),
        size_on.upcast_ref(),
        width.upcast_ref(),
        height.upcast_ref(),
        opacity_on.upcast_ref(),
        opacity.upcast_ref(),
        pin.upcast_ref(),
        class.upcast_ref(),
    ];
    for editor in editors {
        row.add_row(editor);
    }
    row
}

/// One-line description of all effects, shown in the collapsed row.
pub fn summary(rule: &WindowRule, rules: &RuleSet) -> String {
    let mut parts = Vec::new();
    if let Some(ws) = &rule.workspace {
        let mut part = format!("Workspace {}", workspace::label(ws));
        if let Some(bound) = rules.workspace_rules.iter().find(|w| &w.workspace == ws) {
            part.push_str(&format!(" auf {}", bound.monitor));
        }
        if rule.silent {
            part.push_str(" (Hintergrund)");
        }
        parts.push(part);
    }
    if rule.is_floating() {
        parts.push("schwebend".into());
    }
    if let Some([w, h]) = rule.size {
        parts.push(format!("{w}×{h}"));
    }
    if let Some(opacity) = rule.opacity {
        parts.push(format!("{:.0} % Deckkraft", opacity * 100.0));
    }
    if rule.pin {
        parts.push("angeheftet".into());
    }
    if parts.is_empty() {
        parts.push("keine Wirkung".into());
    }
    parts.join(" · ")
}

fn class_row(rule: &WindowRule) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title("Fensterklasse (Regex)")
        .subtitle(&rule.class)
        .subtitle_selectable(true)
        .build();
    row.set_use_markup(false);
    row.add_css_class("property");
    row
}

fn switch_row(title: &str, subtitle: &str, active: bool) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder().title(title).active(active).build();
    if !subtitle.is_empty() {
        row.set_subtitle(subtitle);
    }
    row
}

fn spin_row(title: &str, min: f64, max: f64, step: f64, digits: u32, value: f64) -> adw::SpinRow {
    let row = adw::SpinRow::with_range(min, max, step);
    row.set_title(title);
    row.set_digits(digits);
    row.set_value(value);
    row
}

/// `targets` are only editable while `toggle` is on.
fn enables(toggle: &adw::SwitchRow, targets: &[&gtk::Widget]) {
    for target in targets {
        toggle
            .bind_property("active", *target, "sensitive")
            .sync_create()
            .build();
    }
}

/// Calls `send` whenever the toggle flips or the value changes.
fn on_change(toggle: &adw::SwitchRow, value: &adw::SpinRow, send: impl Fn() + Clone + 'static) {
    let send_toggle = send.clone();
    toggle.connect_active_notify(move |_| send_toggle());
    value.connect_value_notify(move |_| send());
}
