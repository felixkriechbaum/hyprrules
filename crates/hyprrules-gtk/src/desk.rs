//! The desk: monitors drawn to scale in their real arrangement, workspaces
//! as tiles on their screens and apps as stickers. Everything is placed by
//! drag and drop; the rule list stays available as a second view.

use hyprrules_core::{Client, Monitor, RuleSet, workspace};
use relm4::adw::prelude::*;
use relm4::gtk::gdk;
use relm4::{ComponentSender, gtk};

use crate::{App, Msg};

/// Space the monitors are scaled into.
const DESK_WIDTH: f64 = 760.0;
const DESK_HEIGHT: f64 = 300.0;
/// Bezel padding plus the gap between neighbouring monitors.
const BEZEL: i32 = 16;
const STICKER_COLORS: u32 = 6;

/// What is being dragged around the desk.
#[derive(Debug, Clone, PartialEq)]
enum Payload {
    /// A running app without a rule, by window class.
    App(String),
    Rule(usize),
    Workspace(String),
}

impl Payload {
    fn encode(&self) -> String {
        match self {
            Payload::App(class) => format!("app\n{class}"),
            Payload::Rule(index) => format!("rule\n{index}"),
            Payload::Workspace(selector) => format!("ws\n{selector}"),
        }
    }

    fn decode(data: &str) -> Option<Self> {
        let (kind, value) = data.split_once('\n')?;
        match kind {
            "app" => Some(Payload::App(value.to_owned())),
            "rule" => value.parse().ok().map(Payload::Rule),
            "ws" => Some(Payload::Workspace(value.to_owned())),
            _ => None,
        }
    }
}

pub struct Desk {
    pub root: gtk::ScrolledWindow,
    canvas: gtk::Fixed,
    shelf: gtk::FlowBox,
    tray: gtk::FlowBox,
}

/// Everything the desk shows, borrowed from the app model.
pub struct DeskState<'a> {
    pub rules: &'a RuleSet,
    pub monitors: &'a [Monitor],
    pub workspaces: &'a [String],
    pub clients: &'a [Client],
    pub selected: Option<usize>,
}

impl Desk {
    pub fn build(sender: &ComponentSender<App>) -> Self {
        let canvas = gtk::Fixed::builder().halign(gtk::Align::Center).build();

        let shelf = flow_box();
        shelf.set_hexpand(true);
        let shelf_box = gtk::Box::builder().spacing(12).css_classes(["shelf"]).build();
        shelf_box.append(&caption("Ohne festen Monitor"));
        shelf_box.append(&shelf);
        let s = sender.clone();
        drop_target(&shelf_box, move |payload| match payload {
            Payload::Workspace(ws) => {
                s.input(Msg::BindWorkspace(ws, None));
                true
            }
            _ => false,
        });

        let desk = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(24)
            .css_classes(["desk"])
            .build();
        desk.append(&canvas);
        desk.append(&shelf_box);

        let tray = flow_box();
        let tray_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .css_classes(["tray"])
            .build();
        let tray_header = gtk::Box::builder().spacing(12).build();
        let tray_titles = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .hexpand(true)
            .build();
        let title = gtk::Label::builder().label("Nicht platziert").xalign(0.0).css_classes(["heading"]).build();
        let hint = gtk::Label::builder()
            .label("Zieh eine App auf einen Workspace. Zurück hierher ziehen entfernt die Zuordnung.")
            .xalign(0.0)
            .wrap(true)
            .css_classes(["dim-label", "caption"])
            .build();
        tray_titles.append(&title);
        tray_titles.append(&hint);
        tray_header.append(&tray_titles);
        tray_header.append(&add_app_button(sender));
        tray_box.append(&tray_header);
        tray_box.append(&tray);
        let s = sender.clone();
        drop_target(&tray_box, move |payload| match payload {
            Payload::Rule(index) => {
                s.input(Msg::UnplaceRule(index));
                true
            }
            _ => false,
        });

        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        content.append(&desk);
        content.append(&tray_box);

        let root = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&content)
            .build();

