use serde_json::json;
use termlook::{
    model::{Axis, Layout, Tree},
    settings::Settings,
    storage,
};

#[test]
fn legacy_layout_roundtrip_and_split_validation() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().to_str().unwrap();
    let original = json!([{"name":"project", "tabs":[{"name":"server", "cwd":cwd, "panes":[{"cwd":cwd},{"cwd":cwd}], "split":{"axis":"vertical","ratio":0.35,"first":0,"second":{"axis":"horizontal","ratio":0.6,"first":1,"second":2}}}]}]);
    let layout = Layout::from_data(&original);
    assert_eq!(layout.to_data(), original);
    let mut invalid = original.clone();
    invalid[0]["tabs"][0]["split"]["first"] = json!(1);
    let layout = Layout::from_data(&invalid);
    let tab = layout.tab();
    assert_eq!(tab.tree.leaves().len(), 3);
    assert_eq!(
        tab.tree,
        Tree::default_for(&tab.panes.iter().map(|p| p.id).collect::<Vec<_>>())
    );
}
#[test]
fn split_swap_close_and_last_workspace_preserve_invariants() {
    let mut layout = Layout::default();
    let original = layout.tab().active;
    layout.tab_mut().split(Axis::Vertical);
    layout.tab_mut().split(Axis::Horizontal);
    layout.tab_mut().split(Axis::Vertical);
    layout.tab_mut().split(Axis::Horizontal);
    assert_eq!(layout.tab().panes.len(), 4);
    let last = layout.tab().active;
    layout.tab_mut().swap(original, last);
    assert_eq!(layout.tab().tree.leaves()[0], last);
    layout.tab_mut().close_pane(original);
    assert_eq!(layout.tab().panes.len(), 3);
    assert!(!layout.tab().tree.leaves().contains(&original));
    while layout.tab().panes.len() > 1 {
        let active = layout.tab().active;
        layout.tab_mut().close_pane(active);
    }
    let id = layout.tab().id;
    layout.close_tab(id);
    assert_ne!(layout.tab().id, id);
    let id = layout.workspaces[0].id;
    layout.remove_workspace(id);
    assert_eq!(layout.workspaces.len(), 1);
    assert_ne!(layout.workspaces[0].id, id);
}
#[test]
fn malformed_state_is_bounded_and_safe() {
    for value in [json!(null), json!({}), json!([]), json!([{"tabs":[]}])] {
        let layout = Layout::from_data(&value);
        assert_eq!(layout.workspaces.len(), 1);
        assert_eq!(layout.tab().panes.len(), 1);
    }
    let tabs = vec![json!({"name":"x", "cwd":"/does/not/exist", "panes":vec![json!({});10]}); 100];
    let layout = Layout::from_data(&json!(vec![json!({"name":"x","tabs":tabs}); 60]));
    assert_eq!(layout.workspaces.len(), 50);
    assert_eq!(layout.workspaces[0].tabs.len(), 50);
    assert_eq!(layout.tab().panes.len(), 4);
}
#[test]
fn settings_migrate_validate_and_preserve_independent_fields() {
    let s = Settings::from_data(
        &json!({"font":"Mono 14","background":"bad","scrollback":true,"padding":18,"unknown":42}),
    );
    assert_eq!(s.font, "Mono 14");
    assert_eq!(s.background, Settings::default().background);
    assert_eq!(s.scrollback, 20000);
    assert_eq!(s.padding, 18);
    let s = Settings::from_data(
        &json!({"profiles":{"dev":{"shell":"/bin/sh","cwd":"/tmp"}},"default_profile":"dev"}),
    );
    assert_eq!(s.shell(), "/bin/sh");
    assert!(s.validate().is_ok());
    let mut invalid = s;
    invalid.palette.pop();
    assert!(invalid.validate().is_err());
}
#[test]
fn atomic_storage_and_corrupt_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config/layout.json");
    let data = Layout::default().to_data();
    storage::save(&path, &data).unwrap();
    assert_eq!(storage::read(&path).unwrap(), data);
    storage::save(&path, &json!([])).unwrap();
    assert_eq!(storage::read(&path).unwrap(), json!([]));
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    std::fs::write(&path, "{broken").unwrap();
    assert!(storage::read(&path).is_err());
    assert_eq!(storage::load_layout(&path).workspaces.len(), 1);
}
#[test]
fn selection_survives_closing_other_tabs_and_workspaces() {
    use termlook::model::{Tab, Workspace};
    let mut l = Layout::default();
    l.workspaces[0].tabs.push(Tab::new("two", "/tmp".into()));
    l.workspaces[0].tabs.push(Tab::new("three", "/tmp".into()));
    l.workspaces[0].selected = 2;
    let selected = l.tab().id;
    let first = l.workspaces[0].tabs[0].id;
    l.close_tab(first);
    assert_eq!(l.tab().id, selected);
    l.workspaces.push(Workspace::new("two"));
    l.selected = 1;
    let selected = l.workspaces[1].id;
    let first = l.workspaces[0].id;
    l.remove_workspace(first);
    assert_eq!(l.workspaces[l.selected].id, selected);
}
#[test]
fn shortcuts_are_formatted_for_humans() {
    use termlook::settings::{shortcut_keys, shortcut_label};
    assert_eq!(shortcut_label("<Primary><Shift>t"), "Ctrl+Shift+T");
    assert_eq!(shortcut_label("<Primary>comma"), "Ctrl+,");
    assert_eq!(shortcut_label("<Primary><Shift>Return"), "Ctrl+Shift+Enter");
    assert_eq!(shortcut_keys("F2"), ["F2"]);
    assert!(shortcut_keys("").is_empty());
}

