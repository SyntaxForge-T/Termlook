use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const ACTIONS: &[(&str, &str, &str)] = &[
    ("projects", "Open project", "<Primary><Shift>p"),
    ("new-tab", "New tab", "<Primary><Shift>t"),
    ("new-workspace", "New workspace", "<Primary><Shift>n"),
    ("close-tab", "Close tab", "<Primary><Shift>w"),
    (
        "split-horizontal",
        "Split left / right",
        "<Primary><Shift>e",
    ),
    ("split-vertical", "Split top / bottom", "<Primary><Shift>d"),
    ("close-pane", "Close pane", "<Primary><Shift>x"),
    ("zoom", "Focus pane / show all", "<Primary><Shift>Return"),
    ("next-tab", "Next tab", "<Primary>Tab"),
    ("previous-tab", "Previous tab", "<Primary><Shift>Tab"),
    ("copy", "Copy", "<Primary><Shift>c"),
    ("paste", "Paste", "<Primary><Shift>v"),
    ("rename-tab", "Rename tab", "F2"),
    ("help", "Help", "F1"),
    ("settings", "Settings", "<Primary>comma"),
    ("quit", "Quit", "<Primary><Shift>q"),
    ("grid", "Four panes", ""),
    ("reload-settings", "Reload settings", "<Primary><Shift>r"),
];
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectTask {
    pub name: String,
    pub command: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Profile {
    pub shell: String,
    pub cwd: String,
    pub tasks: Vec<ProjectTask>,
    pub autorun: bool,
    /// Empty (legacy) and "errors" notify on failure only.
    pub notifications: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub font: String,
    pub foreground: String,
    pub background: String,
    pub background_image: String,
    pub background_dim: i32,
    pub background_fit: String,
    pub palette: Vec<String>,
    pub scrollback: i64,
    pub cursor: String,
    pub blink: bool,
    pub audible_bell: bool,
    pub scroll_on_output: bool,
    pub scroll_on_keystroke: bool,
    pub padding: i32,
    pub sidebar_width: i32,
    pub shell: String,
    pub close_to_tray: bool,
    pub show_tab_close_buttons: bool,
    pub keybindings: BTreeMap<String, String>,
    pub profiles: BTreeMap<String, Profile>,
    pub default_profile: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            font: "Monospace 11".into(),
            foreground: "#d8d6eb".into(),
            background: "#202127".into(),
            background_image: String::new(),
            background_dim: 70,
            background_fit: "cover".into(),
            palette: [
                "#454158", "#ef8199", "#a9d794", "#edd49a", "#8eb9ee", "#c4a0ed", "#80d8cb",
                "#d8d6eb", "#77718c", "#f5a1b5", "#c0e7ad", "#f5dfb1", "#afd0ff", "#d7bdf6",
                "#a1ebe0", "#ffffff",
            ]
            .map(str::to_owned)
            .to_vec(),
            scrollback: 20000,
            cursor: "block".into(),
            blink: false,
            audible_bell: false,
            scroll_on_output: false,
            scroll_on_keystroke: true,
            padding: 12,
            sidebar_width: 182,
            shell: String::new(),
            close_to_tray: true,
            show_tab_close_buttons: true,
            keybindings: ACTIONS
                .iter()
                .map(|(a, _, k)| (a.to_string(), k.to_string()))
                .collect(),
            profiles: BTreeMap::new(),
            default_profile: String::new(),
        }
    }
}
/// Split a GTK accelerator such as `<Primary><Shift>comma` into readable
/// key names (`["Ctrl", "Shift", ","]`). An empty binding yields no keys.
pub fn shortcut_keys(binding: &str) -> Vec<String> {
    let mut keys = vec![];
    let mut rest = binding.trim();
    while let Some(tail) = rest.strip_prefix('<') {
        let Some((name, after)) = tail.split_once('>') else {
            break;
        };
        keys.push(
            match name.to_ascii_lowercase().as_str() {
                "primary" | "control" | "ctrl" => "Ctrl",
                "shift" => "Shift",
                "alt" | "mod1" => "Alt",
                "super" | "mod4" => "Super",
                "meta" => "Meta",
                _ => name,
            }
            .to_owned(),
        );
        rest = after;
    }
    if !rest.is_empty() {
        keys.push(match rest {
            "comma" => ",".into(),
            "period" => ".".into(),
            "plus" => "+".into(),
            "minus" => "-".into(),
            "slash" => "/".into(),
            "space" => "Space".into(),
            "Return" | "KP_Enter" => "Enter".into(),
            "Escape" => "Esc".into(),
            key if key.chars().count() == 1 => key.to_uppercase(),
            key => key.to_owned(),
        });
    }
    keys
}
/// Human readable shortcut such as `Ctrl+Shift+T`; empty when unassigned.
pub fn shortcut_label(binding: &str) -> String {
    shortcut_keys(binding).join("+")
}
fn color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if self.background_image.contains('\0')
            || (!self.background_image.is_empty()
                && !std::path::Path::new(&self.background_image).is_absolute())
        {
            return Err("Background image must be an absolute local file path".into());
        }
        if !(0..=100).contains(&self.background_dim)
            || !["cover", "contain", "stretch"].contains(&self.background_fit.as_str())
        {
            return Err("Background dimming: 0–100; fit: cover, contain or stretch".into());
        }
        if self.font.contains('\0') || self.font.trim().is_empty() || self.font.len() > 200 {
            return Err("Font must contain 1–200 characters".into());
        }
        if !color(&self.foreground)
            || !color(&self.background)
            || self.palette.len() != 16
            || !self.palette.iter().all(|c| color(c))
        {
            return Err("Colors must be #RRGGBB; the ANSI palette must contain 16 colors".into());
        }
        if !["block", "ibeam", "underline"].contains(&self.cursor.as_str()) {
            return Err("Cursor must be block, ibeam or underline".into());
        }
        if !(100..=1_000_000).contains(&self.scrollback)
            || !(0..=40).contains(&self.padding)
            || !(140..=320).contains(&self.sidebar_width)
        {
            return Err("Scrollback: 100–1000000; padding: 0–40; sidebar width: 140–320".into());
        }
        if self.shell.contains('\0')
            || self
                .profiles
                .values()
                .any(|p| p.shell.contains('\0') || p.cwd.contains('\0'))
        {
            return Err("Shell and directory cannot contain NUL".into());
        }
        for profile in self.profiles.values() {
            if !["", "errors", "all", "off"].contains(&profile.notifications.as_str()) {
                return Err("Project notifications must be errors, all or off".into());
            }
            if profile.tasks.len() > 12 {
                return Err("A project can have at most 12 startup tasks".into());
            }
            let mut names = std::collections::HashSet::new();
            for task in &profile.tasks {
                if task.name.trim().is_empty()
                    || task.name.chars().count() > 80
                    || task.name.contains('|')
                    || task.name.chars().any(char::is_control)
                    || task.command.trim().is_empty()
                    || task.command.len() > 16384
                    || task.command.contains(['\0', '\n', '\r'])
                    || !names.insert(&task.name)
                {
                    return Err("Tasks need unique names (1–80 characters) and nonempty commands (one line, up to 16 KiB, no NUL)".into());
                }
            }
        }
        if !self.default_profile.is_empty() && !self.profiles.contains_key(&self.default_profile) {
            return Err("Default profile must match an existing profile name".into());
        }
        if self
            .keybindings
            .keys()
            .any(|key| !ACTIONS.iter().any(|(action, _, _)| action == key))
        {
            return Err("Unknown shortcut action".into());
        }
        Ok(())
    }
    /// Tolerate old/partially invalid preferences one field at a time.
    pub fn from_data(v: &Value) -> Self {
        let mut result = serde_json::to_value(Self::default()).unwrap();
        if let Some(map) = v.as_object() {
            for (key, value) in map {
                if key == "default_profile" {
                    continue;
                }
                if !result.as_object().unwrap().contains_key(key) {
                    continue;
                }
                let mut candidate = result.clone();
                candidate[key] = value.clone();
                if let Ok(settings) = serde_json::from_value::<Self>(candidate.clone()) {
                    if settings.validate().is_ok() {
                        result = candidate;
                    }
                }
            }
            if let Some(value) = map.get("default_profile") {
                let mut candidate = result.clone();
                candidate["default_profile"] = value.clone();
                if let Ok(settings) = serde_json::from_value::<Self>(candidate) {
                    if settings.validate().is_ok() {
                        return settings;
                    }
                }
            }
        }
        serde_json::from_value(result).unwrap()
    }
    pub fn profile(&self) -> Option<&Profile> {
        self.profiles.get(&self.default_profile)
    }
    pub fn shell(&self) -> &str {
        self.profile()
            .map(|p| p.shell.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.shell)
    }
    pub fn binding(&self, action: &str) -> &str {
        self.keybindings
            .get(action)
            .map(String::as_str)
            .unwrap_or_else(|| {
                ACTIONS
                    .iter()
                    .find(|(a, _, _)| *a == action)
                    .map(|(_, _, k)| *k)
                    .unwrap_or("")
            })
    }
}
