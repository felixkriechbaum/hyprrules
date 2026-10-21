//! Where rules are stored and where the generated Lua goes.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::lua;
use crate::model::RuleSet;

#[derive(Debug, Clone)]
pub struct Paths {
    /// Source of truth, edited only by hyprrules.
    pub rules_file: PathBuf,
    /// Hyprland config directory (contains `hyprland.lua`).
    pub hypr_dir: PathBuf,
}

impl Paths {
    /// Real locations: `~/.config/hyprrules/rules.toml` and `~/.config/hypr/`.
    pub fn system() -> Self {
        let config = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            rules_file: config.join("hyprrules").join("rules.toml"),
            hypr_dir: config.join("hypr"),
        }
    }

    /// Sandbox for the fake backend so nothing real gets touched.
    pub fn fake() -> Self {
        let base = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("hyprrules")
            .join("fake");
        Self {
            rules_file: base.join("rules.toml"),
            hypr_dir: base.join("hypr"),
        }
    }

    pub fn lua_file(&self) -> PathBuf {
        self.hypr_dir.join(format!("{}.lua", lua::MODULE_NAME))
    }

    pub fn load(&self) -> Result<RuleSet> {
        if !self.rules_file.exists() {
            return Ok(RuleSet::default());
        }
        let text = fs::read_to_string(&self.rules_file)
            .with_context(|| format!("reading {}", self.rules_file.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", self.rules_file.display()))
    }

    /// Writes `rules.toml` and the generated Lua module.
    pub fn save(&self, rules: &RuleSet) -> Result<()> {
        write_atomic(&self.rules_file, &toml::to_string_pretty(rules)?)?;
        write_atomic(&self.lua_file(), &lua::render(rules))
    }

    /// Whether any `.lua` file in the Hyprland config requires our module.
    pub fn is_included(&self) -> bool {
        lua_files(&self.hypr_dir)
            .iter()
            .filter(|p| *p != &self.lua_file())
            .filter_map(|p| fs::read_to_string(p).ok())
            .any(|text| requires_module(&text))
    }
}

fn requires_module(text: &str) -> bool {
    let needles = [
        format!("require(\"{}\")", lua::MODULE_NAME),
        format!("require('{}')", lua::MODULE_NAME),
        format!("require \"{}\"", lua::MODULE_NAME),
        format!("require '{}'", lua::MODULE_NAME),
    ];
    text.lines()
        .map(|l| l.split("--").next().unwrap_or_default())
        .any(|code| needles.iter().any(|n| code.contains(n.as_str())))
}

fn lua_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(lua_files(&path));
        } else if path.extension().is_some_and(|e| e == "lua") {
            found.push(path);
        }
    }
    found
}

fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, contents).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::WindowRule;

    fn temp_paths(dir: &Path) -> Paths {
        Paths {
            rules_file: dir.join("hyprrules").join("rules.toml"),
            hypr_dir: dir.join("hypr"),
        }
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let paths = temp_paths(dir.path());
        let mut rules = RuleSet::default();
        rules.add_window_rule(WindowRule::for_class("discord", "2"));
        rules.set_workspace_monitor("2", "DP-2");

        paths.save(&rules).unwrap();
        assert_eq!(paths.load().unwrap(), rules);
        assert!(paths.lua_file().exists());
    }

    #[test]
    fn detects_require_in_nested_files_but_not_comments() {
        let dir = tempfile::tempdir().unwrap();
        let paths = temp_paths(dir.path());
        fs::create_dir_all(paths.hypr_dir.join("config")).unwrap();
        fs::write(paths.hypr_dir.join("hyprland.lua"), "-- require(\"hyprrules\")\n").unwrap();
        assert!(!paths.is_included());

        fs::write(paths.hypr_dir.join("config").join("rules.lua"), "require(\"hyprrules\")\n").unwrap();
        assert!(paths.is_included());
    }
}