#[test]
fn background_settings_and_visible_close_buttons() {
    let defaults = Settings::default();
    assert!(defaults.show_tab_close_buttons);
    let mut settings = defaults.clone();
    settings.background_image = "/tmp/a photo.png".into();
    settings.background_fit = "contain".into();
    settings.background_dim = 45;
    assert!(settings.validate().is_ok());
    settings.background_dim = 101;
    assert!(settings.validate().is_err());
    settings.background_dim = 50;
    settings.background_image = "https://example.com/image.png".into();
    assert!(settings.validate().is_err());
    let legacy = Settings::from_data(&json!({"font":"Mono 13"}));
    assert!(legacy.background_image.is_empty());
    assert_eq!(legacy.font, "Mono 13");
}
#[test]
fn project_identity_survives_layout_roundtrip() {
    let mut layout = Layout::default();
    layout.workspaces[0].profile = "development".into();
    let restored = Layout::from_data(&layout.to_data());
    assert_eq!(restored.workspaces[0].profile, "development");
    let legacy = Layout::from_data(&json!([{"name":"work","tabs":[]}]));
    assert!(legacy.workspaces[0].profile.is_empty());
}

#[test]
fn project_tasks_validate_and_migrate() {
    let legacy: termlook::settings::Profile =
        serde_json::from_value(json!({"shell":"/bin/sh","cwd":"/tmp"})).unwrap();
    assert!(legacy.tasks.is_empty());
    assert!(!legacy.autorun);
    let mut settings = Settings::default();
    let profile = termlook::settings::Profile {
        autorun: true,
        notifications: "all".into(),
        tasks: vec![termlook::settings::ProjectTask {
            name: "Tests".into(),
            command: "printf 'a|b'; exit 7".into(),
        }],
        ..legacy
    };
    settings.profiles.insert("demo".into(), profile);
    let restored: Settings =
        serde_json::from_value(serde_json::to_value(&settings).unwrap()).unwrap();
    assert!(restored.validate().is_ok());
    assert!(restored.profiles["demo"].autorun);
    let task = settings.profiles["demo"].tasks[0].clone();
    settings.profiles.get_mut("demo").unwrap().tasks.push(task);
    assert!(settings.validate().is_err());
    settings.profiles.get_mut("demo").unwrap().tasks.pop();
    settings.profiles.get_mut("demo").unwrap().tasks[0].command = "bad\0command".into();
    assert!(settings.validate().is_err());
}
