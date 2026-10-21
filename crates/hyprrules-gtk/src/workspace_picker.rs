//! Dropdown of known workspaces plus "Andere …" with a free-text entry for
//! workspaces Hyprland doesn't report (yet).

use std::cell::RefCell;
use std::rc::Rc;

use hyprrules_core::workspace;
use relm4::adw::prelude::*;
use relm4::{adw, gtk};

const OTHER: &str = "Andere …";

#[derive(Clone)]
pub struct WorkspacePicker {
    pub combo: adw::ComboRow,
    pub entry: adw::EntryRow,
    /// Selectors matching the combo items; the last item is "other".
    choices: Rc<RefCell<Vec<String>>>,
}

impl WorkspacePicker {
    pub fn new(title: &str) -> Self {
        let combo = adw::ComboRow::builder().title(title).build();
        let entry = adw::EntryRow::builder()
            .title("Eigener Workspace (Zahl oder Name)")
            .visible(false)
            .build();
        let picker = Self {
            combo,
            entry,
            choices: Rc::default(),
        };
        let p = picker.clone();
        picker
            .combo
            .connect_selected_notify(move |_| p.entry.set_visible(p.is_other()));
        picker
    }

    /// Replaces the choices, keeping the current selection if possible.
    pub fn set_choices(&self, choices: &[String]) {
        let current = self.selected();
        *self.choices.borrow_mut() = choices.to_vec();
        let mut labels: Vec<&str> = choices.iter().map(|c| workspace::label(c)).collect();
        labels.push(OTHER);
        self.combo.set_model(Some(&gtk::StringList::new(&labels)));
        if let Some(current) = current {
            self.select(&current);
        }
    }

    /// Selects `selector`, falling back to "other" with the text filled in.
    pub fn select(&self, selector: &str) {
        let index = self.choices.borrow().iter().position(|c| c == selector);
        match index {
            Some(i) => self.combo.set_selected(i as u32),
            None => {
                self.entry.set_text(workspace::label(selector));
                self.combo.set_selected(self.choices.borrow().len() as u32);
            }
        }
        self.entry.set_visible(self.is_other());
    }

    pub fn selected(&self) -> Option<String> {
        let index = self.combo.selected() as usize;
        match self.choices.borrow().get(index) {
            Some(choice) => Some(choice.clone()),
            None => workspace::from_input(&self.entry.text()),
        }
    }

    /// Calls `f` whenever the chosen workspace may have changed.
    pub fn connect_changed(&self, f: impl Fn() + Clone + 'static) {
        let on_combo = f.clone();
        self.combo.connect_selected_notify(move |_| on_combo());
        self.entry.connect_changed(move |_| f());
    }

    pub fn rows(&self) -> [&gtk::Widget; 2] {
        [self.combo.upcast_ref(), self.entry.upcast_ref()]
    }

    fn is_other(&self) -> bool {
        self.combo.selected() as usize >= self.choices.borrow().len()
    }
}
