//! Keybindings tab: every bind Hyprland knows, grouped by submap and
//! searchable. Read only.

use std::cell::RefCell;
use std::rc::Rc;

use hyprrules_core::Bind;
use relm4::adw::prelude::*;
use relm4::{adw, gtk};

use crate::ui::{action_row, boxed_list};

/// One submap: its group and the rows with the bind each one shows.
struct Section {
    group: adw::PreferencesGroup,
    rows: Vec<(adw::ActionRow, Bind)>,
}

pub struct BindsPage {
    pub root: gtk::Box,
    page: adw::PreferencesPage,
    search: gtk::SearchEntry,
    sections: Rc<RefCell<Vec<Section>>>,
}

impl BindsPage {
    pub fn build() -> Self {
        let search = gtk::SearchEntry::builder()
            .placeholder_text("Search keys or actions, e.g. super shift")
            .build();
        let clamp = adw::Clamp::builder()
            .maximum_size(600)
            .margin_top(12)
            .margin_start(12)
            .margin_end(12)
            .child(&search)
            .build();
        let page = adw::PreferencesPage::builder().vexpand(true).build();

        let root = gtk::Box::builder().orientation(gtk::Orientation::Vertical).build();
        root.append(&clamp);
        root.append(&page);

        let sections: Rc<RefCell<Vec<Section>>> = Rc::default();
        let s = sections.clone();
        search.connect_search_changed(move |entry| filter(&s.borrow(), &entry.text()));

        Self {
            root,
            page,
            search,
            sections,
        }
    }

    pub fn set(&self, binds: &[Bind]) {
        let mut sections = self.sections.borrow_mut();
        for section in sections.drain(..) {
            self.page.remove(&section.group);
        }

        // Global binds first, then submaps in the order they appear.
        let mut submaps: Vec<&str> = vec![""];
        for bind in binds {
            if !submaps.contains(&bind.submap.as_str()) {
                submaps.push(&bind.submap);
            }
        }
        for submap in submaps {
            let binds: Vec<&Bind> = binds.iter().filter(|b| b.submap == submap).collect();
            if binds.is_empty() {
                continue;
            }
            let title = if submap.is_empty() {
                "Global".to_owned()
            } else {
                format!("Submap “{submap}”")
            };
            let group = adw::PreferencesGroup::builder()
                .title(title)
                .description(count(binds.len()))
                .build();
            let list = boxed_list("");
            let rows = binds
                .into_iter()
                .map(|bind| {
                    let row = bind_row(bind);
                    list.append(&row);
                    (row, bind.clone())
                })
                .collect();
            group.add(&list);
            self.page.add(&group);
            sections.push(Section { group, rows });
        }
        filter(&sections, &self.search.text());
    }
}

fn bind_row(bind: &Bind) -> adw::ActionRow {
    // With a description, the raw dispatcher goes into the subtitle.
    let mut details = Vec::new();
    if !bind.description.is_empty() {
        details.push(format!("{} {}", bind.dispatcher, bind.arg).trim().to_owned());
    }
    details.extend(bind.flags().iter().map(|f| (*f).to_owned()));
    let row = action_row(&bind.action(), &details.join(" · "));

    let keys = gtk::Box::builder()
        .spacing(4)
        .width_request(220)
        .valign(gtk::Align::Center)
        .build();
    for key in bind.keys() {
        keys.append(&gtk::Label::builder().label(&key).css_classes(["key"]).build());
    }
    row.add_prefix(&keys);
    row
}

fn filter(sections: &[Section], query: &str) {
    for section in sections {
        let mut visible = 0;
        for (row, bind) in &section.rows {
            let matches = bind.matches(query);
            row.set_visible(matches);
            visible += usize::from(matches);
        }
        section.group.set_visible(visible > 0);
    }
}

fn count(n: usize) -> String {
    if n == 1 { "1 binding".into() } else { format!("{n} bindings") }
}
