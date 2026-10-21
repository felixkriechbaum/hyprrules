use serde::{Deserialize, Serialize};

/// Everything hyprrules manages. Persisted as TOML, rendered to Lua.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RuleSet {
    #[serde(default)]
    pub window_rules: Vec<WindowRule>,
    #[serde(default)]
    pub workspace_rules: Vec<WorkspaceRule>,
}

/// "Windows matching `class` open on `workspace`."
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowRule {
    /// Unique, human readable name; also used as the Hyprland rule name.
    pub name: String,
    /// Regex matched against the window class.
    pub class: String,
    pub workspace: String,
    /// Open in the background without switching to the workspace.
    #[serde(default)]
    pub silent: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
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
            workspace: workspace.into(),
            silent: true,
            enabled: true,
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
    fn workspace_monitor_is_replaced_not_duplicated() {
        let mut set = RuleSet::default();
        set.set_workspace_monitor("2", "DP-1");
        set.set_workspace_monitor("2", "HDMI-A-1");
        assert_eq!(set.workspace_rules.len(), 1);
        assert_eq!(set.workspace_rules[0].monitor, "HDMI-A-1");
    }
}
