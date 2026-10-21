//! Key and mouse bindings as Hyprland reports them (`hyprctl binds`).
//! Read only: hyprrules shows them but leaves the config alone.

use serde::Deserialize;

/// Modifier bits of `modmask`, in the order they are shown.
const MODIFIERS: [(u32, &str); 8] = [
    (64, "SUPER"),
    (4, "CTRL"),
    (8, "ALT"),
    (1, "SHIFT"),
    (2, "CAPS"),
    (16, "MOD2"),
    (32, "MOD3"),
    (128, "MOD5"),
];

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Bind {
    #[serde(default)]
    pub modmask: u32,
    /// Empty for the global submap.
    #[serde(default)]
    pub submap: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub keycode: i64,
    #[serde(default)]
    pub mouse: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub release: bool,
    #[serde(default)]
    pub repeat: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub dispatcher: String,
    #[serde(default)]
    pub arg: String,
}

impl Bind {
    /// Modifiers and key, e.g. `["SUPER", "SHIFT", "Q"]`.
    pub fn keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = MODIFIERS
            .iter()
            .filter(|(bit, _)| self.modmask & bit != 0)
            .map(|(_, name)| (*name).to_owned())
            .collect();
        keys.push(self.key_name());
        keys
    }

    fn key_name(&self) -> String {
        match self.key.as_str() {
            "" if self.keycode != 0 => format!("code:{}", self.keycode),
            "mouse:272" => "Left click".into(),
            "mouse:273" => "Right click".into(),
            "mouse:274" => "Middle click".into(),
            "mouse_up" => "Scroll up".into(),
            "mouse_down" => "Scroll down".into(),
            key if key.chars().count() == 1 => key.to_uppercase(),
            key => key.to_owned(),
        }
    }

    /// What the bind does: its description, or dispatcher and argument.
    pub fn action(&self) -> String {
        if !self.description.is_empty() {
            return self.description.clone();
        }
        format!("{} {}", self.dispatcher, self.arg).trim().to_owned()
    }

    /// Flags worth showing next to the bind.
    pub fn flags(&self) -> Vec<&'static str> {
        [
            (self.mouse, "mouse"),
            (self.locked, "locked"),
            (self.repeat, "repeat"),
            (self.release, "on release"),
        ]
        .into_iter()
        .filter_map(|(on, flag)| on.then_some(flag))
        .collect()
    }

    /// Case-insensitive search over keys and action.
    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return true;
        }
        let haystack = format!("{} {} {} {}", self.keys().join(" + "), self.action(), self.dispatcher, self.arg).to_lowercase();
        query.split_whitespace().all(|word| haystack.contains(word))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bind(modmask: u32, key: &str, dispatcher: &str, arg: &str) -> Bind {
        Bind {
            modmask,
            key: key.into(),
            dispatcher: dispatcher.into(),
            arg: arg.into(),
            ..Bind::default()
        }
    }

    #[test]
    fn keys_list_modifiers_in_display_order() {
        assert_eq!(bind(65, "q", "killactive", "").keys(), ["SUPER", "SHIFT", "Q"]);
        assert_eq!(bind(0, "XF86AudioMute", "exec", "wpctl").keys(), ["XF86AudioMute"]);
        assert_eq!(bind(64, "mouse:272", "movewindow", "").keys(), ["SUPER", "Left click"]);
    }

    #[test]
    fn keycode_binds_name_the_code() {
        let mut b = bind(64, "", "workspace", "1");
        b.keycode = 10;
        assert_eq!(b.keys(), ["SUPER", "code:10"]);
    }

    #[test]
    fn action_prefers_description() {
        let mut b = bind(64, "Return", "exec", "kitty");
        assert_eq!(b.action(), "exec kitty");
        b.description = "Terminal".into();
        assert_eq!(b.action(), "Terminal");
    }

    #[test]
    fn search_matches_all_words() {
        let b = bind(65, "1", "movetoworkspace", "1");
        assert!(b.matches("super shift"));
        assert!(b.matches("MOVETO"));
        assert!(!b.matches("super ctrl"));
        assert!(b.matches("  "));
    }

    #[test]
    fn parses_hyprctl_output() {
        let json = r#"[{"locked": false, "mouse": false, "release": false, "repeat": true,
            "non_consuming": false, "has_description": false, "modmask": 64, "submap": "",
            "key": "Return", "keycode": 0, "catch_all": false, "description": "",
            "dispatcher": "exec", "arg": "kitty"}]"#;
        let binds: Vec<Bind> = serde_json::from_str(json).unwrap();
        assert_eq!(binds[0].keys(), ["SUPER", "Return"]);
        assert_eq!(binds[0].flags(), ["repeat"]);
    }
}