        Self {
            root,
            canvas,
            shelf,
            tray,
        }
    }

    pub fn rebuild(&self, state: &DeskState, sender: &ComponentSender<App>) {
        while let Some(child) = self.canvas.first_child() {
            self.canvas.remove(&child);
        }
        self.shelf.remove_all();
        self.tray.remove_all();

        let monitor_of = |ws: &str| {
            state
                .rules
                .workspace_rules
                .iter()
                .find(|r| r.workspace == ws)
                .map(|r| r.monitor.as_str())
                .filter(|m| state.monitors.iter().any(|known| known.name == *m))
        };

        // Monitors, scaled into the desk area in their real arrangement.
        let logical = |m: &Monitor| {
            let scale = if m.scale > 0.0 { m.scale } else { 1.0 };
            (m.width as f64 / scale, m.height as f64 / scale)
        };
        let min_x = state.monitors.iter().map(|m| m.x).min().unwrap_or(0) as f64;
        let min_y = state.monitors.iter().map(|m| m.y).min().unwrap_or(0) as f64;
        let max_x = state.monitors.iter().map(|m| m.x as f64 + logical(m).0).fold(0.0, f64::max);
        let max_y = state.monitors.iter().map(|m| m.y as f64 + logical(m).1).fold(0.0, f64::max);
        let scale = (DESK_WIDTH / (max_x - min_x).max(1.0)).min(DESK_HEIGHT / (max_y - min_y).max(1.0));

        for monitor in state.monitors {
            let (w, h) = logical(monitor);
            let tiles: Vec<gtk::Widget> = state
                .workspaces
                .iter()
                .filter(|ws| monitor_of(ws) == Some(monitor.name.as_str()))
                .map(|ws| workspace_tile(ws, Some(&monitor.name), state, sender))
                .collect();
            let widget = monitor_widget(monitor, (w * scale) as i32, (h * scale) as i32, &tiles, sender);
            self.canvas.put(
                &widget,
                (monitor.x as f64 - min_x) * scale,
                (monitor.y as f64 - min_y) * scale,
            );
        }

        for ws in state.workspaces.iter().filter(|ws| monitor_of(ws).is_none()) {
            self.shelf.append(&workspace_tile(ws, None, state, sender));
        }

        // Rules without a workspace, then running apps without any rule.
        for (index, rule) in state.rules.window_rules.iter().enumerate() {
            if rule.workspace.is_none() {
                self.tray.append(&rule_sticker(index, state, sender));
            }
        }
        for client in state.clients {
            if state.rules.rule_for_class(&client.class).is_none() {
                let sticker = sticker(&client.class, Payload::App(client.class.clone()));
                sticker.set_tooltip_text(Some(&client.title));
                self.tray.append(&sticker);
            }
        }
    }
}

fn monitor_widget(
    monitor: &Monitor,
    width: i32,
    height: i32,
    tiles: &[gtk::Widget],
    sender: &ComponentSender<App>,
) -> gtk::Box {
    let screen = flow_box();
    screen.add_css_class("screen");
    screen.set_size_request((width - BEZEL).max(120), (height - BEZEL).max(80));
    screen.set_valign(gtk::Align::Fill);
    for tile in tiles {
        screen.append(tile);
    }

    let bezel = gtk::Box::builder().css_classes(["bezel"]).build();
    bezel.append(&screen);
    // The focused monitor gets Hyprland's active border gradient.
    let frame = gtk::Box::builder().css_classes(["frame"]).build();
    if monitor.focused {
        frame.add_css_class("focused");
    }
    frame.append(&bezel);

    let stand = gtk::Box::builder()
        .css_classes(["stand"])
        .halign(gtk::Align::Center)
        .build();
    let name = gtk::Label::builder()
        .label(&monitor.name)
        .css_classes(["monitor-name"])
        .tooltip_text(format!("{} · {}×{}", monitor.description, monitor.width, monitor.height))
        .build();

    let widget = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .build();
    widget.append(&frame);
    widget.append(&stand);
    widget.append(&name);

    let s = sender.clone();
    let monitor_name = monitor.name.clone();
    drop_target(&frame, move |payload| match payload {
        Payload::Workspace(ws) => {
            s.input(Msg::BindWorkspace(ws, Some(monitor_name.clone())));
            true
        }
        _ => false,
    });
    widget
}

fn workspace_tile(
    selector: &str,
    monitor: Option<&str>,
    state: &DeskState,
    sender: &ComponentSender<App>,
) -> gtk::Widget {
    let label = gtk::Label::builder()
        .label(workspace::label(selector))
        .xalign(0.0)
        .css_classes(["ws-label"])
        .build();
    let apps = flow_box();
    let mut count = 0;
    for (index, rule) in state.rules.window_rules.iter().enumerate() {
        if rule.workspace.as_deref() == Some(selector) {
            apps.append(&rule_sticker(index, state, sender));
            count += 1;
        }
    }

    let tile = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .css_classes(["ws"])
        .tooltip_text("Auf einen anderen Monitor ziehen, um den Workspace dort festzulegen")
        .build();
    if count > 0 {
        tile.add_css_class("filled");
    }
    tile.append(&label);
    tile.append(&apps);
    drag_source(&tile, Payload::Workspace(selector.to_owned()));

    let s = sender.clone();
    let target_ws = selector.to_owned();
    let target_monitor = monitor.map(str::to_owned);
    drop_target(&tile, move |payload| {
        let msg = match payload {
            Payload::App(class) => Msg::PlaceApp(class, target_ws.clone()),
            Payload::Rule(index) => Msg::PlaceRule(index, target_ws.clone()),
            // Dropping a workspace on a tile moves it next to that tile.
            Payload::Workspace(ws) if ws != target_ws => Msg::BindWorkspace(ws, target_monitor.clone()),
            Payload::Workspace(_) => return false,
        };
        s.input(msg);
        true
    });
    tile.upcast()
}

