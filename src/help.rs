//! The in-app help window: a quick start guide, searchable shortcuts that
//! always reflect the user's current keybindings, and customization tips.
use crate::ui::{enqueue, WeakState};
use gtk::prelude::*;
use std::path::Path;
use termlook::settings::{shortcut_keys, shortcut_label, Settings, ACTIONS};

/// Shortcut sections shown on the "Shortcuts" page, in display order.
const GROUPS: &[(&str, &[&str])] = &[
    (
        "Workspaces and tabs",
        &[
            "projects",
            "new-workspace",
            "new-tab",
            "rename-tab",
            "next-tab",
            "previous-tab",
            "close-tab",
        ],
    ),
    (
        "Panes",
        &[
            "split-horizontal",
            "split-vertical",
            "grid",
            "zoom",
            "close-pane",
        ],
    ),
    ("Terminal", &["copy", "paste"]),
    (
        "Application",
        &["settings", "reload-settings", "help", "quit"],
    ),
];

/// Mouse interactions: (what it does, how to do it).
const MOUSE: &[(&str, &str)] = &[
    ("Rename a workspace or tab", "Double-click its name"),
    ("Swap two panes", "Drag a pane header onto another"),
    ("Resize panes", "Drag a divider"),
    ("Copy or paste", "Right-click in a terminal"),
    ("Move the window", "Drag the top bar"),
];

fn label(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.set_xalign(0.0);
    l.set_line_wrap(true);
    l.style_context().add_class(class);
    l
}

fn card(title: &str, body: &str) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 4);
    card.style_context().add_class("help-card");
    card.pack_start(&label(title, "help-card-title"), false, false, 0);
    card.pack_start(&label(body, "help-card-body"), false, false, 0);
    card
}

/// A scrollable page with consistent padding; returns the outer widget and
/// the box that receives the content.
fn page() -> (gtk::ScrolledWindow, gtk::Box) {
    let scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 10);
    body.set_margin_top(6);
    body.set_margin_bottom(12);
    body.set_margin_start(18);
    body.set_margin_end(18);
    scroll.add(&body);
    (scroll, body)
}

/// One line of the shortcut list: a title on the left and key caps on the right.
fn row(title: &str, caps: &[String], class: &str, empty: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.style_context().add_class("help-row");
    row.pack_start(&label(title, "help-row-title"), true, true, 0);
    let keys = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    if caps.is_empty() {
        keys.pack_start(&label(empty, "pref-muted"), false, false, 0);
    }
    for cap in caps {
        let key = gtk::Label::new(Some(cap));
        key.style_context().add_class(class);
        keys.pack_start(&key, false, false, 0);
    }
    row.pack_end(&keys, false, false, 0);
    row
}

fn action_title(action: &str) -> String {
    ACTIONS
        .iter()
        .find(|(a, _, _)| *a == action)
        .map(|(_, title, _)| (*title).to_owned())
        .unwrap_or_else(|| action.to_owned())
}

/// ` (Ctrl+Shift+T)` for an assigned action, or nothing.
fn hint(settings: &Settings, action: &str) -> String {
    let shortcut = shortcut_label(settings.binding(action));
    if shortcut.is_empty() {
        String::new()
    } else {
        format!(" ({shortcut})")
    }
}

