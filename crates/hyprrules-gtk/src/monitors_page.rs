//! Monitors tab: the monitors as Hyprland reports them right now, plus the
//! user's `monitors.lua` for reference. Read only.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use hyprrules_core::{Monitor, workspace};
use relm4::adw::prelude::*;
use relm4::gtk::gio;
use relm4::{adw, gtk};

use crate::ui::{action_row, boxed_list};

pub struct MonitorsPage {
    pub root: adw::PreferencesPage,
    list: gtk::ListBox,
    file_group: adw::PreferencesGroup,
    source: gtk::TextView,
    open: gtk::Button,
    path: Rc<RefCell<Option<PathBuf>>>,
}

impl MonitorsPage {
    pub fn build() -> Self {
        let list = boxed_list("No monitors reported");
        let connected = adw::PreferencesGroup::builder()
            .title("Connected")
            .description("As Hyprland reports them right now")
            .build();
        connected.add(&list);

        let source = gtk::TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .monospace(true)
            .wrap_mode(gtk::WrapMode::WordChar)
            .top_margin(12)
            .bottom_margin(12)
            .left_margin(12)
            .right_margin(12)
            .css_classes(["card", "source"])
            .build();
        let path: Rc<RefCell<Option<PathBuf>>> = Rc::default();
        let open = gtk::Button::builder()
            .label("Open")
            .tooltip_text("Open in your default editor")
            .css_classes(["flat"])
            .build();
        let p = path.clone();
        open.connect_clicked(move |button| {
            if let Some(path) = p.borrow().as_ref() {
                let window = button.root().and_downcast::<gtk::Window>();
                gtk::FileLauncher::new(Some(&gio::File::for_path(path))).launch(
                    window.as_ref(),
                    gio::Cancellable::NONE,
                    |_| {},
                );
            }
        });
        let file_group = adw::PreferencesGroup::builder().title("monitors.lua").build();
        file_group.set_header_suffix(Some(&open));
        file_group.add(&source);

        let root = adw::PreferencesPage::new();
        root.add(&connected);
        root.add(&file_group);

        Self {
            root,
            list,
            file_group,
            source,
            open,
            path,
        }
    }

    /// `file` is the found `monitors.lua` with its contents.
    pub fn set(&self, monitors: &[Monitor], file: Option<(PathBuf, String)>, hypr_dir: &Path) {
        self.list.remove_all();
        for monitor in monitors {
            let mut subtitle = String::new();
            if !monitor.description.is_empty() {
                subtitle = format!("{}\n", monitor.description);
            }
            subtitle.push_str(&details(monitor));
            let row = action_row(&monitor.name, &subtitle);
            if let Some(ws) = monitor.shown_workspace() {
                let shown = gtk::Label::builder()
                    .label(format!("Workspace {}", workspace::label(&ws)))
                    .css_classes(["dim-label"])
                    .build();
                row.add_suffix(&shown);
            }
            if monitor.focused {
                row.add_suffix(&gtk::Label::builder().label("Focused").css_classes(["accent"]).build());
            }
            self.list.append(&row);
        }

        match file {
            Some((path, text)) => {
                self.file_group.set_description(Some(&path.display().to_string()));
                self.source.buffer().set_text(text.trim_end());
                self.source.set_visible(true);
                self.open.set_visible(true);
                *self.path.borrow_mut() = Some(path);
            }
            None => {
                let hint = format!("No monitors.lua found in {}", hypr_dir.display());
                self.file_group.set_description(Some(&hint));
                self.source.set_visible(false);
                self.open.set_visible(false);
                *self.path.borrow_mut() = None;
            }
        }
    }
}

/// `2560×1440 @ 144 Hz · scale 1.25 · at 0, 0 · rotated 90°`
fn details(m: &Monitor) -> String {
    let mut parts = vec![format!("{}×{}", m.width, m.height)];
    if m.refresh_rate > 0.0 {
        parts[0].push_str(&format!(" @ {:.0} Hz", m.refresh_rate));
    }
    parts.push(format!("scale {}", (m.scale * 100.0).round() / 100.0));
    parts.push(format!("at {}, {}", m.x, m.y));
    let transform = match m.transform {
        1 => "rotated 90°",
        2 => "rotated 180°",
        3 => "rotated 270°",
        4 => "flipped",
        5 => "flipped, rotated 90°",
        6 => "flipped, rotated 180°",
        7 => "flipped, rotated 270°",
        _ => "",
    };
    if !transform.is_empty() {
        parts.push(transform.to_owned());
    }
    parts.join(" · ")
}
