//! Workspaces are stored as Hyprland selectors: `"2"` for numbered and
//! `"name:G"` for named workspaces. These helpers convert between what
//! Hyprland reports, what the user types and what the UI shows.

/// Selector for a workspace reported by Hyprland (`id` + `name`).
/// Special workspaces (scratchpads) are skipped.
pub fn from_ipc(id: i64, name: &str) -> Option<String> {
    if name.is_empty() || name.starts_with("special") {
        return None;
    }
    if id > 0 && name.parse::<i64>() == Ok(id) {
        return Some(id.to_string());
    }
    Some(format!("name:{name}"))
}

/// Strict parse of a selector from the Hyprland config. Only plain numbers
/// and `name:` selectors are kept; rule selectors like `w[tv1]` are not
/// concrete workspaces.
pub fn from_config(selector: &str) -> Option<String> {
    let selector = selector.trim();
    if selector.parse::<u32>().is_ok_and(|n| n > 0) {
        return Some(selector.to_owned());
    }
    let name = selector.strip_prefix("name:")?;
    (!name.is_empty()).then(|| selector.to_owned())
}

/// Lenient parse of user input: `"2"` stays numbered, anything else
/// (`"G"` or `"name:G"`) becomes a named workspace.
pub fn from_input(input: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    from_config(input).or_else(|| Some(format!("name:{input}")))
}

/// What the UI shows: `"name:G"` -> `"G"`.
pub fn label(selector: &str) -> &str {
    selector.strip_prefix("name:").unwrap_or(selector)
}

/// Numbered workspaces ascending, then named ones alphabetically; no duplicates.
pub fn sorted(mut selectors: Vec<String>) -> Vec<String> {
    selectors.sort_by(|a, b| {
        match (a.parse::<u32>(), b.parse::<u32>()) {
            (Ok(x), Ok(y)) => x.cmp(&y),
            (Ok(_), Err(_)) => std::cmp::Ordering::Less,
            (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
            (Err(_), Err(_)) => label(a).to_lowercase().cmp(&label(b).to_lowercase()),
        }
    });
    selectors.dedup();
    selectors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_workspaces() {
        assert_eq!(from_ipc(2, "2").as_deref(), Some("2"));
        assert_eq!(from_ipc(-1337, "G").as_deref(), Some("name:G"));
        assert_eq!(from_ipc(-98, "special:magic"), None);
    }

    #[test]
    fn config_selectors() {
        assert_eq!(from_config("3").as_deref(), Some("3"));
        assert_eq!(from_config("name:G").as_deref(), Some("name:G"));
        assert_eq!(from_config("w[tv1]"), None);
        assert_eq!(from_config("0"), None);
    }

    #[test]
    fn user_input() {
        assert_eq!(from_input(" G ").as_deref(), Some("name:G"));
        assert_eq!(from_input("name:G").as_deref(), Some("name:G"));
        assert_eq!(from_input("4").as_deref(), Some("4"));
        assert_eq!(from_input("  "), None);
    }

    #[test]
    fn sorting() {
        let input = ["name:G", "10", "2", "name:chat", "2", "1"].map(String::from).to_vec();
        assert_eq!(sorted(input), ["1", "2", "10", "name:chat", "name:G"]);
    }
}