fn quick_start(settings: &Settings) -> gtk::ScrolledWindow {
    let (scroll, body) = page();
    body.pack_start(
        &label(
            "Everything you need to get going. Press F1 at any time to open this window.",
            "pref-muted",
        ),
        false,
        false,
        0,
    );
    let cards = [
        ("Project profiles".to_owned(), format!("Use Projects… in the sidebar{} to open a saved project workspace. Settings → Profiles lets you choose its name, folder and shell. Save project or Apply saves the current form directly; closing Settings saves too. Discard changes cancels unsaved edits. Add startup tasks as Name | command, enable automatic launch, and choose completion notifications. Tasks run concurrently in separate tabs with live logs and exit status. Opening the same profile again returns to its workspace without duplicate tasks; Open + run tasks reruns a finished template. Ctrl+C stops a foreground task. Logs last until the tab or app closes.", hint(settings,"projects"))),

        (
            "Workspaces".to_owned(),
            format!(
                "A workspace groups the tabs of one project. Create one with the + above the sidebar{}, \
                 switch by clicking it, and double-click a name to rename it. The × next to a \
                 workspace asks for confirmation before deleting it and ending its sessions.",
                hint(settings, "new-workspace")
            ),
        ),
        (
            "Tabs".to_owned(),
            format!(
                "Add a tab with the + in the top bar{}. Switching tabs never interrupts a shell: \
                 running programs, output and variables stay alive. Closing a tab asks for confirmation; Cancel leaves its sessions running.",
                hint(settings, "new-tab")
            ),
        ),
        (
            "Split panes".to_owned(),
            format!(
                "Split a tab left/right{} or top/bottom{}, up to four panes. Drag a pane header \
                 onto another pane to swap them and drag dividers to resize. Focus pane{} \
                 temporarily hides the others; their shells keep running.",
                hint(settings, "split-horizontal"),
                hint(settings, "split-vertical"),
                hint(settings, "zoom")
            ),
        ),
        (
            "Restarting".to_owned(),
            "TermLook remembers workspace and tab names, folders and split layout. After a restart \
             you get fresh shells in those folders; running programs and scrollback are not restored."
                .to_owned(),
        ),
        (
            "Background mode".to_owned(),
            format!(
                "Closing the window hides TermLook to the tray (or minimizes it when no tray is \
                 available) so your sessions stay alive. Quit completely with {}. \
                 You can turn this off in Settings → Behavior.",
                match shortcut_label(settings.binding("quit")).as_str() {
                    "" => "the tray menu's Quit".to_owned(),
                    key => format!("{key} or the tray menu's Quit"),
                }
            ),
        ),
    ];
    for (title, text) in &cards {
        body.pack_start(&card(title, text), false, false, 0);
    }
    scroll
}

fn shortcuts(settings: &Settings) -> gtk::Box {
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Search actions or shortcuts…"));
    search.set_margin_start(18);
    search.set_margin_end(18);
    outer.pack_start(&search, false, false, 0);
    let (scroll, body) = page();
    outer.pack_start(&scroll, true, true, 0);

    // Each section keeps its rows with a lowercase haystack for filtering.
    let mut sections: Vec<(gtk::Box, Vec<(String, gtk::Box)>)> = vec![];
    for (name, actions) in GROUPS {
        let section = gtk::Box::new(gtk::Orientation::Vertical, 4);
        section.pack_start(
            &label(&name.to_uppercase(), "pref-section"),
            false,
            false,
            2,
        );
        let mut rows = vec![];
        for action in *actions {
            let title = action_title(action);
            let binding = settings.binding(action);
            let widget = row(&title, &shortcut_keys(binding), "keycap", "not set");
            section.pack_start(&widget, false, false, 0);
            let haystack = format!("{title} {} {name}", shortcut_label(binding)).to_lowercase();
            rows.push((haystack, widget));
        }
        body.pack_start(&section, false, false, 0);
        sections.push((section, rows));
    }
    let mouse = gtk::Box::new(gtk::Orientation::Vertical, 4);
    mouse.pack_start(&label("MOUSE", "pref-section"), false, false, 2);
    let mut rows = vec![];
    for (what, how) in MOUSE {
        let widget = row(what, &[(*how).to_owned()], "help-gesture", "");
        mouse.pack_start(&widget, false, false, 0);
        rows.push((format!("{what} {how} mouse").to_lowercase(), widget));
    }
    body.pack_start(&mouse, false, false, 0);
    sections.push((mouse, rows));

    let empty = label("No shortcuts match your search.", "pref-muted");
    empty.set_no_show_all(true);
    empty.hide();
    body.pack_start(&empty, false, false, 0);
    body.pack_start(
        &label("Change any shortcut in Settings → Shortcuts.", "pref-muted"),
        false,
        false,
        4,
    );

    search.connect_search_changed(move |entry| {
        let needle = entry.text().to_lowercase();
        let mut any = false;
        for (section, rows) in &sections {
            let mut shown = false;
            for (haystack, widget) in rows {
                let hit = haystack.contains(&needle);
                widget.set_visible(hit);
                shown |= hit;
            }
            section.set_visible(shown);
            any |= shown;
        }
        empty.set_visible(!any);
    });
    outer
}

