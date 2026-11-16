use crate::ui::{enqueue, State};
use gtk::prelude::*;
use serde_json::Value;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use termlook::settings::{shortcut_label, Settings, ACTIONS};

const ANSI_NAMES: [&str; 8] = [
    "Black", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "White",
];

#[derive(Clone)]
enum Field {
    Text(gtk::Entry),
    Bool(gtk::Switch),
    Number(gtk::SpinButton),
    Choice(gtk::ComboBoxText),
    Font(gtk::FontButton),
    Color(gtk::ColorButton),
    Palette(Vec<gtk::ColorButton>),
    Profiles(crate::projects::Editor),
}
/// `#rrggbb` for a color picked in a color button.
fn hex(color: &gdk::RGBA) -> String {
    let channel = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        channel(color.red()),
        channel(color.green()),
        channel(color.blue())
    )
}
fn set_color(button: &gtk::ColorButton, value: &str) {
    if let Ok(color) = gdk::RGBA::parse(value) {
        button.set_rgba(&color);
    }
}
fn color_button(value: &str, title: &str) -> gtk::ColorButton {
    let button = gtk::ColorButton::new();
    button.set_use_alpha(false);
    button.set_title(title);
    button.set_valign(gtk::Align::Center);
    set_color(&button, value);
    button
}
fn choice(options: &[(&str, &str)], current: &str) -> gtk::ComboBoxText {
    let input = gtk::ComboBoxText::new();
    for (id, label) in options {
        input.append(Some(id), label);
    }
    input.set_active_id(Some(current));
    input.set_valign(gtk::Align::Center);
    input
}
/// Plain modifier presses must not be recorded as a shortcut.
fn is_modifier(key: u32) -> bool {
    (0xffe1..=0xffee).contains(&key) || key == 0xfe03 || key == 0xff7f
}
/// A field that records the next key combination pressed in it. The stored
/// value stays a GTK accelerator; the field shows it as `Ctrl+Shift+T`.
fn shortcut_field(initial: &str) -> (gtk::Entry, Rc<RefCell<String>>) {
    let accelerator = Rc::new(RefCell::new(initial.to_owned()));
    let entry = gtk::Entry::new();
    entry.set_editable(false);
    entry.set_width_chars(22);
    entry.set_valign(gtk::Align::Center);
    entry.set_text(&shortcut_label(initial));
    entry.set_placeholder_text(Some("not set"));
    entry.set_tooltip_text(Some(
        "Click here, then press the new shortcut. Backspace clears it.",
    ));
    let stored = accelerator.clone();
    entry.connect_key_press_event(move |entry, event| {
        use gdk::keys::constants as k;
        let key = *event.keyval();
        let mods = event.state() & gtk::accelerator_get_default_mod_mask();
        // Tab still moves between fields; modifiers on their own do nothing yet.
        if (key == *k::Tab && mods.is_empty())
            || (key == *k::ISO_Left_Tab && mods == gdk::ModifierType::SHIFT_MASK)
        {
            return glib::Propagation::Proceed;
        }
        if is_modifier(key) || (key == *k::Escape && mods.is_empty()) {
            return glib::Propagation::Stop;
        }
        if mods.is_empty() && (key == *k::BackSpace || key == *k::Delete) {
            stored.borrow_mut().clear();
            entry.set_text("");
            return glib::Propagation::Stop;
        }
        let key = if key == *k::ISO_Left_Tab {
            *k::Tab
        } else {
            key
        };
        let Some(name) = gtk::accelerator_name(key, mods).map(|name| name.to_string()) else {
            return glib::Propagation::Stop;
        };
        entry.set_text(&shortcut_label(&name));
        *stored.borrow_mut() = name;
        glib::Propagation::Stop
    });
    (entry, accelerator)
}
impl Field {
    fn value(&self) -> Result<Value, String> {
        Ok(match self {
            Self::Text(e) => Value::String(e.text().into()),
            Self::Choice(c) => Value::String(c.active_id().unwrap_or_default().into()),
            Self::Profiles(editor) => editor.value()?,
            Self::Bool(b) => Value::Bool(b.is_active()),
            Self::Number(n) => Value::from(n.value_as_int()),
            Self::Font(f) => Value::String(f.font().map(Into::into).unwrap_or_default()),
            Self::Color(c) => Value::String(hex(&c.rgba())),
            Self::Palette(buttons) => Value::Array(
                buttons
                    .iter()
                    .map(|b| Value::String(hex(&b.rgba())))
                    .collect(),
            ),
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
            Self::Font(f) => f.set_font(v.as_str().unwrap_or_default()),
            Self::Color(c) => set_color(c, v.as_str().unwrap_or_default()),
            Self::Palette(buttons) => {
                for (button, color) in buttons.iter().zip(v.as_array().into_iter().flatten()) {
                    set_color(button, color.as_str().unwrap_or_default());
                }
            }
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
            "Pick a monospace font and its size",
        ),
        (
            "foreground",
            "Text color",
            &appearance,
            "Default color of terminal text",
        ),
        (
            "background",
            "Background color",
            &appearance,
            "Terminal and window background",
        ),
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
            "Shape of the text cursor",
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
            "Saved project whose shell and folder new tabs use. Leave empty for the global shell and the current folder.",
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
        let current = value.as_str().unwrap_or_default();
        let field = match (key, value) {
            ("font", _) => {
                let input = gtk::FontButton::new();
                input.set_title("Terminal font");
                input.set_show_size(true);
                input.set_filter_func(Some(Box::new(
                    |family: &pango::FontFamily, _: &pango::FontFace| family.is_monospace(),
                )));
                input.set_font(current);
                input.set_valign(gtk::Align::Center);
                row.pack_end(&input, false, false, 0);
                Field::Font(input)
            }
            ("foreground" | "background", _) => {
                let input = color_button(current, label);
                row.pack_end(&input, false, false, 0);
                Field::Color(input)
            }
            ("background_fit", _) => {
                let input = choice(
                    &[
                        ("cover", "Cover"),
                        ("contain", "Contain"),
                        ("stretch", "Stretch"),
                    ],
                    current,
                );
                row.pack_end(&input, false, false, 0);
                Field::Choice(input)
            }
            ("cursor", _) => {
                let input = choice(
                    &[
                        ("block", "Block"),
                        ("ibeam", "I-beam"),
                        ("underline", "Underline"),
                    ],
                    current,
                );
                row.pack_end(&input, false, false, 0);
                Field::Choice(input)
            }
            (_, Value::Bool(b)) => {
                let input = gtk::Switch::new();
                input.set_active(*b);
                input.set_valign(gtk::Align::Center);
                row.pack_end(&input, false, false, 0);
                Field::Bool(input)
            }
            (_, Value::Number(_)) => {
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
            _ => {
                let input = gtk::Entry::new();
                input.set_text(current);
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
        "ANSI palette: the 16 colors programs use. Top row normal, bottom row bright.",
    );
    let grid = gtk::Grid::new();
    grid.set_column_spacing(8);
    grid.set_row_spacing(8);
    let palette: Vec<gtk::ColorButton> = values["palette"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(i, color)| {
            let name = format!(
                "{} {}",
                if i < 8 { "Normal" } else { "Bright" },
                ANSI_NAMES[i % 8].to_lowercase()
            );
            let button = color_button(color.as_str().unwrap_or_default(), &name);
            button.set_tooltip_text(Some(&name));
            grid.attach(&button, (i % 8) as i32, (i / 8) as i32, 1, 1);
            button
        })
        .collect();
    appearance.pack_start(&grid, false, false, 0);
    let reset = gtk::Button::with_label("Reset palette");
    reset.set_halign(gtk::Align::Start);
    let colors = palette.clone();
    reset.connect_clicked(move |_| {
        let defaults: Vec<String> = Settings::default().palette;
        for (button, color) in colors.iter().zip(&defaults) {
            set_color(button, color);
        }
    });
    appearance.pack_start(&reset, false, false, 0);
    fields.insert("palette".into(), Field::Palette(palette));
    if let Field::Text(default) = &fields["default_profile"] {
        let editor = crate::projects::Editor::new(&state.settings.profiles, &dialog, default);
        profiles.pack_start(&editor.widget, false, false, 0);
        fields.insert("profiles".into(), Field::Profiles(editor));
    }
    note(&shortcuts, "Click a field and press the new shortcut. Backspace clears it, which disables the action. Duplicates are rejected when you apply.");
    let mut bindings = vec![];
    for (action, title, _) in ACTIONS {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        row.style_context().add_class("pref-row");
        let label = gtk::Label::new(Some(title));
        label.set_xalign(0.0);
        label.style_context().add_class("pref-row-title");
        row.pack_start(&label, true, true, 0);
        let (entry, accelerator) = shortcut_field(state.settings.binding(action));
        row.pack_end(&entry, false, false, 0);
        shortcuts.pack_start(&row, false, false, 0);
        bindings.push((action.to_string(), accelerator));
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
                .map(|(key, input)| (key.clone(), Value::String(input.borrow().clone())))
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
