use crate::ui::{enqueue, State};
use gtk::prelude::*;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    path::Path,
    rc::Rc,
};
use termlook::{
    model::{home, Id, Tab, Workspace},
    settings::{Profile, ProjectTask, Settings},
};

pub struct RunningTask {
    pub workspace: Id,
    pub tab: Id,
    pub name: String,
    pub command: String,
    pub notifications: String,
    pub running: bool,
}

pub fn choose_folder(parent: &impl IsA<gtk::Window>, entry: &gtk::Entry) {
    let dialog = gtk::FileChooserDialog::new(
        Some("Project folder"),
        Some(parent),
        gtk::FileChooserAction::SelectFolder,
    );
    dialog.add_buttons(&[
        ("Cancel", gtk::ResponseType::Cancel),
        ("Select folder", gtk::ResponseType::Accept),
    ]);
    if !entry.text().is_empty() {
        dialog.set_filename(entry.text().as_str());
    }
    let entry = entry.clone();
    dialog.connect_response(move |d, r| {
        if r == gtk::ResponseType::Accept {
            if let Some(path) = d.filename() {
                entry.set_text(&path.to_string_lossy());
            }
        }
        unsafe {
            d.destroy();
        }
    });
    dialog.show();
}
// Visible rows instead of an empty-looking collapsed dropdown.
#[derive(Clone)]
struct ProfileList(gtk::ListBox);
impl ProfileList {
    fn new() -> Self {
        let list = gtk::ListBox::new();
        list.set_widget_name("project-selector");
        Self(list)
    }
    fn append(&self, id: Option<&str>, name: &str) {
        let row = gtk::ListBoxRow::new();
        row.set_widget_name(id.unwrap_or(name));
        let label = gtk::Label::new(Some(name));
        label.set_xalign(0.0);
        label.set_margin_start(10);
        label.set_margin_end(10);
        label.set_margin_top(8);
        label.set_margin_bottom(8);
        label.set_ellipsize(pango::EllipsizeMode::Middle);
        row.add(&label);
        self.0.add(&row);
        row.show_all();
    }
    fn remove_all(&self) {
        for child in self.0.children() {
            unsafe {
                child.destroy();
            }
        }
    }
    fn active_id(&self) -> Option<glib::GString> {
        self.0.selected_row().map(|row| row.widget_name())
    }
    fn set_active_id(&self, id: Option<&str>) {
        let row = self
            .0
            .children()
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::ListBoxRow>().ok())
            .find(|row| Some(row.widget_name().as_str()) == id);
        self.0.select_row(row.as_ref());
    }
    fn connect_changed(&self, callback: impl Fn(&Self) + 'static) {
        self.0
            .connect_row_selected(move |list, _| callback(&Self(list.clone())));
    }
}