fn customize(
    settings: &Settings,
    dir: &Path,
    dialog: &gtk::Dialog,
    weak: &WeakState,
) -> gtk::ScrolledWindow {
    let (scroll, body) = page();
    let settings_card = card(
        "Settings",
        &format!(
            "Choose a background photo, fonts, colors, cursor, shell, behavior and shortcuts. Changes apply live{}.",
            hint(settings, "settings")
        ),
    );
    let open = gtk::Button::with_label("Open Settings");
    open.set_halign(gtk::Align::Start);
    open.set_margin_top(6);
    open.style_context().add_class("pref-primary");
    let dialog = dialog.downgrade();
    let weak = weak.clone();
    open.connect_clicked(move |_| {
        // The help window is modal, so close it before showing Settings.
        if let Some(dialog) = dialog.upgrade() {
            dialog.response(gtk::ResponseType::Close);
        }
        enqueue(&weak, |s| s.action("settings"));
    });
    settings_card.pack_start(&open, false, false, 0);
    body.pack_start(&settings_card, false, false, 0);

    body.pack_start(&card("Background photo", "Settings → Appearance → Choose image… loads your own photo. Choose cover, contain or stretch and adjust dimming for readable text. Apply changes live; Remove image restores the solid color."),false,false,0);
    let files = card(
        "Files",
        &format!(
            "Settings: {}\nLayout: {}\nInterface styling: {}\n\nAfter editing a file by hand, {}. \
             Invalid files are reported and the current settings stay active.",
            dir.join("settings.json").display(),
            dir.join("layout.json").display(),
            dir.join("theme.css").display(),
            match shortcut_label(settings.binding("reload-settings")).as_str() {
                "" => "restart TermLook or assign a Reload settings shortcut".to_owned(),
                key => format!("press {key} to reload it"),
            }
        ),
    );
    for child in files.children() {
        if let Ok(text) = child.downcast::<gtk::Label>() {
            text.set_selectable(true);
        }
    }
    body.pack_start(&files, false, false, 0);

    let css = card(
        "Custom styling",
        "Create theme.css next to settings.json to restyle the interface, then reload settings. For example:",
    );
    let sample = label(
        "#sidebar { background: #11111b; }\n.terminal-tab.active { background: #45475a; }\n.terminal-pane.focused { border-color: #89b4fa; }",
        "help-code",
    );
    sample.set_selectable(true);
    css.pack_start(&sample, false, false, 4);
    body.pack_start(&css, false, false, 0);

    body.pack_start(
        &card(
            "Command line",
            "termlook --print-default-settings prints every option with its default value.\ntermlook --version shows the installed version.",
        ),
        false,
        false,
        0,
    );
    scroll
}

