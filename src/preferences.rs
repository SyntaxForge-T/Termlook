use crate::ui::{enqueue, State};
use gtk::prelude::*;
use serde_json::Value;
use std::collections::BTreeMap;
use termlook::settings::{Settings, ACTIONS};

#[derive(Clone)]
enum Field {
    Text(gtk::Entry),
    Bool(gtk::Switch),
    Number(gtk::SpinButton),
    Json(gtk::TextBuffer),
    Choice(gtk::ComboBoxText),
    Profiles(crate::projects::Editor),
}
impl Field {
    fn value(&self) -> Result<Value, String> {
        Ok(match self {
            Self::Text(e) => Value::String(e.text().into()),
            Self::Choice(c) => Value::String(c.active_id().unwrap_or_default().into()),
            Self::Profiles(editor) => editor.value()?,
            Self::Bool(b) => Value::Bool(b.is_active()),
            Self::Number(n) => Value::from(n.value_as_int()),
            Self::Json(b) => serde_json::from_str(
                &b.text(&b.start_iter(), &b.end_iter(), true)
                    .unwrap_or_default(),
            )
            .map_err(|e| e.to_string())?,
        })
    }
    fn set(&self, v: &Value) {
        match self {
            Self::Text(e) => e.set_text(v.as_str().unwrap_or_default()),
            Self::Choice(c) => {
                c.set_active_id(v.as_str());
            }
            Self::Profiles(_) => {}
            Self::Bool(b) => b.set_active(v.as_bool().unwrap_or(false)),
            Self::Number(n) => n.set_value(v.as_f64().unwrap_or_default()),
            Self::Json(b) => b.set_text(&serde_json::to_string_pretty(v).unwrap()),
        }
    }
}
fn page(book: &gtk::Notebook, name: &str) -> gtk::Box {
    let scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    for side in [0, 1, 2, 3] {
        match side {
            0 => body.set_margin_top(16),
            1 => body.set_margin_bottom(16),
            2 => body.set_margin_start(16),
            _ => body.set_margin_end(16),
        }
    }
    scroll.add(&body);
    book.append_page(&scroll, Some(&gtk::Label::new(Some(name))));
    body
}
/// Show a validation error (red) or a success message (green) under the pages.
fn report(label: &gtk::Label, ok: bool, text: &str) {
    let style = label.style_context();
    style.remove_class(if ok { "pref-error" } else { "pref-success" });
    style.add_class(if ok { "pref-success" } else { "pref-error" });
    label.set_text(text);
}
fn note(body: &gtk::Box, text: &str) {
    let label = gtk::Label::new(Some(text));
    label.set_line_wrap(true);
    label.set_xalign(0.0);
    label.style_context().add_class("pref-muted");
    body.pack_start(&label, false, false, 0);
}
pub fn show(state: &mut State) {
    show_page(state, 0);
}
pub fn show_page(state: &mut State, page_index: u32) {
    state.restore();
    if let Some(dialog) = &state.preferences {
        for child in dialog.content_area().children() {
            if let Ok(book) = child.downcast::<gtk::Notebook>() {
                book.set_current_page(Some(page_index));
            }
        }
        dialog.present();
        return;
    }
    let dialog = gtk::Dialog::with_buttons(
        Some("TermLook Settings"),
        Some(&state.window),
        gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[
            ("Discard changes", gtk::ResponseType::Cancel),
            ("Apply", gtk::ResponseType::Apply),
            ("Save and close", gtk::ResponseType::Accept),
        ],
    );
    dialog.set_default_size(740, 620);
    dialog.style_context().add_class("preferences-window");
    let book = gtk::Notebook::new();
    dialog.content_area().pack_start(&book, true, true, 0);
    let appearance = page(&book, "Appearance");
    let terminal = page(&book, "Terminal");
    let behavior = page(&book, "Behavior");
    let shortcuts = page(&book, "Shortcuts");
    let profiles = page(&book, "Profiles");
    let presets = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    appearance.pack_start(&presets, false, false, 0);
    let values = serde_json::to_value(&state.settings).unwrap();
    let mut fields = BTreeMap::new();
    for (key, label, body, tip) in [
        (
            "font",
            "Font",
            &appearance,
            "Pango font description, e.g. Monospace 11",
        ),
        ("foreground", "Text color", &appearance, "#RRGGBB"),
        ("background", "Background color", &appearance, "#RRGGBB"),
        (
            "background_dim",
            "Photo dimming (%)",
            &appearance,
            "0: original photo; 100: solid terminal color",
        ),
        (
            "background_fit",
            "Photo fit",
            &appearance,
            "Cover crops edges; contain keeps the entire photo; stretch fills the area",
        ),
        (
            "cursor",
            "Cursor shape",
            &appearance,
            "block, ibeam or underline",
        ),
        (
            "blink",
            "Blink cursor",
            &appearance,
            "Enable cursor blinking",
        ),
        ("padding", "Terminal padding", &appearance, "0–40 pixels"),
        (
            "sidebar_width",
            "Sidebar width",
            &appearance,
            "140–320 pixels",
        ),
        (
            "shell",
            "Shell executable",
            &terminal,
            "Absolute path; empty uses your system shell. Applies to new sessions.",
        ),
        (
            "scrollback",
            "Scrollback lines",
            &terminal,
            "100–1000000 lines; larger values use more memory",
        ),
        (
            "audible_bell",
            "Audible bell",
            &terminal,
            "Allow programs to ring the terminal bell",
        ),
        (
            "scroll_on_output",
            "Scroll on output",
            &terminal,
            "Jump to the bottom when output arrives",
        ),
        (
            "scroll_on_keystroke",
            "Scroll on keystroke",
            &terminal,
            "Jump to the bottom when typing",
        ),
        (
            "close_to_tray",
            "Keep running when window closes",
            &behavior,
            "Hide to the tray, or minimize if no tray host is available. Quit ends sessions.",
        ),
        (
            "show_tab_close_buttons",
            "Show tab close buttons",
            &behavior,
            "You can always close a tab with its keyboard shortcut",
        ),
        (
            "default_profile",
            "Default profile",
            &profiles,
            "Profile name from the JSON below, or empty to use global shell and current directory",
        ),
    ] {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        row.style_context().add_class("pref-row");
        row.set_tooltip_text(Some(tip));
        let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let title = gtk::Label::new(Some(label));
        title.set_xalign(0.0);
        title.style_context().add_class("pref-row-title");
        text.pack_start(&title, false, false, 0);
        let description = gtk::Label::new(Some(tip));
        description.set_xalign(0.0);
        description.set_line_wrap(true);
        description.style_context().add_class("pref-muted");
        text.pack_start(&description, false, false, 0);
        row.pack_start(&text, true, true, 0);
        let value = &values[key];
        let field = match value {
            Value::Bool(b) => {
                let input = gtk::Switch::new();
                input.set_active(*b);
                input.set_valign(gtk::Align::Center);
                row.pack_end(&input, false, false, 0);
                Field::Bool(input)
            }
            Value::Number(_) => {
                let (min, max) = match key {
                    "scrollback" => (100.0, 1_000_000.0),
                    "padding" => (0.0, 40.0),
                    "background_dim" => (0.0, 100.0),
                    _ => (140.0, 320.0),
                };
                let input = gtk::SpinButton::with_range(min, max, 1.0);
                input.set_value(value.as_f64().unwrap());
                input.set_valign(gtk::Align::Center);
                row.pack_end(&input, false, false, 0);
                Field::Number(input)
            }
            _ if key == "background_fit" => {
                let input = gtk::ComboBoxText::new();
                for (id, label) in [
                    ("cover", "Cover"),
                    ("contain", "Contain"),
                    ("stretch", "Stretch"),
                ] {
                    input.append(Some(id), label);
                }
                input.set_active_id(value.as_str());
                row.pack_end(&input, false, false, 0);
                Field::Choice(input)
            }
            _ => {
                let input = gtk::Entry::new();
                input.set_text(value.as_str().unwrap_or_default());
                input.set_width_chars(22);
                input.set_valign(gtk::Align::Center);
                row.pack_end(&input, false, false, 0);
                Field::Text(input)
            }
        };
        body.pack_start(&row, false, false, 0);
        fields.insert(key.to_owned(), field);
    }
    note(&appearance, "Background image: choose a local photo (PNG, JPEG, WebP or another supported image format). Apply changes without restarting terminals.");
    let image_path = gtk::Entry::new();
    image_path.set_text(&state.settings.background_image);
    image_path.set_placeholder_text(Some("No image — solid color"));
    appearance.pack_start(&image_path, false, false, 0);
    let image_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let browse = gtk::Button::with_label("Choose image…");
    let remove = gtk::Button::with_label("Remove image");
    image_actions.pack_start(&browse, false, false, 0);
    image_actions.pack_start(&remove, false, false, 0);
    appearance.pack_start(&image_actions, false, false, 0);
    let parent = dialog.downgrade();
    let input = image_path.clone();
    browse.connect_clicked(move |_| {
        if let Some(parent) = parent.upgrade() {
            let chooser = gtk::FileChooserDialog::new(
                Some("Terminal background image"),
                Some(&parent),
                gtk::FileChooserAction::Open,
            );
            chooser.add_buttons(&[
                ("Cancel", gtk::ResponseType::Cancel),
                ("Select image", gtk::ResponseType::Accept),
            ]);
            let filter = gtk::FileFilter::new();
            filter.set_name(Some("Images"));
            filter.add_pixbuf_formats();
            chooser.add_filter(filter);
            let input = input.clone();
            chooser.connect_response(move |d, r| {
                if r == gtk::ResponseType::Accept {
                    if let Some(path) = d.filename() {
                        input.set_text(&path.to_string_lossy());
                    }
                }
                unsafe {
                    d.destroy();
                }
            });
            chooser.show();
        }
    });
    let input = image_path.clone();
    remove.connect_clicked(move |_| input.set_text(""));
    fields.insert("background_image".into(), Field::Text(image_path));
    note(
        &appearance,
        "ANSI palette: exactly 16 #RRGGBB colors, as a JSON array.",
    );
    let view = gtk::TextView::new();
    view.set_monospace(true);
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    let buffer = view.buffer().unwrap();
    buffer.set_text(&serde_json::to_string_pretty(&values["palette"]).unwrap());
    let scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    scroll.set_min_content_height(190);
    scroll.add(&view);
    appearance.pack_start(&scroll, true, true, 0);
    fields.insert("palette".into(), Field::Json(buffer));
    if let Field::Text(default) = &fields["default_profile"] {
        let editor = crate::projects::Editor::new(&state.settings.profiles, &dialog, default);
        profiles.pack_start(&editor.widget, false, false, 0);
        fields.insert("profiles".into(), Field::Profiles(editor));
    }
    note(&shortcuts, "Use GTK accelerator notation: <Primary><Shift>t, <Alt>1, F2. Leave empty to disable an action. Press Apply to save; duplicates are rejected.");
    let mut bindings = vec![];
    for (action, title, _) in ACTIONS {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        row.style_context().add_class("pref-row");
        let label = gtk::Label::new(Some(title));
        label.set_xalign(0.0);
        label.style_context().add_class("pref-row-title");
        row.pack_start(&label, true, true, 0);
        let entry = gtk::Entry::new();
        entry.set_text(state.settings.binding(action));
        entry.set_width_chars(22);
        entry.set_valign(gtk::Align::Center);
        entry.set_placeholder_text(Some("not set"));
        row.pack_end(&entry, false, false, 0);
        shortcuts.pack_start(&row, false, false, 0);
        bindings.push((action.to_string(), entry));
    }
    for (name, fg, bg) in [
        ("Charcoal", "#d8d6eb", "#202127"),
        ("Midnight", "#cdd6f4", "#11111b"),
        ("Light", "#24292f", "#f6f8fa"),
    ] {
        let b = gtk::Button::with_label(name);
        let foreground = fields["foreground"].clone();
        let background = fields["background"].clone();
        b.connect_clicked(move |_| {
            foreground.set(&Value::String(fg.into()));
            background.set(&Value::String(bg.into()));
        });
        presets.pack_start(&b, false, false, 0);
    }
    let error = gtk::Label::new(None);
    error.set_line_wrap(true);
    error.style_context().add_class("pref-error");
    dialog.content_area().pack_end(&error, false, false, 8);
    let weak = state.weak.clone();
    dialog.connect_delete_event(|d, _| {
        d.response(gtk::ResponseType::Accept);
        glib::Propagation::Stop
    });
    dialog.connect_response(move |d, response| {
        if ![
            gtk::ResponseType::Apply,
            gtk::ResponseType::Accept,
            gtk::ResponseType::DeleteEvent,
        ]
        .contains(&response)
        {
            unsafe {
                d.destroy();
            }
            return;
        }
        let mut data = values.clone();
        for (key, field) in &fields {
            match field.value() {
                Ok(value) => data[key] = value,
                Err(e) => {
                    report(&error, false, &format!("{key}: {e}"));
                    return;
                }
            }
        }
        data["keybindings"] = Value::Object(
            bindings
                .iter()
                .map(|(key, input)| (key.clone(), Value::String(input.text().into())))
                .collect(),
        );
        let result = serde_json::from_value::<Settings>(data)
            .map_err(|e| e.to_string())
            .and_then(|s| {
                s.validate()?;
                crate::ui::validate_shortcuts(&s)?;
                Ok(s)
            });
        match result {
            Err(e) => report(&error, false, &e),
            Ok(settings) => {
                let error = error.clone();
                let dialog = d.downgrade();
                d.set_sensitive(false);
                enqueue(&weak, move |s| {
                    let Some(dialog) = dialog.upgrade() else {
                        return;
                    };
                    dialog.set_sensitive(true);
                    match s.apply_settings(settings) {
                        Ok(()) => {
                            if response != gtk::ResponseType::Apply {
                                unsafe {
                                    dialog.destroy();
                                }
                            } else {
                                report(&error, true, "Settings and projects saved.");
                            }
                        }
                        Err(e) => report(&error, false, &e),
                    }
                });
            }
        }
    });
    let weak = state.weak.clone();
    dialog.connect_destroy(move |_| {
        enqueue(&weak, |s| {
            s.preferences = None;
        })
    });
    state.preferences = Some(dialog.clone());
    dialog.show_all();
    book.set_current_page(Some(page_index));
}