fn rule_sticker(index: usize, state: &DeskState, sender: &ComponentSender<App>) -> gtk::Widget {
    let rule = &state.rules.window_rules[index];
    let sticker = sticker(&rule.name, Payload::Rule(index));
    if !rule.enabled {
        sticker.add_css_class("off");
    }
    if rule.is_floating() {
        sticker.add_css_class("floating");
    }
    if state.selected == Some(index) {
        sticker.add_css_class("selected");
    }
    sticker.set_tooltip_text(Some(&crate::rule_row::summary(rule, state.rules)));

    let click = gtk::GestureClick::new();
    let s = sender.clone();
    click.connect_released(move |_, _, _, _| s.input(Msg::SelectRule(Some(index))));
    sticker.add_controller(click);
    sticker.upcast()
}

fn sticker(text: &str, payload: Payload) -> gtk::Label {
    let color = text.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)) % STICKER_COLORS;
    let sticker = gtk::Label::builder()
        .label(text)
        .css_classes(["sticker", &format!("c{color}")])
        .build();
    sticker.set_cursor_from_name(Some("grab"));
    drag_source(&sticker, payload);
    sticker
}

fn add_app_button(sender: &ComponentSender<App>) -> gtk::MenuButton {
    let entry = gtk::Entry::builder()
        .placeholder_text("Fensterklasse, z. B. steam")
        .build();
    let add = gtk::Button::builder()
        .label("Hinzufügen")
        .css_classes(["suggested-action"])
        .build();
    let form = gtk::Box::builder()
        .spacing(6)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(6)
        .margin_end(6)
        .build();
    form.append(&entry);
    form.append(&add);
    let popover = gtk::Popover::builder().child(&form).build();

    let submit = {
        let (entry, popover, sender) = (entry.clone(), popover.clone(), sender.clone());
        move || {
            let class = entry.text().trim().to_owned();
            if !class.is_empty() {
                sender.input(Msg::AddApp(class));
                entry.set_text("");
                popover.popdown();
            }
        }
    };
    let on_click = submit.clone();
    add.connect_clicked(move |_| on_click());
    entry.connect_activate(move |_| submit());

    gtk::MenuButton::builder()
        .label("Andere App")
        .valign(gtk::Align::Center)
        .popover(&popover)
        .build()
}

fn caption(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .css_classes(["desk-caption"])
        .valign(gtk::Align::Center)
        .build()
}

fn flow_box() -> gtk::FlowBox {
    gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .row_spacing(6)
        .column_spacing(6)
        .homogeneous(false)
        .valign(gtk::Align::Start)
        .build()
}

fn drag_source(widget: &impl IsA<gtk::Widget>, payload: Payload) {
    let source = gtk::DragSource::builder()
        .actions(gdk::DragAction::MOVE)
        .content(&gdk::ContentProvider::for_value(&payload.encode().to_value()))
        .build();
    let dragged = widget.clone().upcast::<gtk::Widget>();
    source.connect_drag_begin(move |source, _| {
        source.set_icon(Some(&gtk::WidgetPaintable::new(Some(&dragged))), 0, 0);
    });
    // Bubble phase: a sticker claims the drag before the tile around it.
    widget.add_controller(source);
}

/// `on_drop` returns whether it handled the payload.
fn drop_target(widget: &impl IsA<gtk::Widget>, on_drop: impl Fn(Payload) -> bool + 'static) {
    let target = gtk::DropTarget::new(String::static_type(), gdk::DragAction::MOVE);
    target.connect_drop(move |_, value, _, _| {
        value
            .get::<String>()
            .ok()
            .and_then(|data| Payload::decode(&data))
            .is_some_and(&on_drop)
    });
    widget.add_controller(target);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_roundtrip() {
        for payload in [
            Payload::App("org.gnome.Nautilus".into()),
            Payload::Rule(3),
            Payload::Workspace("name:G".into()),
        ] {
            assert_eq!(Payload::decode(&payload.encode()), Some(payload));
        }
        assert_eq!(Payload::decode("garbage"), None);
    }
}
