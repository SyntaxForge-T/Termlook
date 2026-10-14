use crate::{background::Backdrop, closing::Target, terminal::Session, tray::Tray};
use gtk::prelude::*;
use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    rc::{Rc, Weak},
    time::Duration,
};
use termlook::{
    model::{Axis, Id, Layout, Tab, Tree, Workspace},
    settings::{shortcut_label, Settings, ACTIONS},
    storage,
};
pub type Shared = Rc<RefCell<State>>;
pub type WeakState = Weak<RefCell<State>>;
struct Grid {
    root: gtk::Box,
    tree: Tree,
    zoom: Option<Id>,
    generation: Id,
}
pub struct State {
    pub window: gtk::ApplicationWindow,
    pub settings: Settings,
    pub weak: WeakState,
    pub layout: Layout,
    pub sessions: HashMap<Id, Session>,
    pub project_tasks: HashMap<Id, crate::projects::RunningTask>,
    sidebar: gtk::Box,
    spaces: gtk::Box,
    tabs: gtk::Box,
    stack: gtk::Stack,
    backdrop: Backdrop,
    pub confirmation: Option<gtk::MessageDialog>,
    status: gtk::Label,
    grids: HashMap<Id, Grid>,
    space_buttons: HashMap<Id, gtk::Box>,
    tab_buttons: HashMap<Id, gtk::Box>,
    nav_signature: String,
    tab_signature: String,
    css: gtk::CssProvider,
    dir: PathBuf,
    saved: Option<serde_json::Value>,
    save_timer: Option<glib::SourceId>,
    tray: Option<Tray>,
    closed: bool,
    pub preferences: Option<gtk::Dialog>,
    hints: Vec<(gtk::Button, &'static str, &'static str)>,
}
pub fn enqueue(weak: &WeakState, f: impl FnOnce(&mut State) + 'static) {
    let weak = weak.clone();
    glib::idle_add_local_once(move || {
        if let Some(state) = weak.upgrade() {
            let mut state = state.borrow_mut();
            if !state.closed {
                f(&mut state);
            }
        }
    });
}
fn button(
    label: &str,
    tip: &str,
    weak: &WeakState,
    f: impl Fn(&mut State) + 'static,
) -> gtk::Button {
    let b = gtk::Button::with_label(label);
    b.set_tooltip_text(Some(tip));
    b.set_can_focus(false);
    let weak = weak.clone();
    let f = Rc::new(f);
    b.connect_clicked(move |_| {
        let f = f.clone();
        enqueue(&weak, move |s| f(s));
    });
    b
}
fn action_button(label: &str, tip: &str, weak: &WeakState, action: &'static str) -> gtk::Button {
    button(label, tip, weak, move |s| s.action(action))
}
/// An action button whose tooltip also lists the action's current shortcut.
fn tool_button(
    state: &Shared,
    label: &str,
    title: &'static str,
    action: &'static str,
    weak: &WeakState,
) -> gtk::Button {
    let b = action_button(label, title, weak, action);
    b.style_context().add_class("tool-button");
    state.borrow_mut().hints.push((b.clone(), title, action));
    b
}
fn detach(widget: &gtk::Widget) {
    if let Some(parent) = widget
        .parent()
        .and_then(|p| p.downcast::<gtk::Container>().ok())
    {
        parent.remove(widget);
    }
}
fn clear(container: &impl IsA<gtk::Container>) {
    for child in container.children() {
        unsafe {
            child.destroy();
        }
    }
}
fn class(widget: &impl IsA<gtk::Widget>, name: &str, enabled: bool) {
    if enabled {
        widget.style_context().add_class(name);
    } else {
        widget.style_context().remove_class(name);
    }
}
impl State {
    pub fn new(app: &gtk::Application, dir: PathBuf) -> Shared {
        let mut settings = storage::load_settings(&dir.join("settings.json"));
        if let Err(error) = validate_shortcuts(&settings) {
            eprintln!("Ignoring invalid keybindings: {error}");
            settings.keybindings = Settings::default().keybindings;
        }
        let image = match crate::background::load(&settings) {
            Ok(image) => image,
            Err(e) => {
                eprintln!("{e}");
                settings.background_image.clear();
                None
            }
        };
        let layout = storage::load_layout(&dir.join("layout.json"));
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title("TermLook")
            .default_width(1120)
            .default_height(740)
            .build();
        window.style_context().add_class("main-window");
        let loader = gtk::gdk_pixbuf::PixbufLoader::new();
        if loader
            .write(include_bytes!("../assets/io.termlook.TermLook.svg"))
            .is_ok()
            && loader.close().is_ok()
        {
            window.set_icon(loader.pixbuf().as_ref());
        }
        let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 10);
        sidebar.set_widget_name("sidebar");
        let spaces = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        let stack = gtk::Stack::new();
        let backdrop = Backdrop::new(&stack, &settings, image);
        let status = gtk::Label::new(None);
        status.set_widget_name("status");
        status.set_xalign(0.0);
        status.set_no_show_all(true);
        let state = Rc::new_cyclic(|weak| {
            RefCell::new(Self {
                window: window.clone(),
                settings,
                weak: weak.clone(),
                layout,
                sessions: HashMap::new(),
                project_tasks: HashMap::new(),
                sidebar: sidebar.clone(),
                spaces,
                tabs: tabs.clone(),
                stack: stack.clone(),
                backdrop,
                confirmation: None,
                status: status.clone(),
                grids: HashMap::new(),
                space_buttons: HashMap::new(),
                tab_buttons: HashMap::new(),
                nav_signature: String::new(),
                tab_signature: String::new(),
                css: gtk::CssProvider::new(),
                dir,
                saved: None,
                save_timer: None,
                tray: None,
                closed: false,
                preferences: None,
                hints: Vec::new(),
            })
        });
        let weak = Rc::downgrade(&state);
        let title = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        window.set_titlebar(Some(&title));
        let heading = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        heading.set_widget_name("heading");
        let brand = gtk::Label::new(Some("WORKSPACES"));
        brand.set_widget_name("brand");
        brand.set_xalign(0.0);
        brand.set_margin_start(12);
        heading.pack_start(&brand, true, true, 0);
        heading.pack_end(
            &tool_button(&state, "+", "New workspace", "new-workspace", &weak),
            false,
            false,
            0,
        );
        title.pack_start(&heading, false, false, 0);
        let group = gtk::SizeGroup::new(gtk::SizeGroupMode::Horizontal);
        group.add_widget(&heading);
        group.add_widget(&sidebar);
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        bar.set_widget_name("tabbar");
        title.pack_start(&bar, true, true, 0);
        let scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
        scroll.set_min_content_height(38);
        scroll.add(&tabs);
        bar.pack_start(&scroll, true, true, 0);
        for (label, tip, action) in [
            ("◫", "Split left / right", "split-horizontal"),
            ("⬒", "Split top / bottom", "split-vertical"),
            ("⊞", "Four panes", "grid"),
            ("□", "Focus pane / show all", "zoom"),
            ("", "", ""),
            ("+", "New tab", "new-tab"),
            ("", "", ""),
            ("×", "Close / hide window", "close-window"),
        ] {
            if action.is_empty() {
                let separator = gtk::Separator::new(gtk::Orientation::Vertical);
                separator.set_margin_top(8);
                separator.set_margin_bottom(8);
                separator.set_margin_start(4);
                separator.set_margin_end(4);
                bar.pack_start(&separator, false, false, 0);
            } else {
                bar.pack_start(
                    &tool_button(&state, label, tip, action, &weak),
                    false,
                    false,
                    0,
                );
            }
        }
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        window.add(&root);
        root.pack_start(&sidebar, false, false, 0);
        let scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.add(&state.borrow().spaces);
        let projects_button = tool_button(&state, "Projects…", "Open project", "projects", &weak);
        sidebar.pack_start(&projects_button, false, false, 0);
        sidebar.pack_start(&scroll, true, true, 0);
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        footer.set_homogeneous(true);
        for (label, tip, action) in [
            ("⚙  Settings", "Settings", "settings"),
            ("?  Help", "Help and shortcuts", "help"),
        ] {
            let b = tool_button(&state, label, tip, action, &weak);
            b.style_context().add_class("footer-button");
            footer.pack_start(&b, true, true, 0);
        }
        sidebar.pack_end(&footer, false, false, 0);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.set_widget_name("surface");
        root.pack_start(&body, true, true, 0);
        body.pack_start(&state.borrow().backdrop.widget, true, true, 0);
        body.pack_end(&status, false, false, 0);
        let w = weak.clone();
        window.connect_delete_event(move |_, _| {
            enqueue(&w, |s| s.action("close-window"));
            glib::Propagation::Stop
        });
        let w = weak.clone();
        window.connect_key_press_event(move |_, event| {
            let Some(state) = w.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let state = state.borrow();
            for (action, _, _) in ACTIONS {
                let binding = state.settings.binding(action);
                if binding.is_empty() {
                    continue;
                }
                let (key, mods) = gtk::accelerator_parse(binding);
                let actual = event.state() & gtk::accelerator_get_default_mod_mask();
                if normalize_key(*event.keyval()) == normalize_key(key) && actual == mods {
                    let action = action.to_string();
                    enqueue(&w, move |s| s.action(&action));
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        let w = weak.clone();
        window.connect_destroy(move |_| {
            if let Some(s) = w.upgrade() {
                if let Ok(mut s) = s.try_borrow_mut() {
                    s.shutdown();
                }
            }
        });
        let menu = gtk::Menu::new();
        for (label, action) in [
            ("Open TermLook", "show"),
            ("Settings", "settings"),
            ("Quit", "quit"),
        ] {
            let item = gtk::MenuItem::with_label(label);
            let w = weak.clone();
            item.connect_activate(move |_| enqueue(&w, move |s| s.action(action)));
            menu.append(&item);
        }
        menu.show_all();
        let w = weak.clone();
        state.borrow_mut().tray = Some(Tray::new(menu, move || {
            enqueue(&w, |s| {
                if !s.window.is_visible() {
                    s.restore();
                }
            })
        }));
        let mut s = state.borrow_mut();
        gtk::StyleContext::add_provider_for_screen(
            &gdk::Screen::default().unwrap(),
            &s.css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        s.style();
        s.refresh_hints();
        s.render();
        window.show_all();
        drop(s);
        state
    }
    /// Show each toolbar action's current shortcut in its tooltip.
    fn refresh_hints(&self) {
        for (button, title, action) in &self.hints {
            let shortcut = shortcut_label(self.settings.binding(action));
            if shortcut.is_empty() {
                button.set_tooltip_text(Some(*title));
            } else {
                button.set_tooltip_text(Some(&format!("{title}  ·  {shortcut}")));
            }
        }
    }
    fn style(&self) {
        let css = format!(
            "{}\n#surface, #tabbar {{ background: {}; }}",
            include_str!("../assets/style.css"),
            self.settings.background
        );
        if let Err(error) = self.css.load_from_data(css.as_bytes()) {
            eprintln!("Theme: {error}");
        }
        let path = self.dir.join("theme.css");
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(custom) => {
                    let combined = format!("{css}\n{custom}");
                    let candidate = gtk::CssProvider::new();
                    match candidate.load_from_data(combined.as_bytes()) {
                        Ok(()) => {
                            let _ = self.css.load_from_data(combined.as_bytes());
                        }
                        Err(e) => self.error(&format!("theme.css: {e}")),
                    }
                }
                Err(e) => self.error(&format!("theme.css: {e}")),
            }
        }
        self.sidebar
            .set_size_request(self.settings.sidebar_width, -1);
    }
    pub fn restore(&self) {
        self.window.show();
        self.window.deiconify();
        self.window.present();
    }
    pub fn error(&self, text: &str) {
        self.status.set_text(text);
        self.status.show();
        eprintln!("{text}");
    }
    fn schedule_save(&mut self) {
        if self.save_timer.is_none() {
            let weak = self.weak.clone();
            self.save_timer = Some(glib::timeout_add_local_once(
                Duration::from_millis(250),
                move || {
                    if let Some(s) = weak.upgrade() {
                        let mut s = s.borrow_mut();
                        s.save_timer = None;
                        s.persist();
                    }
                },
            ));
        }
    }
    fn persist(&mut self) {
        let data = self.layout.to_data();
        if self.saved.as_ref() == Some(&data) {
            return;
        }
        match storage::save(&self.dir.join("layout.json"), &data) {
            Ok(()) => {
                self.saved = Some(data);
                self.status.hide();
            }
            Err(e) => self.error(&format!("Could not save layout: {e}")),
        }
    }
    pub fn apply_settings(&mut self, settings: Settings) -> Result<(), String> {
        settings.validate()?;
        validate_shortcuts(&settings)?;
        let image = crate::background::load(&settings)?;
        storage::save(
            &self.dir.join("settings.json"),
            &serde_json::to_value(&settings).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        self.backdrop.apply(&settings, image);
        self.settings = settings;
        self.style();
        self.refresh_hints();
        for session in self.sessions.values() {
            session.apply(&self.settings);
        }
        self.tab_signature.clear();
        self.render();
        Ok(())
    }
    pub fn action(&mut self, action: &str) {
        match action {
            "projects" => {
                self.projects();
                return;
            }
            "new-tab" => {
                if self.layout.workspaces[self.layout.selected].tabs.len() >= 50 {
                    self.error("Maximum 50 tabs per workspace");
                    return;
                }
                let effective = self.project_settings();
                let cwd = effective
                    .profile()
                    .filter(|p| !p.cwd.is_empty())
                    .map(|p| p.cwd.clone())
                    .unwrap_or_else(|| self.layout.tab().active_pane().cwd.clone());
                let w = &mut self.layout.workspaces[self.layout.selected];
                let mut n = 1;
                while w.tabs.iter().any(|t| t.name == format!("terminal {n}")) {
                    n += 1;
                }
                w.tabs.push(Tab::new(&format!("terminal {n}"), cwd));
                w.selected = w.tabs.len() - 1;
            }
            "new-workspace" => {
                self.ask("New workspace", "project", |s, name| {
                    if s.layout.workspaces.len() < 50 {
                        s.layout.workspaces.push(Workspace::new(&name));
                        s.layout.selected = s.layout.workspaces.len() - 1;
                        s.render();
                    }
                });
                return;
            }
            "rename-tab" => {
                let id = self.layout.tab().id;
                self.rename_tab(id);
                return;
            }
            "close-tab" => {
                let id = self.layout.tab().id;
                self.request_close(Target::Tab(id));
                return;
            }
            "split-horizontal" => self.layout.tab_mut().split(Axis::Horizontal),
            "split-vertical" => self.layout.tab_mut().split(Axis::Vertical),
            "grid" => self.layout.tab_mut().grid(),
            "close-pane" => {
                let t = self.layout.tab();
                self.request_close(Target::Pane {
                    tab: t.id,
                    pane: t.active,
                });
                return;
            }
            "zoom" => {
                let t = self.layout.tab_mut();
                if t.panes.len() > 1 {
                    t.zoomed = !t.zoomed;
                }
            }
            "next-tab" | "previous-tab" => {
                let w = &mut self.layout.workspaces[self.layout.selected];
                w.selected = (w.selected
                    + if action == "next-tab" {
                        1
                    } else {
                        w.tabs.len() - 1
                    })
                    % w.tabs.len();
            }
            "copy" | "paste" => {
                if let Some(s) = self.sessions.get(&self.layout.tab().active) {
                    if action == "copy" {
                        s.copy();
                    } else {
                        s.paste();
                    }
                }
                return;
            }
            "settings" => {
                crate::preferences::show(self);
                return;
            }
            "reload-settings" => {
                match storage::read(&self.dir.join("settings.json"))
                    .map_err(|e| e.to_string())
                    .and_then(|v| serde_json::from_value::<Settings>(v).map_err(|e| e.to_string()))
                    .and_then(|s| self.apply_settings(s))
                {
                    Ok(()) => {}
                    Err(e) => self.error(&e),
                }
                return;
            }
            "help" => {
                self.help();
                return;
            }
            "show" => {
                self.restore();
                return;
            }
            "close-window" if self.settings.close_to_tray => {
                self.persist();
                if self.tray.as_ref().is_some_and(|t| t.available.get()) {
                    self.window.hide();
                } else {
                    self.window.iconify();
                }
                return;
            }
            "quit" | "close-window" => {
                self.request_close(Target::Quit);
                return;
            }
            _ => return,
        }
        self.render();
    }
    fn ask(&self, title: &str, initial: &str, f: impl Fn(&mut State, String) + 'static) {
        let dialog = gtk::Dialog::with_buttons(
            Some(title),
            Some(&self.window),
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            &[
                ("Cancel", gtk::ResponseType::Cancel),
                ("Save", gtk::ResponseType::Accept),
            ],
        );
        let entry = gtk::Entry::new();
        entry.set_text(initial);
        entry.set_max_length(80);
        entry.set_activates_default(true);
        entry.set_margin_top(12);
        entry.set_margin_bottom(12);
        entry.set_margin_start(12);
        entry.set_margin_end(12);
        dialog.content_area().add(&entry);
        dialog.set_default_response(gtk::ResponseType::Accept);
        let weak = self.weak.clone();
        let f = Rc::new(f);
        dialog.connect_response(move |d, response| {
            let name = entry.text().trim().to_owned();
            unsafe {
                d.destroy();
            }
            if response == gtk::ResponseType::Accept && !name.is_empty() {
                let f = f.clone();
                enqueue(&weak, move |s| f(s, name));
            }
        });
        dialog.show_all();
    }
    fn rename_tab(&self, id: Id) {
        if let Some(tab) = self
            .layout
            .workspaces
            .iter()
            .flat_map(|w| &w.tabs)
            .find(|t| t.id == id)
        {
            self.ask("Tab name", &tab.name, move |s, name| {
                if let Some(t) = s.layout.find_tab_mut(id) {
                    t.name = name;
                    s.render();
                }
            });
        }
    }
    fn rename_workspace(&self, id: Id) {
        if let Some(w) = self.layout.workspaces.iter().find(|w| w.id == id) {
            self.ask("Workspace name", &w.name, move |s, name| {
                if let Some(w) = s.layout.workspaces.iter_mut().find(|w| w.id == id) {
                    w.name = name;
                    s.render();
                }
            });
        }
    }
    fn title_button(&self, name: &str, id: Id, workspace: bool) -> gtk::Button {
        let b = button(name, "Double-click to rename", &self.weak, move |s| {
            if workspace {
                if let Some(i) = s.layout.workspaces.iter().position(|w| w.id == id) {
                    s.layout.selected = i;
                }
            } else {
                let w = &mut s.layout.workspaces[s.layout.selected];
                if let Some(i) = w.tabs.iter().position(|t| t.id == id) {
                    w.selected = i;
                }
            }
            s.render();
        });
        if let Some(label) = b.child().and_then(|c| c.downcast::<gtk::Label>().ok()) {
            label.set_ellipsize(pango::EllipsizeMode::End);
            label.set_max_width_chars(20);
            if !workspace {
                label.set_width_chars(12);
            }
            label.set_xalign(0.0);
        }
        let weak = self.weak.clone();
        b.connect_button_press_event(move |_, e| {
            if e.button() == 1 && e.event_type() == gdk::EventType::DoubleButtonPress {
                enqueue(&weak, move |s| {
                    if workspace {
                        s.rename_workspace(id);
                    } else {
                        s.rename_tab(id);
                    }
                });
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        b
    }
    fn navigation(&mut self) {
        let signature = format!(
            "{:?}",
            self.layout
                .workspaces
                .iter()
                .map(|w| (w.id, &w.name))
                .collect::<Vec<_>>()
        );
        if signature != self.nav_signature {
            clear(&self.spaces);
            self.space_buttons.clear();
            for w in &self.layout.workspaces {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                row.style_context().add_class("workspace");
                let id = w.id;
                row.pack_start(&self.title_button(&w.name, id, true), true, true, 0);
                row.pack_end(
                    &button(
                        "×",
                        "Delete workspace and end its sessions",
                        &self.weak,
                        move |s| {
                            s.request_close(Target::Workspace(id));
                        },
                    ),
                    false,
                    false,
                    0,
                );
                self.spaces.pack_start(&row, false, false, 0);
                self.space_buttons.insert(id, row);
            }
            self.spaces.show_all();
            self.nav_signature = signature;
        }
        let w = &self.layout.workspaces[self.layout.selected];
        let signature = format!(
            "{}:{:?}",
            w.id,
            w.tabs.iter().map(|t| (t.id, &t.name)).collect::<Vec<_>>()
        );
        if signature != self.tab_signature {
            clear(&self.tabs);
            self.tab_buttons.clear();
            for t in &w.tabs {
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                row.style_context().add_class("terminal-tab");
                let id = t.id;
                row.pack_start(&self.title_button(&t.name, id, false), false, false, 0);
                if self.settings.show_tab_close_buttons {
                    row.pack_end(
                        &button("×", "Close tab", &self.weak, move |s| {
                            s.request_close(Target::Tab(id));
                        }),
                        false,
                        false,
                        0,
                    );
                }
                self.tabs.pack_start(&row, false, false, 0);
                self.tab_buttons.insert(id, row);
            }
            self.tabs.show_all();
            self.tab_signature = signature;
        }
        for (id, row) in &self.space_buttons {
            class(row, "active", *id == w.id);
        }
        for t in &w.tabs {
            if let Some(row) = self.tab_buttons.get(&t.id) {
                class(row, "active", t.id == w.tabs[w.selected].id);
                row.set_tooltip_text(Some(&t.active_pane().cwd));
            }
        }
    }
    pub fn render(&mut self) {
        let live = self.layout.live_ids();
        for (id, task) in &mut self.project_tasks {
            if !live.contains(id) {
                task.running = false;
            }
        }
        self.project_tasks.retain(|_, task| {
            self.layout
                .workspaces
                .iter()
                .any(|w| w.id == task.workspace)
        });
        if self.sessions.keys().any(|id| !live.contains(id)) {
            gtk::prelude::GtkWindowExt::set_focus(&self.window, None::<&gtk::Widget>);
        }
        self.sessions.retain(|id, s| {
            if live.contains(id) {
                true
            } else {
                s.close();
                false
            }
        });
        let tabs: Vec<_> = self
            .layout
            .workspaces
            .iter()
            .flat_map(|w| &w.tabs)
            .map(|t| t.id)
            .collect();
        self.grids.retain(|id, grid| {
            if tabs.contains(id) {
                true
            } else {
                unsafe {
                    grid.root.destroy();
                }
                false
            }
        });
        let tab = self.layout.tab().clone();
        let effective = self.project_settings();
        for pane in &tab.panes {
            if !self.sessions.contains_key(&pane.id) {
                let session = Session::new(&effective);
                let weak = self.weak.clone();
                let id = pane.id;
                let tid = tab.id;
                session.widget().connect_focus_in_event(move |_, _| {
                    enqueue(&weak, move |s| {
                        if s.layout.tab().id == tid
                            && s.layout.tab().panes.iter().any(|p| p.id == id)
                        {
                            s.layout.tab_mut().active = id;
                            s.highlight();
                        }
                    });
                    glib::Propagation::Proceed
                });
                let weak = self.weak.clone();
                session.widget().connect_notify_local(
                    Some("current-directory-uri"),
                    move |_, _| {
                        enqueue(&weak, move |s| {
                            let cwd = s.sessions.get(&id).and_then(Session::directory);
                            if let Some(cwd) = cwd {
                                if let Some(t) = s.layout.find_tab_mut(tid) {
                                    if let Some(p) = t.panes.iter_mut().find(|p| p.id == id) {
                                        p.cwd = cwd;
                                        s.schedule_save();
                                    }
                                }
                            }
                        });
                    },
                );
                if let Some(task) = self.project_tasks.get(&pane.id) {
                    let weak = self.weak.clone();
                    let id = pane.id;
                    session.start_task(&pane.cwd, &effective, &task.command, move |result| {
                        enqueue(&weak, move |s| s.task_finished(id, result));
                    });
                } else {
                    session.start(&pane.cwd, &effective);
                }
                self.sessions.insert(pane.id, session);
            }
        }
        let zoom = tab.zoomed.then_some(tab.active);
        let rebuild = self
            .grids
            .get(&tab.id)
            .is_none_or(|g| g.tree != tab.tree || g.zoom != zoom);
        if rebuild {
            for pane in &tab.panes {
                detach(self.sessions[&pane.id].widget());
            }
            if let Some(grid) = self.grids.remove(&tab.id) {
                unsafe {
                    grid.root.destroy();
                }
            }
            let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let tree = zoom.map(Tree::Leaf).unwrap_or_else(|| tab.tree.clone());
            let generation = termlook::model::id();
            let content = self.build_tree(&tab, &tree, vec![], generation);
            root.pack_start(&content, true, true, 0);
            root.show_all();
            self.stack.add_named(&root, &tab.id.to_string());
            self.grids.insert(
                tab.id,
                Grid {
                    root,
                    tree: tab.tree.clone(),
                    zoom,
                    generation,
                },
            );
        }
        self.stack.set_visible_child_name(&tab.id.to_string());
        self.navigation();
        self.highlight();
        if let Some(session) = self.sessions.get(&tab.active) {
            session.widget().grab_focus();
        }
        self.schedule_save();
    }
    fn highlight(&self) {
        for (id, session) in &self.sessions {
            if let Some(parent) = session.widget().parent() {
                class(&parent, "focused", *id == self.layout.tab().active);
            }
        }
    }
    fn build_tree(&self, tab: &Tab, tree: &Tree, path: Vec<bool>, generation: Id) -> gtk::Widget {
        match tree {
            Tree::Leaf(id) => {
                let id = *id;
                let tid = tab.id;
                let cell = gtk::Box::new(gtk::Orientation::Vertical, 0);
                cell.style_context().add_class("terminal-pane");
                let header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                header.style_context().add_class("pane-header");
                let grip = gtk::EventBox::new();
                let label = gtk::Label::new(Some(&format!(
                    "⠿  Pane {}",
                    tab.panes.iter().position(|p| p.id == id).unwrap_or(0) + 1
                )));
                grip.add(&label);
                grip.set_widget_name(&format!("pane-{tid}-{id}"));
                grip.set_tooltip_text(Some("Drag onto another pane to swap"));
                header.pack_start(&grip, true, true, 0);
                header.pack_end(
                    &button("×", "Close pane", &self.weak, move |s| {
                        if s.layout.tab().id == tid {
                            s.request_close(Target::Pane { tab: tid, pane: id });
                        }
                    }),
                    false,
                    false,
                    0,
                );
                cell.pack_start(&header, false, false, 0);
                cell.pack_start(self.sessions[&id].widget(), true, true, 0);
                let targets = [gtk::TargetEntry::new(
                    "application/x-termlook-pane",
                    gtk::TargetFlags::SAME_APP,
                    0,
                )];
                grip.drag_source_set(
                    gdk::ModifierType::BUTTON1_MASK,
                    &targets,
                    gdk::DragAction::MOVE,
                );
                header.drag_dest_set(gtk::DestDefaults::ALL, &targets, gdk::DragAction::MOVE);
                grip.connect_drag_data_get(move |_, _, data, _, _| {
                    data.set_text(&format!("{tid}:{id}"));
                });
                let weak = self.weak.clone();
                header.connect_drag_data_received(move |_, context, _, _, data, _, time| {
                    let source = data.text().and_then(|text| {
                        let (t, p) = text.split_once(':')?;
                        Some((t.parse::<Id>().ok()?, p.parse::<Id>().ok()?))
                    });
                    let valid = source.is_some_and(|(t, p)| t == tid && p != id);
                    if let Some((_, source)) = source.filter(|_| valid) {
                        enqueue(&weak, move |s| {
                            if let Some(t) = s.layout.find_tab_mut(tid) {
                                t.swap(source, id);
                                s.render();
                            }
                        });
                    }
                    context.drag_finish(valid, false, time);
                });
                let c = cell.downgrade();
                grip.connect_drag_begin(move |_, context| {
                    if let Some(c) = c.upgrade() {
                        let width = c.allocated_width().max(1);
                        let height = c.allocated_height().max(1);
                        let scale = (260.0 / f64::from(width))
                            .min(160.0 / f64::from(height))
                            .min(1.0);
                        if let Ok(surface) = gtk::cairo::ImageSurface::create(
                            gtk::cairo::Format::ARgb32,
                            (f64::from(width) * scale).ceil() as i32,
                            (f64::from(height) * scale).ceil() as i32,
                        ) {
                            if let Ok(cr) = gtk::cairo::Context::new(&surface) {
                                cr.scale(scale, scale);
                                c.draw(&cr);
                                context.drag_set_icon_surface(&surface);
                            }
                        }
                        class(&c, "drag-source", true);
                    }
                });
                let c = cell.downgrade();
                grip.connect_drag_end(move |_, _| {
                    if let Some(c) = c.upgrade() {
                        class(&c, "drag-source", false);
                    }
                });
                let c = cell.downgrade();
                let l = label.clone();
                header.connect_drag_motion(move |_, context, _, _, time| {
                    let valid = context.drag_get_source_widget().is_some_and(|w| {
                        w.widget_name().starts_with(&format!("pane-{tid}-"))
                            && w.widget_name() != format!("pane-{tid}-{id}")
                    });
                    if let Some(c) = c.upgrade() {
                        class(&c, "drop-target", valid);
                    }
                    if valid {
                        l.set_text("↔ Release to swap");
                    }
                    context.drag_status(
                        if valid {
                            gdk::DragAction::MOVE
                        } else {
                            gdk::DragAction::empty()
                        },
                        time,
                    );
                    true
                });
                let c = cell.downgrade();
                let title = label.text();
                header.connect_drag_leave(move |_, _, _| {
                    if let Some(c) = c.upgrade() {
                        class(&c, "drop-target", false);
                    }
                    label.set_text(&title);
                });
                cell.upcast()
            }
            Tree::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let horizontal = *axis == Axis::Horizontal;
                let paned = gtk::Paned::new(if horizontal {
                    gtk::Orientation::Horizontal
                } else {
                    gtk::Orientation::Vertical
                });
                paned.set_wide_handle(true);
                let mut p = path.clone();
                p.push(false);
                paned.pack1(&self.build_tree(tab, first, p, generation), true, false);
                let mut p = path.clone();
                p.push(true);
                paned.pack2(&self.build_tree(tab, second, p, generation), true, false);
                let initialized = Rc::new(std::cell::Cell::new(false));
                let init = initialized.clone();
                let ratio = *ratio;
                paned.connect_size_allocate(move |paned, rect| {
                    let size = if horizontal {
                        rect.width()
                    } else {
                        rect.height()
                    };
                    if size > 1 && !init.replace(true) {
                        paned.set_position((f64::from(size) * ratio) as i32);
                    }
                });
                let weak = self.weak.clone();
                let tid = tab.id;
                paned.connect_position_notify(move |paned| {
                    if !initialized.get() {
                        return;
                    }
                    let size = if horizontal {
                        paned.allocated_width()
                    } else {
                        paned.allocated_height()
                    };
                    if size <= 1 {
                        return;
                    }
                    let ratio = f64::from(paned.position()) / f64::from(size);
                    let path = path.clone();
                    enqueue(&weak, move |s| {
                        if !s
                            .grids
                            .get(&tid)
                            .is_some_and(|g| g.generation == generation)
                        {
                            return;
                        }
                        if let Some(t) = s.layout.find_tab_mut(tid) {
                            t.tree.set_ratio(&path, ratio);
                            if let Some(grid) = s.grids.get_mut(&tid) {
                                grid.tree = t.tree.clone();
                            }
                            s.schedule_save();
                        }
                    });
                });
                paned.upcast()
            }
        }
    }
    fn help(&self) {
        crate::help::show(&self.window, &self.settings, &self.dir, &self.weak);
    }
    pub fn shutdown(&mut self) {
        if self.closed {
            return;
        }
        if let Some(timer) = self.save_timer.take() {
            timer.remove();
        }
        self.persist();
        self.closed = true;
        self.tray.take();
        if let Some(dialog) = self.confirmation.take() {
            unsafe {
                dialog.destroy();
            }
        }
        if let Some(d) = self.preferences.take() {
            unsafe {
                d.destroy();
            }
        }
        for session in self.sessions.values() {
            session.close();
        }
        self.sessions.clear();
        gtk::StyleContext::remove_provider_for_screen(&gdk::Screen::default().unwrap(), &self.css);
    }
}
pub fn validate_shortcuts(settings: &Settings) -> Result<(), String> {
    let mut used = HashMap::new();
    for (action, _, _) in ACTIONS {
        let binding = settings.binding(action);
        if binding.is_empty() {
            continue;
        }
        let (key, mods) = gtk::accelerator_parse(binding);
        if key == 0 {
            return Err(format!("Invalid shortcut for {action}: {binding}"));
        }
        let normalized = (normalize_key(key), mods.bits());
        if let Some(previous) = used.insert(normalized, action) {
            return Err(format!("{action} and {previous} use the same shortcut"));
        }
    }
    Ok(())
}

fn normalize_key(key: u32) -> u32 {
    if key == *gdk::keys::constants::ISO_Left_Tab {
        return *gdk::keys::constants::Tab;
    }
    // Every u32 is a valid GDK key value; unknown keysyms remain unchanged.
    *unsafe { glib::translate::from_glib::<u32, gdk::keys::Key>(key) }.to_lower()
}
