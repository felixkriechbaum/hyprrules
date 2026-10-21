use serde::{Deserialize, Serialize};

/// Everything hyprrules manages. Persisted as TOML, rendered to Lua.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RuleSet {
    #[serde(default)]
    pub window_rules: Vec<WindowRule>,
    #[serde(default)]
    pub workspace_rules: Vec<WorkspaceRule>,
}

/// "Windows matching `class` get these effects." Every effect is optional.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowRule {
    /// Unique, human readable name; also used as the Hyprland rule name.
    pub name: String,
    /// Regex matched against the window class.
    pub class: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// Open in the background without switching to the workspace.
    #[serde(default)]
    pub silent: bool,
    #[serde(default)]
    pub float: bool,
    /// Initial size in pixels `[width, height]`; implies floating.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[u32; 2]>,
    /// 0.0 (invisible) to 1.0 (opaque).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    /// Show on all workspaces; implies floating.
    #[serde(default)]
    pub pin: bool,
}

/// A single change to a window rule's effects, as made in the UI.
#[derive(Debug, Clone, PartialEq)]
pub enum RuleEdit {
    Workspace(Option<String>),
    Silent(bool),
    Float(bool),
    Size(Option<[u32; 2]>),
    Opacity(Option<f32>),
    Pin(bool),
}

/// "Workspace `workspace` lives on monitor `monitor`."
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceRule {
    pub workspace: String,
    pub monitor: String,
    #[serde(default)]
    pub default: bool,
}

fn default_true() -> bool {
    true
}

impl WindowRule {
    /// Rule for an exact window class, e.g. picked from a running client.
    pub fn for_class(class: &str, workspace: impl Into<String>) -> Self {
        Self {
            name: slug(class),
            class: exact_regex(class),
            enabled: true,
            workspace: Some(workspace.into()),
            silent: true,
            float: false,
            size: None,
            opacity: None,
            pin: false,
        }
    }

    /// Hyprland only sizes and pins floating windows, so those imply float.
    pub fn is_floating(&self) -> bool {
        self.float || self.pin || self.size.is_some()
    }

    /// Whether the rule would change anything at all.
    pub fn has_effects(&self) -> bool {
        self.workspace.is_some() || self.is_floating() || self.opacity.is_some()
    }

    pub fn apply(&mut self, edit: RuleEdit) {
        match edit {
            RuleEdit::Workspace(ws) => self.workspace = ws,
            RuleEdit::Silent(silent) => self.silent = silent,
            RuleEdit::Float(float) => self.float = float,
            RuleEdit::Size(size) => self.size = size,
            // Two decimals are plenty and keep the generated Lua readable.
            RuleEdit::Opacity(opacity) => {
                self.opacity = opacity.map(|o| (o.clamp(0.0, 1.0) * 100.0).round() / 100.0)
            }
            RuleEdit::Pin(pin) => self.pin = pin,
        }
    }
}

impl RuleSet {
    /// Adds a window rule, making its name unique within the set.
    pub fn add_window_rule(&mut self, mut rule: WindowRule) {
        let base = rule.name.clone();
        let mut n = 2;
        while self.window_rules.iter().any(|r| r.name == rule.name) {
            rule.name = format!("{base}-{n}");
            n += 1;
        }
        self.window_rules.push(rule);
    }

    /// Moves a window rule so it ends up at index `to`. Order matters:
    /// Hyprland applies later rules on top of earlier ones.
    pub fn move_window_rule(&mut self, from: usize, to: usize) {
        let len = self.window_rules.len();
        if from >= len || from == to {
            return;
        }
        let rule = self.window_rules.remove(from);
        self.window_rules.insert(to.min(len - 1), rule);
    }

    /// Index of the rule matching exactly this window class, if any.
    pub fn rule_for_class(&self, class: &str) -> Option<usize> {
        let regex = exact_regex(class);
        self.window_rules.iter().position(|r| r.class == regex)
    }

    /// Puts an app on a workspace: updates its rule or creates one.
    /// Returns the rule's index.
    pub fn place_app(&mut self, class: &str, workspace: &str) -> usize {
        match self.rule_for_class(class) {
            Some(index) => {
                self.window_rules[index].workspace = Some(workspace.to_owned());
                index
            }
            None => {
                self.add_window_rule(WindowRule::for_class(class, workspace));
                self.window_rules.len() - 1
            }
        }
    }

    /// Removes the workspace from a rule; drops the rule if nothing is left.
    /// Returns whether the rule was removed.
    pub fn unplace_rule(&mut self, index: usize) -> bool {
        let Some(rule) = self.window_rules.get_mut(index) else {
            return false;
        };
        rule.workspace = None;
        if rule.has_effects() {
            return false;
        }
        self.window_rules.remove(index);
        true
    }

    /// Lets Hyprland decide where the workspace goes again.
    pub fn unbind_workspace(&mut self, workspace: &str) {
        self.workspace_rules.retain(|r| r.workspace != workspace);
    }

