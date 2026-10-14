mod background;
mod closing;
mod help;
mod preferences;
mod projects;
mod terminal;
mod tray;
mod ui;
use gtk::prelude::*;
use std::{cell::RefCell, rc::Rc};
fn main() -> glib::ExitCode {
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("TermLook {}", env!("CARGO_PKG_VERSION"));
        return glib::ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--print-default-settings") {
        println!(
            "{}",
            serde_json::to_string_pretty(&termlook::settings::Settings::default()).unwrap()
        );
        return glib::ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("TermLook {}\n\nUsage: termlook [--version | --print-default-settings | --help]\n\nConfiguration: $XDG_CONFIG_HOME/termlook (default ~/.config/termlook)\nSettings: Ctrl+,    Help: F1    Quit: Ctrl+Shift+Q", env!("CARGO_PKG_VERSION"));
        return glib::ExitCode::SUCCESS;
    }
    glib::set_prgname(Some("io.termlook.TermLook"));
    glib::set_application_name("TermLook");
    let app = gtk::Application::new(
        Some("io.termlook.TermLook"),
        gio::ApplicationFlags::FLAGS_NONE,
    );
    let state: Rc<RefCell<Option<ui::Shared>>> = Rc::new(RefCell::new(None));
    let s = state.clone();
    app.connect_activate(move |app| {
        if let Some(state) = s.borrow().as_ref() {
            state.borrow().restore();
            return;
        }
        *s.borrow_mut() = Some(ui::State::new(app, termlook::storage::config_dir()));
    });
    app.connect_shutdown(move |_| {
        if let Some(s) = state.borrow_mut().take() {
            s.borrow_mut().shutdown();
        }
    });
    app.run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{Duration, Instant},
    };
    fn pump(ms: u64) {
        let until = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < until {
            while glib::MainContext::default().pending() {
                glib::MainContext::default().iteration(false);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn wait_file(path: &std::path::Path) -> String {
        for _ in 0..100 {
            pump(20);
            if let Ok(text) = fs::read_to_string(path) {
                if !text.is_empty() {
                    return text;
                }
            }
        }
        panic!("shell did not write {}", path.display());
    }
    fn named_widget(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
        if root.widget_name() == name {
            return Some(root.clone());
        }
        root.clone()
            .downcast::<gtk::Container>()
            .ok()?
            .children()
            .iter()
            .find_map(|child| named_widget(child, name))
    }
    fn project_entry(dialog: &gtk::Dialog, name: &str) -> gtk::Entry {
        named_widget(dialog.upcast_ref(), name)
            .unwrap()
            .downcast()
            .unwrap()
    }
    fn close_response(state: &ui::Shared, response: gtk::ResponseType) {
        let dialog = state
            .borrow()
            .confirmation
            .clone()
            .expect("closing must ask first");
        dialog.response(response);
        pump(20);
    }
    #[test]
    #[ignore = "requires a display; run under dbus-run-session and xvfb-run"]
    fn gui_lifecycle() {
        gtk::init().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let app =
            gtk::Application::new(Some("io.termlook.Test"), gio::ApplicationFlags::NON_UNIQUE);
        app.register(None::<&gio::Cancellable>).unwrap();
        let layout = serde_json::json!([{"name":"test","tabs":(0..20).map(|i| serde_json::json!({"name":format!("tab {i}"),"cwd":dir.path()})).collect::<Vec<_>>()}]);
        termlook::storage::save(&dir.path().join("layout.json"), &layout).unwrap();
        let settings = termlook::settings::Settings {
            shell: "/bin/sh".into(),
            ..Default::default()
        };
        termlook::storage::save(
            &dir.path().join("settings.json"),
            &serde_json::to_value(settings).unwrap(),
        )
        .unwrap();
        let state = ui::State::new(&app, dir.path().into());
        pump(400);
        assert_eq!(state.borrow().sessions.len(), 1);
        let first = state.borrow().layout.tab().active;
        let session = state.borrow().sessions[&first].clone();
        session.send("TERMLOOK_TEST=alive; printf '%s' \"$$\" > shell.pid\n");
        let pid = wait_file(&dir.path().join("shell.pid"))
            .trim()
            .parse::<i32>()
            .unwrap();
        session.send("printf '\\033]7;file://localhost/tmp\\007'\n");
        pump(100);
        assert_eq!(state.borrow().layout.tab().active_pane().cwd, "/tmp");
        let mut duplicate = state.borrow().settings.clone();
        duplicate.keybindings.insert("new-tab".into(), "F2".into());
        assert!(ui::validate_shortcuts(&duplicate).is_err());
        state.borrow_mut().action("next-tab");
        pump(150);
        assert_eq!(state.borrow().sessions.len(), 2);
        state.borrow_mut().action("previous-tab");
        pump(150);
        session.send("printf '%s' \"$TERMLOOK_TEST\" > survived\n");
        assert_eq!(wait_file(&dir.path().join("survived")), "alive");
        state.borrow_mut().action("grid");
        pump(200);
        assert_eq!(state.borrow().layout.tab().panes.len(), 4);
        assert_eq!(state.borrow().sessions.len(), 5);
        if let Ok(path) = std::env::var("TERMLOOK_TEST_SCREENSHOT") {
            let window = state.borrow().window.clone();
            if let Some(surface) = window.window() {
                if let Some(pixbuf) =
                    surface.pixbuf(0, 0, window.allocated_width(), window.allocated_height())
                {
                    pixbuf.savev(path, "png", &[]).unwrap();
                }
            }
        }
        state.borrow_mut().action("zoom");
        pump(100);
        state.borrow_mut().action("zoom");
        pump(100);
        assert_eq!(state.borrow().sessions.len(), 5);
        let panes = state.borrow().layout.tab().panes.clone();
        state
            .borrow_mut()
            .layout
            .tab_mut()
            .swap(panes[0].id, panes[3].id);
        state.borrow_mut().render();
        pump(100);
        let mut settings = state.borrow().settings.clone();
        settings.font = "Monospace 13".into();
        settings.background = "#101020".into();
        state.borrow_mut().apply_settings(settings).unwrap();
        pump(100);
        session.send("printf '%s' \"$TERMLOOK_TEST\" > after-settings\n");
        assert_eq!(wait_file(&dir.path().join("after-settings")), "alive");
        for (index, pane) in panes.iter().enumerate() {
            let shell = state.borrow().sessions[&pane.id].clone();
            shell.send(&format!(
                "printf alive > '{}/pane-{index}'\n",
                dir.path().display()
            ));
            assert_eq!(
                wait_file(&dir.path().join(format!("pane-{index}"))),
                "alive"
            );
        }
        let image =
            gtk::gdk_pixbuf::Pixbuf::new(gtk::gdk_pixbuf::Colorspace::Rgb, false, 8, 64, 64)
                .unwrap();
        image.fill(0xe06020ff);
        let image_path = dir.path().join("background.png");
        image.savev(&image_path, "png", &[]).unwrap();
        let mut with_image = state.borrow().settings.clone();
        with_image.background_image = image_path.to_string_lossy().into();
        with_image.background_dim = 0;
        state
            .borrow_mut()
            .apply_settings(with_image.clone())
            .unwrap();
        pump(200);
        let window = state.borrow().window.clone();
        let surface = window.window().unwrap();
        let capture = surface
            .pixbuf(0, 0, window.allocated_width(), window.allocated_height())
            .unwrap();
        let bytes = capture.read_pixel_bytes();
        let stride = capture.rowstride() as usize;
        let channels = capture.n_channels() as usize;
        let mut colored = 0;
        for row in 0..capture.height() as usize {
            for col in 0..capture.width() as usize {
                let i = row * stride + col * channels;
                if bytes[i] > 180 && (60..130).contains(&bytes[i + 1]) && bytes[i + 2] < 60 {
                    colored += 1;
                }
            }
        }
        assert!(
            colored > (capture.width() * capture.height() / 5) as usize,
            "image must show through the VTE panes, got {colored} pixels"
        );
        let mut broken = with_image.clone();
        broken.background_image = dir.path().join("missing.png").to_string_lossy().into();
        assert!(state.borrow_mut().apply_settings(broken).is_err());
        assert_eq!(
            state.borrow().settings.background_image,
            with_image.background_image
        );
        with_image.background_image.clear();
        state.borrow_mut().apply_settings(with_image).unwrap();
        pump(50);
        state.borrow_mut().action("settings");
        pump(100);
        let dialog = state.borrow().preferences.clone().unwrap();
        dialog.response(gtk::ResponseType::Apply);
        pump(100);
        dialog.response(gtk::ResponseType::Cancel);
        pump(100);
        assert!(state.borrow().preferences.is_none());
        state.borrow_mut().action("close-tab");
        assert_eq!(state.borrow().sessions.len(), 5);
        close_response(&state, gtk::ResponseType::Cancel);
        assert_eq!(state.borrow().sessions.len(), 5);
        state.borrow_mut().action("close-tab");
        close_response(&state, gtk::ResponseType::Accept);
        pump(200);
        assert_eq!(state.borrow().sessions.len(), 1);
        drop(session);
        pump(200);
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
        for _ in 0..12 {
            state.borrow_mut().action("new-tab");
            state.borrow_mut().action("close-tab");
            close_response(&state, gtk::ResponseType::Accept);
        }
        pump(600);
        // Closing an appended tab selects the previous (formerly unopened) last tab.
        assert_eq!(state.borrow().sessions.len(), 2);
        let mut projects = state.borrow().settings.clone();
        projects.profiles.insert(
            "project-test".into(),
            termlook::settings::Profile {
                shell: "/bin/sh".into(),
                cwd: dir.path().to_string_lossy().into(),
                ..Default::default()
            },
        );
        state.borrow_mut().apply_settings(projects).unwrap();
        state.borrow_mut().open_project("project-test");
        pump(150);
        let workspace = state.borrow().layout.workspaces[state.borrow().layout.selected].id;
        let count = state.borrow().layout.workspaces.len();
        state.borrow_mut().open_project("project-test");
        assert_eq!(state.borrow().layout.workspaces.len(), count);
        assert_eq!(state.borrow().project_settings().shell(), "/bin/sh");
        let project = state.borrow().sessions[&state.borrow().layout.tab().active].clone();
        project.send("pwd > project-cwd\n");
        assert_eq!(
            wait_file(&dir.path().join("project-cwd")).trim(),
            dir.path().to_str().unwrap()
        );
        state.borrow_mut().action("split-horizontal");
        pump(100);
        state.borrow_mut().action("close-pane");
        close_response(&state, gtk::ResponseType::Cancel);
        assert_eq!(state.borrow().layout.tab().panes.len(), 2);
        state.borrow_mut().action("close-pane");
        close_response(&state, gtk::ResponseType::Accept);
        assert_eq!(state.borrow().layout.tab().panes.len(), 1);
        // Run real commands in background tabs, retain output, and report exit status.
        {
            let mut s = state.borrow_mut();
            let p = s.settings.profiles.get_mut("project-test").unwrap();
            p.autorun = true;
            p.notifications = "off".into();
            p.tasks = vec![
                termlook::settings::ProjectTask { name: "Success".into(), command: "pwd > task-cwd; printf started >> runs; printf 'live output\\n'; sleep 0.2; exit 0".into() },
                termlook::settings::ProjectTask { name: "Failure".into(), command: "printf 'example error\\n' >&2; exit 7".into() },
                termlook::settings::ProjectTask { name: "Signal".into(), command: "kill -TERM $$".into() },
            ];
            s.open_project("project-test");
        }
        let task_count = state.borrow().sessions.len();
        state.borrow_mut().open_project("project-test");
        assert_eq!(state.borrow().sessions.len(), task_count);
        assert_eq!(
            wait_file(&dir.path().join("task-cwd")).trim(),
            dir.path().to_str().unwrap()
        );
        pump(800);
        {
            let s = state.borrow();
            let tasks = &s.layout.workspaces[s.layout.selected].tabs;
            assert!(tasks.iter().any(|t| t.name == "✓ Success — Done"));
            assert!(tasks.iter().any(|t| t.name == "✗ Failure — Exit 7"));
            assert!(tasks.iter().any(|t| t.name == "✗ Signal — Signal 15"));
            assert!(s.project_tasks.values().all(|t| !t.running));
        }
        state.borrow_mut().open_project("project-test");
        pump(100);
        assert_eq!(
            fs::read_to_string(dir.path().join("runs")).unwrap(),
            "started"
        );
        state.borrow_mut().run_project_tasks(true);
        pump(800);
        assert_eq!(
            fs::read_to_string(dir.path().join("runs")).unwrap(),
            "startedstarted"
        );
        state
            .borrow_mut()
            .request_close(closing::Target::Workspace(workspace));
        close_response(&state, gtk::ResponseType::Cancel);
        assert_eq!(state.borrow().layout.workspaces.len(), count);
        state
            .borrow_mut()
            .request_close(closing::Target::Workspace(workspace));
        close_response(&state, gtk::ResponseType::Accept);
        assert_eq!(state.borrow().layout.workspaces.len(), count - 1);
        // Regression: choosing only a folder and closing Settings must persist a project.
        let folder = dir.path().join("folder-only-project");
        fs::create_dir(&folder).unwrap();
        preferences::show_page(&mut state.borrow_mut(), 4);
        let settings_dialog = state.borrow().preferences.clone().unwrap();
        project_entry(&settings_dialog, "project-folder").set_text(folder.to_str().unwrap());
        settings_dialog.close();
        pump(150);
        assert!(state.borrow().preferences.is_none());
        let saved = termlook::storage::read(&dir.path().join("settings.json")).unwrap();
        assert_eq!(
            saved["profiles"]["folder-only-project"]["cwd"],
            folder.to_str().unwrap()
        );
        state.borrow_mut().open_project("folder-only-project");
        assert_eq!(
            state.borrow().layout.tab().panes[0].cwd,
            folder.to_str().unwrap()
        );

        // Apply also captures the form without a preliminary Add / update click.
        preferences::show_page(&mut state.borrow_mut(), 4);
        let settings_dialog = state.borrow().preferences.clone().unwrap();
        project_entry(&settings_dialog, "project-name").set_text("direct-apply");
        project_entry(&settings_dialog, "project-folder").set_text(folder.to_str().unwrap());
        settings_dialog.response(gtk::ResponseType::Apply);
        pump(100);
        assert!(state
            .borrow()
            .settings
            .profiles
            .contains_key("direct-apply"));
        let selector: gtk::ListBox = named_widget(settings_dialog.upcast_ref(), "project-selector")
            .unwrap()
            .downcast()
            .unwrap();
        assert_eq!(
            selector.selected_row().unwrap().widget_name(),
            "direct-apply"
        );
        let commands: gtk::TextView =
            named_widget(settings_dialog.upcast_ref(), "project-commands")
                .unwrap()
                .downcast()
                .unwrap();
        commands.buffer().unwrap().set_text("missing separator");
        settings_dialog.close();
        pump(100);
        assert!(state.borrow().preferences.is_some());
        assert!(state.borrow().settings.profiles["direct-apply"]
            .tasks
            .is_empty());
        // Discard remains an explicit way out of an invalid draft.
        settings_dialog.response(gtk::ResponseType::Cancel);
        pump(100);
        assert!(state.borrow().preferences.is_none());

        // Removing a selected profile must not reinsert its still-visible form on save.
        preferences::show_page(&mut state.borrow_mut(), 4);
        let settings_dialog = state.borrow().preferences.clone().unwrap();
        let selector: gtk::ListBox = named_widget(settings_dialog.upcast_ref(), "project-selector")
            .unwrap()
            .downcast()
            .unwrap();
        let row = selector
            .children()
            .into_iter()
            .find(|w| w.widget_name() == "direct-apply")
            .unwrap()
            .downcast::<gtk::ListBoxRow>()
            .unwrap();
        selector.select_row(Some(&row));
        let remove: gtk::Button = named_widget(settings_dialog.upcast_ref(), "project-remove")
            .unwrap()
            .downcast()
            .unwrap();
        remove.emit_clicked();
        settings_dialog.response(gtk::ResponseType::Accept);
        pump(100);
        assert!(!state
            .borrow()
            .settings
            .profiles
            .contains_key("direct-apply"));
        assert!(state.borrow().preferences.is_none());
        // Saved projects are visible and can be deleted directly from the manager.
        state.borrow_mut().settings.default_profile = "folder-only-project".into();
        let sessions_before = state.borrow().sessions.len();
        state.borrow().projects();
        fn named_dialog(name: &str) -> gtk::Dialog {
            gtk::Window::list_toplevels()
                .into_iter()
                .find(|w| w.widget_name() == name)
                .unwrap()
                .downcast()
                .unwrap()
        }
        let manager = named_dialog("project-manager");
        let edit: gtk::Button =
            named_widget(manager.upcast_ref(), "edit-project-folder-only-project")
                .unwrap()
                .downcast()
                .unwrap();
        edit.emit_clicked();
        pump(100);
        let settings_dialog = state.borrow().preferences.clone().unwrap();
        assert_eq!(
            project_entry(&settings_dialog, "project-name").text(),
            "folder-only-project"
        );
        settings_dialog.response(gtk::ResponseType::Accept);
        pump(100);
        state.borrow().projects();
        let manager = named_dialog("project-manager");
        let delete: gtk::Button =
            named_widget(manager.upcast_ref(), "delete-project-folder-only-project")
                .unwrap()
                .downcast()
                .unwrap();
        assert!(delete.is_visible());
        delete.emit_clicked();
        named_dialog("project-delete-confirmation").response(gtk::ResponseType::Cancel);
        pump(50);
        assert!(state
            .borrow()
            .settings
            .profiles
            .contains_key("folder-only-project"));
        delete.emit_clicked();
        named_dialog("project-delete-confirmation").response(gtk::ResponseType::Accept);
        pump(100);
        assert!(!state
            .borrow()
            .settings
            .profiles
            .contains_key("folder-only-project"));
        assert!(state.borrow().settings.default_profile.is_empty());
        assert_eq!(state.borrow().sessions.len(), sessions_before);
        assert!(folder.is_dir());
        let saved = termlook::storage::read(&dir.path().join("settings.json")).unwrap();
        assert!(saved["profiles"].get("folder-only-project").is_none());
        let manager = named_dialog("project-manager");
        assert!(named_widget(manager.upcast_ref(), "delete-project-folder-only-project").is_none());
        manager.response(gtk::ResponseType::Cancel);
        drop(project);
        state.borrow_mut().action("quit");
        close_response(&state, gtk::ResponseType::Cancel);
        assert!(!state.borrow().sessions.is_empty());
        state.borrow_mut().action("quit");
        close_response(&state, gtk::ResponseType::Accept);
        pump(100);
        assert!(state.borrow().sessions.is_empty());
        assert!(termlook::storage::read(&dir.path().join("layout.json")).is_ok());
    }
}