#[derive(Clone)]
pub struct Editor {
    pub widget: gtk::Box,
    data: Rc<RefCell<BTreeMap<String, Profile>>>,
    selected: ProfileList,
    busy: Rc<Cell<bool>>,
    previous: Rc<RefCell<Option<glib::GString>>>,
    name: gtk::Entry,
    cwd: gtk::Entry,
    shell: gtk::Entry,
    commands: gtk::TextView,
    autorun: gtk::CheckButton,
    notifications: gtk::ComboBoxText,
}
impl Editor {
    pub fn new(
        profiles: &BTreeMap<String, Profile>,
        parent: &gtk::Dialog,
        default: &gtk::Entry,
    ) -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let heading = gtk::Label::new(Some("Saved projects — select a row to edit"));
        heading.set_xalign(0.0);
        heading.set_line_wrap(true);
        widget.pack_start(&heading, false, false, 0);
        let selected = ProfileList::new();
        let empty = gtk::Label::new(Some(
            "No saved projects yet. Choose a folder below and click Save project.",
        ));
        empty.set_line_wrap(true);
        selected.0.set_placeholder(Some(&empty));
        for name in profiles.keys() {
            selected.append(Some(name), name);
        }
        let saved_scroll =
            gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        saved_scroll.set_min_content_height(130);
        saved_scroll.set_max_content_height(180);
        saved_scroll.add(&selected.0);
        widget.pack_start(&saved_scroll, false, false, 0);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        widget.pack_start(&actions, false, false, 0);
        let name = gtk::Entry::new();
        name.set_placeholder_text(Some("Project name (defaults to folder name)"));
        name.set_max_length(80);
        name.set_widget_name("project-name");
        let cwd = gtk::Entry::new();
        cwd.set_widget_name("project-folder");
        cwd.set_placeholder_text(Some("Project folder (empty uses home)"));
        let shell = gtk::Entry::new();
        shell.set_placeholder_text(Some("Shell executable, e.g. /bin/zsh (optional)"));
        widget.pack_start(&name, false, false, 0);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.pack_start(&cwd, true, true, 0);
        let browse = gtk::Button::with_label("Choose folder…");
        row.pack_end(&browse, false, false, 0);
        widget.pack_start(&row, false, false, 0);
        widget.pack_start(&shell, false, false, 0);
        let instructions = gtk::Label::new(Some("Startup tasks (optional): one per line, Name | shell command.\nExample: Server | npm run dev\nEach task runs in its own tab in the project folder. Leave empty for a normal terminal."));
        instructions.set_xalign(0.0);
        instructions.set_line_wrap(true);
        widget.pack_start(&instructions, false, false, 0);
        let commands = gtk::TextView::new();
        commands.set_widget_name("project-commands");
        commands.set_monospace(true);
        commands.set_wrap_mode(gtk::WrapMode::WordChar);
        let scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        scroll.set_min_content_height(120);
        scroll.add(&commands);
        widget.pack_start(&scroll, true, true, 0);
        let autorun =
            gtk::CheckButton::with_label("Run tasks automatically when I open this project");
        widget.pack_start(&autorun, false, false, 0);
        let notifications = gtk::ComboBoxText::new();
        notifications.append(Some("errors"), "Notify on errors only");
        notifications.append(Some("all"), "Notify on every completion");
        notifications.append(Some("off"), "No desktop notifications");
        notifications.set_active_id(Some("errors"));
        widget.pack_start(&notifications, false, false, 0);
        let p = parent.downgrade();
        let e = cwd.clone();
        browse.connect_clicked(move |_| {
            if let Some(p) = p.upgrade() {
                choose_folder(&p, &e);
            }
        });
        let save = gtk::Button::with_label("Save project");
        let new = gtk::Button::with_label("New project");
        let delete = gtk::Button::with_label("Delete selected project");
        delete.set_sensitive(false);
        delete.set_widget_name("project-remove");
        for button in [&save, &new, &delete] {
            actions.pack_start(button, false, false, 0);
        }
        let status = gtk::Label::new(Some("Choose a folder, then Save project. Apply or closing Settings also saves your form. Discard changes cancels unsaved edits."));
        status.set_line_wrap(true);
        status.set_xalign(0.0);
        widget.pack_start(&status, false, false, 0);
        let busy = Rc::new(Cell::new(false));
        let previous = Rc::new(RefCell::new(None::<glib::GString>));
        let editor = Self {
            selected: selected.clone(),
            busy: busy.clone(),
            previous: previous.clone(),
            widget,
            data: Rc::new(RefCell::new(profiles.clone())),
            name,
            cwd,
            shell,
            commands,
            autorun,
            notifications,
        };
        // Keep signal captures free of the parent widget to avoid reference cycles.
        let form = editor.form();
        let guard = busy.clone();
        let last = previous.clone();
        let msg = status.clone();
        let delete_button = delete.clone();
        selected.connect_changed(move |combo| {
            delete_button.set_sensitive(combo.active_id().is_some());
            if guard.get() {
                return;
            }
            let next = combo.active_id();
            if let Err(error) = form.commit() {
                msg.set_text(&error);
                guard.set(true);
                combo.set_active_id(last.borrow().as_deref());
                guard.set(false);
                return;
            }
            if let Some(id) = &next {
                let profile = form.data.borrow().get(id.as_str()).cloned();
                if let Some(profile) = profile {
                    form.load(id, &profile);
                }
            }
            *last.borrow_mut() = next;
        });
        let p = parent.downgrade();
        save.connect_clicked(move |_| {
            if let Some(p) = p.upgrade() {
                p.response(gtk::ResponseType::Apply);
            }
        });
        let form = editor.form();
        let combo = selected.clone();
        let guard = busy.clone();
        let last = previous.clone();
        let msg = status.clone();
        new.connect_clicked(move |_| {
            if let Err(error) = form.commit() {
                msg.set_text(&error);
                return;
            }
            guard.set(true);
            combo.remove_all();
            for name in form.data.borrow().keys() {
                combo.append(Some(name), name);
            }
            *last.borrow_mut() = None;
            form.load("", &Profile::default());
            guard.set(false);
            msg.set_text("Enter another project. Apply or Save project saves all your changes.");
        });
        let form = editor.form();
        let default = default.clone();
        delete.connect_clicked(move |_| {
            let key = selected.active_id().map(|s| s.to_string()).unwrap_or_else(|| form.name.text().trim().to_owned());
            form.data.borrow_mut().remove(&key);
            if default.text() == key { default.set_text(""); }
            busy.set(true);
            selected.remove_all();
            for name in form.data.borrow().keys() { selected.append(Some(name), name); }
            *previous.borrow_mut() = None;
            form.load("", &Profile::default());
            busy.set(false);
            status.set_text("Profile removed from the form. Save changes to confirm; running sessions are unaffected.");
        });
        editor
    }
    fn form(&self) -> ProjectForm {
        ProjectForm {
            data: self.data.clone(),
            name: self.name.clone(),
            cwd: self.cwd.clone(),
            shell: self.shell.clone(),
            commands: self.commands.clone(),
            autorun: self.autorun.clone(),
            notifications: self.notifications.clone(),
        }
    }
    pub fn value(&self) -> Result<serde_json::Value, String> {
        self.form().commit()?;
        self.busy.set(true);
        self.selected.remove_all();
        for name in self.data.borrow().keys() {
            self.selected.append(Some(name), name);
        }
        let name = self.name.text();
        if !name.is_empty() {
            self.selected.set_active_id(Some(&name));
        }
        *self.previous.borrow_mut() = self.selected.active_id();
        self.busy.set(false);
        serde_json::to_value(&*self.data.borrow()).map_err(|e| e.to_string())
    }
}
#[derive(Clone)]
struct ProjectForm {
    data: Rc<RefCell<BTreeMap<String, Profile>>>,
    name: gtk::Entry,
    cwd: gtk::Entry,
    shell: gtk::Entry,
    commands: gtk::TextView,
    autorun: gtk::CheckButton,
    notifications: gtk::ComboBoxText,
}
impl ProjectForm {
    fn load(&self, name: &str, profile: &Profile) {
        self.name.set_text(name);
        self.cwd.set_text(&profile.cwd);
        self.shell.set_text(&profile.shell);
        self.commands.buffer().unwrap().set_text(
            &profile
                .tasks
                .iter()
                .map(|t| format!("{} | {}", t.name, t.command))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        self.autorun.set_active(profile.autorun);
        self.notifications
            .set_active_id(Some(if profile.notifications.is_empty() {
                "errors"
            } else {
                &profile.notifications
            }));
    }
    fn commit(&self) -> Result<(), String> {
        let cwd = self.cwd.text().to_string();
        let shell = self.shell.text().to_string();
        let buffer = self.commands.buffer().unwrap();
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .unwrap_or_default();
        let mut key = self.name.text().trim().to_owned();
        if key.is_empty()
            && cwd.is_empty()
            && shell.is_empty()
            && text.trim().is_empty()
            && !self.autorun.is_active()
        {
            return Ok(());
        }
        if key.is_empty() {
            key = Path::new(&cwd)
                .file_name()
                .map(|s| s.to_string_lossy().chars().take(80).collect())
                .unwrap_or_default();
            if key.is_empty() {
                return Err("Enter a project name or choose its folder.".into());
            }
            if self.data.borrow().contains_key(&key) {
                return Err("A project with this folder name already exists. Enter a different project name.".into());
            }
        }
        if !cwd.is_empty() && (!Path::new(&cwd).is_absolute() || !Path::new(&cwd).is_dir()) {
            return Err("Select an existing project folder using an absolute path.".into());
        }
        if !shell.is_empty()
            && (!Path::new(&shell).is_absolute() || crate::terminal::user_shell(&shell) != shell)
        {
            return Err(
                "Select an executable shell using an absolute path, or leave empty.".into(),
            );
        }
        let mut tasks = Vec::new();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let Some((name, command)) = line.split_once('|') else {
                return Err("Each task needs Name | command (one per line).".into());
            };
            tasks.push(ProjectTask {
                name: name.trim().into(),
                command: command.trim().into(),
            });
        }
        let profile = Profile {
            cwd,
            shell,
            tasks,
            autorun: self.autorun.is_active(),
            notifications: self
                .notifications
                .active_id()
                .unwrap_or_else(|| "errors".into())
                .to_string(),
        };
        let mut check = Settings::default();
        check.profiles.insert(key.clone(), profile.clone());
        check.validate()?;
        self.data.borrow_mut().insert(key.clone(), profile);
        self.name.set_text(&key);
        Ok(())
    }
}
impl State {
    pub fn project_settings(&self) -> Settings {
        let mut settings = self.settings.clone();
        let profile = &self.layout.workspaces[self.layout.selected].profile;
        if !profile.is_empty() && settings.profiles.contains_key(profile) {
            settings.default_profile = profile.clone();
        }
        settings
    }
    pub fn open_project(&mut self, name: &str) {
        self.open_project_mode(name, false);
    }
    fn open_project_mode(&mut self, name: &str, explicit_run: bool) {
        let Some(profile) = self.settings.profiles.get(name).cloned() else {
            return;
        };
        if let Some(index) = self
            .layout
            .workspaces
            .iter()
            .position(|w| w.profile == name)
        {
            self.layout.selected = index;
            self.render();
            if profile.autorun || explicit_run {
                self.run_project_tasks(explicit_run);
            }
            return;
        }
        if self.layout.workspaces.len() >= 50 {
            self.error("Maximum 50 workspaces");
            return;
        }
        let cwd = if profile.cwd.is_empty() {
            home()
        } else {
            profile.cwd
        };
        if !Path::new(&cwd).is_dir() {
            self.error("Project folder is unavailable. Update it in Settings → Profiles.");
            return;
        }
        let mut workspace = Workspace::new(name);
        workspace.profile = name.to_owned();
        workspace.tabs = vec![Tab::new("main", cwd)];
        self.layout.workspaces.push(workspace);
        self.layout.selected = self.layout.workspaces.len() - 1;
        self.render();
        if profile.autorun || explicit_run {
            self.run_project_tasks(explicit_run);
        }
    }
    pub fn projects(&self) {
        let dialog = gtk::Dialog::with_buttons(
            Some("Open project"),
            Some(&self.window),
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            &[
                ("Manage profiles", gtk::ResponseType::Other(1)),
                ("Open + run tasks", gtk::ResponseType::Other(2)),
                ("Cancel", gtk::ResponseType::Cancel),
                ("Open project", gtk::ResponseType::Accept),
            ],
        );
        dialog.set_widget_name("project-manager");
        dialog.set_default_size(720, 440);
        let list = gtk::ListBox::new();
        let names: Vec<_> = self.settings.profiles.keys().cloned().collect();
        if names.is_empty() {
            let label = gtk::Label::new(Some(
                "Create a profile with a folder and shell to open your project in one click.",
            ));
            label.set_line_wrap(true);
            dialog.content_area().pack_start(&label, false, false, 16);
        }
        for (name, profile) in &self.settings.profiles {
            let row = gtk::ListBoxRow::new();
            let label = gtk::Label::new(Some(&format!(
                "{name}\n{}\n{}\n{} tasks · {}",
                if profile.cwd.is_empty() {
                    "Home directory"
                } else {
                    &profile.cwd
                },
                if profile.shell.is_empty() {
                    "Default shell"
                } else {
                    &profile.shell
                },
                profile.tasks.len(),
                if profile.autorun {
                    "automatic launch"
                } else {
                    "manual launch"
                }
            )));
            label.set_xalign(0.0);
            label.set_margin_start(12);
            label.set_margin_top(8);
            label.set_margin_bottom(8);
            let contents = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            label.set_ellipsize(pango::EllipsizeMode::Middle);
            contents.pack_start(&label, true, true, 0);
            let edit = gtk::Button::with_label("Edit");
            edit.set_widget_name(&format!("edit-project-{name}"));
            let delete = gtk::Button::with_label("Delete");
            delete.set_widget_name(&format!("delete-project-{name}"));
            edit.set_valign(gtk::Align::Center);
            delete.set_valign(gtk::Align::Center);
            contents.pack_start(&edit, false, false, 0);
            contents.pack_start(&delete, false, false, 8);
            let weak = self.weak.clone();
            let key = name.clone();
            let d = dialog.downgrade();
            edit.connect_clicked(move |_| {
                if let Some(d) = d.upgrade() {
                    unsafe {
                        d.destroy();
                    }
                }
                let key = key.clone();
                enqueue(&weak, move |s| {
                    crate::preferences::show_page(s, 4);
                    if let Some(dialog) = &s.preferences {
                        select_project(dialog.upcast_ref(), &key);
                    }
                });
            });
            // An open settings draft must be saved/discarded before editing the on-disk list.
            delete.set_sensitive(self.preferences.is_none());
            delete.set_tooltip_text(Some(if self.preferences.is_some() {
                "Save or close Settings first."
            } else {
                "Delete only the profile. Files and running terminals are kept."
            }));
            let weak = self.weak.clone();
            let key = name.clone();
            let d = dialog.downgrade();
            delete.connect_clicked(move |_| {
                let Some(parent) = d.upgrade() else {
                    return;
                };
                let confirm = gtk::MessageDialog::new(
                    Some(&parent),
                    gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
                    gtk::MessageType::Question,
                    gtk::ButtonsType::None,
                    &format!("Delete saved project “{key}”?"),
                );
                confirm.set_widget_name("project-delete-confirmation");
                confirm.set_secondary_text(Some(
                    "Only the profile will be deleted. Its folder, files, and running terminals are kept.",
                ));
                confirm.add_buttons(&[
                    ("Cancel", gtk::ResponseType::Cancel),
                    ("Delete", gtk::ResponseType::Accept),
                ]);
                confirm.set_default_response(gtk::ResponseType::Cancel);
                let weak = weak.clone();
                let key = key.clone();
                let parent = parent.downgrade();
                confirm.connect_response(move |c, response| {
                    unsafe {
                        c.destroy();
                    }
                    if response == gtk::ResponseType::Accept {
                        let key = key.clone();
                        let parent = parent.clone();
                        enqueue(&weak, move |s| {
                            if let Err(error) = s.delete_project(&key) {
                                s.error(&error);
                                return;
                            }
                            if let Some(parent) = parent.upgrade() {
                                unsafe {
                                    parent.destroy();
                                }
                            }
                            s.projects();
                        });
                    }
                });
                confirm.show_all();
            });
            row.add(&contents);
            list.add(&row);
        }
        let scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        scroll.add(&list);
        dialog.content_area().pack_start(&scroll, true, true, 8);
        let d = dialog.downgrade();
        list.connect_row_activated(move |_, _| {
            if let Some(d) = d.upgrade() {
                d.response(gtk::ResponseType::Accept);
            }
        });
        if let Some(row) = list.row_at_index(0) {
            list.select_row(Some(&row));
        }
        dialog.set_response_sensitive(gtk::ResponseType::Accept, !names.is_empty());
        dialog.set_response_sensitive(gtk::ResponseType::Other(2), !names.is_empty());
        let weak = self.weak.clone();
        dialog.connect_response(move |d, r| {
            let name = list
                .selected_row()
                .and_then(|row| names.get(row.index() as usize).cloned());
            unsafe {
                d.destroy();
            }
            if r == gtk::ResponseType::Accept || r == gtk::ResponseType::Other(2) {
                if let Some(name) = name {
                    enqueue(&weak, move |s| {
                        if r == gtk::ResponseType::Other(2) {
                            s.open_project_mode(&name, true);
                        } else {
                            s.open_project(&name);
                        }
                    });
                }
            } else if r == gtk::ResponseType::Other(1) {
                enqueue(&weak, |s| crate::preferences::show_page(s, 4));
            }
        });
        dialog.show_all();
    }
}