fn project_guide() -> gtk::ScrolledWindow {
    let (scroll, body) = page();
    for (title, text) in [
        ("Створення проєкту", "1. Відкрийте Projects… → Manage profiles (або Settings → Profiles).\n2. Натисніть Choose folder… та виберіть папку проєкту. Назву можна вказати самостійно; якщо залишити порожньою, вона буде взята з назви папки.\n3. Shell можна залишити порожнім — використається стандартна оболонка.\n4. Натисніть Save project або Save and close. Apply і хрестик вікна налаштувань також зберігають форму. Discard changes відкидає лише незбережені зміни.\n5. Відкрийте Projects…, виберіть проєкт і натисніть Open project. Збережені проєкти видно списком над формою. Натисніть рядок, щоб редагувати його, або New project для нового профілю."),
        ("Видалення збереженого проєкту", "Відкрийте Projects…: біля кожного проєкту є кнопки Редагувати та Видалити. Натисніть Видалити й підтвердьте дію — профіль одразу зникне зі списку та буде видалений із налаштувань. Папка, файли й уже запущені термінали залишаться. Якщо налаштування відкриті, спочатку збережіть або відкиньте їхні зміни. У Settings → Profiles також можна вибрати видимий рядок та натиснути Видалити вибраний проєкт, потім зберегти зміни."),
        ("Автозапуск команд", "У полі Startup tasks введіть по одному завданню на рядок: Назва | команда. Наприклад:\nServer | npm run dev\nTests | npm test\nLogs | tail -f app.log\n\nЦе приклади: вкажіть команди саме вашого проєкту. Для звичайного термінала залиште поле порожнім. Увімкніть Run tasks automatically when I open this project та збережіть налаштування. Кожна команда запускається паралельно у своїй вкладці з папки проєкту. Сервер залишайте на передньому плані, без & в кінці команди."),
        ("Логи, помилки та сповіщення", "Вивід програми й повідомлення про помилки з’являються в її терміналі під час роботи. ▶ означає запуск, ✓ — успішне завершення, ✗ — ненульовий код завершення або сигнал зупинки. Текст помилки у ще працюючій програмі не змінює статус автоматично.\nУ профілі виберіть Notify on errors only (лише помилки), Notify on every completion (усі завершення) або No desktop notifications. Сповіщення потребують служби повідомлень вашого робочого столу.\nЛоги залишаються у вкладці до її закриття або виходу з TermLook; на диск вони автоматично не архівуються."),
        ("Зупинка та повторний запуск", "Ctrl+C у вкладці зупиняє команду; закриття вкладки потребує підтвердження. Повторне Open project повертає до робочого простору без дублювання завдань. Open + run tasks запускає завершений набір ще раз у нових вкладках. Поки хоча б одне завдання працює, повторний запуск заблокований. Після перезапуску TermLook відкрийте профіль через Projects, щоб запустити команди знову."),
        ("Якщо проєкт не зберігається", "Після успішного Save project або Apply внизу налаштувань з’явиться Settings and projects saved. Якщо папка відсутня, оболонка не виконується або формат команди неправильний, буде показано помилку, а вікно залишиться відкритим. Виправте дані та збережіть ще раз. Якщо назва папки вже зайнята іншим профілем, задайте унікальну назву. Зміна профілю впливає на наступні запуски, а не перезапускає поточні процеси."),
    ] { body.pack_start(&card(title, text), false, false, 0); }
    scroll
}

pub fn show(window: &gtk::ApplicationWindow, settings: &Settings, dir: &Path, weak: &WeakState) {
    let dialog = gtk::Dialog::with_buttons(
        Some("TermLook Help"),
        Some(window),
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        &[("Close", gtk::ResponseType::Close)],
    );
    dialog.set_default_size(700, 640);
    dialog.style_context().add_class("help-window");
    let content = dialog.content_area();

    let header = gtk::Box::new(gtk::Orientation::Vertical, 2);
    header.set_margin_top(18);
    header.set_margin_start(18);
    header.set_margin_end(18);
    header.set_margin_bottom(10);
    header.pack_start(&label("Help", "help-title"), false, false, 0);
    header.pack_start(
        &label(
            &format!(
                "TermLook {} — a little room for every project.",
                env!("CARGO_PKG_VERSION")
            ),
            "pref-muted",
        ),
        false,
        false,
        0,
    );
    content.pack_start(&header, false, false, 0);

    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.add_titled(&quick_start(settings), "start", "Quick start");
    stack.add_titled(&project_guide(), "projects", "Проєкти");
    stack.add_titled(&shortcuts(settings), "shortcuts", "Shortcuts");
    stack.add_titled(
        &customize(settings, dir, &dialog, weak),
        "customize",
        "Customize",
    );
    let switcher = gtk::StackSwitcher::new();
    switcher.set_stack(Some(&stack));
    switcher.set_halign(gtk::Align::Center);
    switcher.set_margin_bottom(8);
    content.pack_start(&switcher, false, false, 0);
    content.pack_start(&stack, true, true, 0);

    dialog.connect_response(|d, _| unsafe {
        d.destroy();
    });
    dialog.show_all();
}