    /// Binds a workspace to a monitor, replacing an existing binding.
    pub fn set_workspace_monitor(&mut self, workspace: &str, monitor: &str) {
        match self
            .workspace_rules
            .iter_mut()
            .find(|r| r.workspace == workspace)
        {
            Some(rule) => rule.monitor = monitor.to_owned(),
            None => self.workspace_rules.push(WorkspaceRule {
                workspace: workspace.to_owned(),
                monitor: monitor.to_owned(),
                default: false,
            }),
        }
    }
}

/// `^(discord)$` with regex metacharacters escaped.
pub fn exact_regex(literal: &str) -> String {
    let mut out = String::from("^(");
    for c in literal.chars() {
        if "\\.+*?()|[]{}^$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out.push_str(")$");
    out
}

fn slug(s: &str) -> String {
    let slug: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-');
    if slug.is_empty() { "rule".into() } else { slug.into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_regex_escapes_metacharacters() {
        assert_eq!(exact_regex("discord"), "^(discord)$");
        assert_eq!(exact_regex("org.gnome.Nautilus"), r"^(org\.gnome\.Nautilus)$");
    }

    #[test]
    fn rule_names_stay_unique() {
        let mut set = RuleSet::default();
        set.add_window_rule(WindowRule::for_class("discord", "2"));
        set.add_window_rule(WindowRule::for_class("discord", "3"));
        assert_eq!(set.window_rules[1].name, "discord-2");
    }

    #[test]
    fn move_window_rule_reorders() {
        let mut set = RuleSet::default();
        for class in ["a", "b", "c"] {
            set.add_window_rule(WindowRule::for_class(class, "1"));
        }
        let names = |s: &RuleSet| s.window_rules.iter().map(|r| r.name.clone()).collect::<Vec<_>>();

        set.move_window_rule(0, 2);
        assert_eq!(names(&set), ["b", "c", "a"]);
        set.move_window_rule(2, 0);
        assert_eq!(names(&set), ["a", "b", "c"]);
        set.move_window_rule(1, 99);
        assert_eq!(names(&set), ["a", "c", "b"]);
        set.move_window_rule(99, 0);
        assert_eq!(names(&set), ["a", "c", "b"]);
    }

    #[test]
    fn size_and_pin_imply_floating() {
        let mut rule = WindowRule::for_class("mpv", "1");
        assert!(!rule.is_floating());
        rule.apply(RuleEdit::Size(Some([800, 600])));
        assert!(rule.is_floating());
        rule.apply(RuleEdit::Size(None));
        rule.apply(RuleEdit::Pin(true));
        assert!(rule.is_floating());
    }

    #[test]
    fn opacity_is_clamped_and_rounded() {
        let mut rule = WindowRule::for_class("kitty", "1");
        rule.apply(RuleEdit::Opacity(Some(0.854_999)));
        assert_eq!(rule.opacity, Some(0.85));
        rule.apply(RuleEdit::Opacity(Some(3.0)));
        assert_eq!(rule.opacity, Some(1.0));
    }

    #[test]
    fn old_rules_files_still_load() {
        let old = r#"
            [[window_rules]]
            name = "discord"
            class = "^(discord)$"
            workspace = "2"
            silent = true
            enabled = true
        "#;
        let set: RuleSet = toml::from_str(old).unwrap();
        let rule = &set.window_rules[0];
        assert_eq!(rule.workspace.as_deref(), Some("2"));
        assert!(!rule.float && rule.size.is_none() && rule.opacity.is_none());
    }

    #[test]
    fn place_app_updates_existing_rule() {
        let mut set = RuleSet::default();
        assert_eq!(set.place_app("discord", "2"), 0);
        assert_eq!(set.place_app("discord", "name:G"), 0);
        assert_eq!(set.window_rules.len(), 1);
        assert_eq!(set.window_rules[0].workspace.as_deref(), Some("name:G"));
    }

    #[test]
    fn unplace_keeps_rules_with_other_effects() {
        let mut set = RuleSet::default();
        set.place_app("mpv", "1");
        set.window_rules[0].apply(RuleEdit::Float(true));
        assert!(!set.unplace_rule(0));
        assert_eq!(set.window_rules[0].workspace, None);

        set.place_app("kitty", "1");
        assert!(set.unplace_rule(1));
        assert_eq!(set.window_rules.len(), 1);
    }

    #[test]
    fn unbind_workspace_removes_binding() {
        let mut set = RuleSet::default();
        set.set_workspace_monitor("2", "DP-1");
        set.unbind_workspace("2");
        assert!(set.workspace_rules.is_empty());
    }

    #[test]
    fn workspace_monitor_is_replaced_not_duplicated() {
        let mut set = RuleSet::default();
        set.set_workspace_monitor("2", "DP-1");
        set.set_workspace_monitor("2", "HDMI-A-1");
        assert_eq!(set.workspace_rules.len(), 1);
        assert_eq!(set.workspace_rules[0].monitor, "HDMI-A-1");
    }
}