impl State {
    pub fn run_project_tasks(&mut self, explicit: bool) {
        let workspace = &self.layout.workspaces[self.layout.selected];
        let wid = workspace.id;
        let Some(profile) = self.settings.profiles.get(&workspace.profile).cloned() else {
            return;
        };
        if self
            .project_tasks
            .values()
            .any(|t| t.workspace == wid && (!explicit || t.running))
        {
            if explicit {
                self.error("Project tasks are still running. Stop them with Ctrl+C or close their tabs before running again.");
            }
            return;
        }
        if profile.tasks.is_empty() {
            return;
        }
        if workspace.tabs.len() + profile.tasks.len() > 50 {
            self.error(
                "Not enough space for project tasks (maximum 50 tabs). Close old task tabs first.",
            );
            return;
        }
        let cwd = if profile.cwd.is_empty() {
            home()
        } else {
            profile.cwd.clone()
        };
        if !Path::new(&cwd).is_dir() {
            self.error("Project folder is unavailable. Update the profile before running tasks.");
            return;
        }
        let selected = workspace.tabs.len();
        for task in profile.tasks {
            let tab = Tab::new(&format!("▶ {}", task.name), cwd.clone());
            self.project_tasks.insert(
                tab.active,
                RunningTask {
                    workspace: wid,
                    tab: tab.id,
                    name: task.name,
                    command: task.command,
                    notifications: profile.notifications.clone(),
                    running: true,
                },
            );
            let workspace = &mut self.layout.workspaces[self.layout.selected];
            workspace.tabs.push(tab);
            workspace.selected = workspace.tabs.len() - 1;
            // Materialize every task immediately, including background tabs.
            self.render();
        }
        self.layout.workspaces[self.layout.selected].selected = selected;
        self.render();
    }
    pub fn task_finished(&mut self, pane: Id, result: String) {
        let Some(task) = self.project_tasks.get_mut(&pane) else {
            return;
        };
        if !task.running {
            return;
        }
        task.running = false;
        let success = result == "Done";
        if let Some(tab) = self.layout.find_tab_mut(task.tab) {
            tab.name = format!(
                "{} {} — {}",
                if success { "✓" } else { "✗" },
                task.name,
                result
            );
            if task.notifications == "all" || (!success && task.notifications != "off") {
                if let Some(app) = self.window.application() {
                    let notification = gio::Notification::new(&format!("TermLook: {}", task.name));
                    notification.set_body(Some(&format!(
                        "{result}. Open the project tab to view logs."
                    )));
                    if !success {
                        notification.set_priority(gio::NotificationPriority::High);
                    }
                    app.send_notification(Some(&format!("task-{pane}")), &notification);
                }
            }
        }
        self.render();
    }
}

fn select_project(widget: &gtk::Widget, name: &str) {
    if widget.widget_name() == "project-selector" {
        if let Ok(list) = widget.clone().downcast::<gtk::ListBox>() {
            ProfileList(list).set_active_id(Some(name));
        }
        return;
    }
    if let Ok(container) = widget.clone().downcast::<gtk::Container>() {
        for child in container.children() {
            select_project(&child, name);
        }
    }
}
impl State {
    pub fn delete_project(&mut self, name: &str) -> Result<(), String> {
        let mut settings = self.settings.clone();
        settings.profiles.remove(name);
        if settings.default_profile == name {
            settings.default_profile.clear();
        }
        self.apply_settings(settings)
    }
}
