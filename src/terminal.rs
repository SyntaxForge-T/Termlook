//! Small GTK 3 VTE adapter. All FFI stays on the GTK main thread.
//! VTE owns the PTY, parser and rendering; Rust owns cancellation and widget lifetime.
use glib::translate::*;
use gtk::prelude::*;
use std::{
    cell::{Cell, RefCell},
    ffi::{CStr, CString},
    os::raw::{c_char, c_int, c_void},
    path::Path,
    rc::Rc,
};
use termlook::{model::home, settings::Settings};

type Terminal = c_void;
#[link(name = "vte-2.91")]
extern "C" {
    fn vte_terminal_set_clear_background(t: *mut Terminal, enabled: c_int);
    fn vte_terminal_new() -> *mut gtk::ffi::GtkWidget;
    fn vte_terminal_set_font(t: *mut Terminal, font: *const pango::ffi::PangoFontDescription);
    fn vte_terminal_set_colors(
        t: *mut Terminal,
        fg: *const gdk::ffi::GdkRGBA,
        bg: *const gdk::ffi::GdkRGBA,
        palette: *const gdk::ffi::GdkRGBA,
        count: usize,
    );
    fn vte_terminal_set_scrollback_lines(t: *mut Terminal, lines: libc::c_long);
    fn vte_terminal_set_cursor_shape(t: *mut Terminal, shape: c_int);
    fn vte_terminal_set_cursor_blink_mode(t: *mut Terminal, mode: c_int);
    fn vte_terminal_set_audible_bell(t: *mut Terminal, enabled: c_int);
    fn vte_terminal_set_scroll_on_output(t: *mut Terminal, enabled: c_int);
    fn vte_terminal_set_scroll_on_keystroke(t: *mut Terminal, enabled: c_int);
    fn vte_terminal_set_mouse_autohide(t: *mut Terminal, enabled: c_int);
    fn vte_terminal_get_current_directory_uri(t: *mut Terminal) -> *const c_char;
    fn vte_terminal_copy_clipboard_format(t: *mut Terminal, format: c_int);
    fn vte_terminal_paste_clipboard(t: *mut Terminal);
    fn vte_terminal_feed(t: *mut Terminal, text: *const c_char, len: isize);
    fn vte_terminal_spawn_async(
        t: *mut Terminal,
        flags: c_int,
        cwd: *const c_char,
        argv: *mut *mut c_char,
        env: *mut *mut c_char,
        spawn_flags: c_int,
        setup: Option<unsafe extern "C" fn(*mut c_void)>,
        setup_data: *mut c_void,
        destroy: Option<unsafe extern "C" fn(*mut c_void)>,
        timeout: c_int,
        cancellable: *mut gio::ffi::GCancellable,
        callback: Option<
            unsafe extern "C" fn(*mut Terminal, c_int, *mut glib::ffi::GError, *mut c_void),
        >,
        data: *mut c_void,
    );
}
type Completion = Box<dyn Fn(String)>;
struct Inner {
    widget: gtk::Widget,
    cancel: gio::Cancellable,
    pending: Cell<bool>,
    started: Cell<bool>,
    closed: Cell<bool>,
    completion: RefCell<Option<Completion>>,
}
#[derive(Clone)]
pub struct Session(Rc<Inner>);
impl Session {
    pub fn new(settings: &Settings) -> Self {
        assert!(gtk::is_initialized_main_thread());
        // GTK constructors return a floating reference; from_glib_none sinks it.
        let widget = unsafe { from_glib_none(vte_terminal_new()) };
        let session = Self(Rc::new(Inner {
            widget,
            cancel: gio::Cancellable::new(),
            pending: Cell::new(false),
            started: Cell::new(false),
            closed: Cell::new(false),
            completion: RefCell::new(None),
        }));
        session.apply(settings);
        let weak = Rc::downgrade(&session.0);
        session
            .widget()
            .connect_local("child-exited", false, move |args| {
                if let Some(inner) = weak.upgrade() {
                    let s = Session(inner);
                    if !s.0.closed.get() {
                        if let Some(done) = s.0.completion.borrow_mut().take() {
                            let status = args[1].get::<i32>().unwrap_or(-1);
                            let result = if status >= 0 && libc::WIFEXITED(status) {
                                let code = libc::WEXITSTATUS(status);
                                if code == 0 {
                                    "Done".to_owned()
                                } else {
                                    format!("Exit {code}")
                                }
                            } else if status >= 0 && libc::WIFSIGNALED(status) {
                                format!("Signal {}", libc::WTERMSIG(status))
                            } else {
                                "Failed".to_owned()
                            };
                            s.feed(&format!("\r\n[Task: {result}. Logs remain here.]\r\n"));
                            done(result);
                        } else {
                            s.feed("\r\n[Session ended. Open a new tab to start a shell.]\r\n");
                        }
                    }
                }
                None
            });
        let weak = Rc::downgrade(&session.0);
        session
            .widget()
            .connect_button_press_event(move |_, event| {
                if event.button() != 3 {
                    return glib::Propagation::Proceed;
                }
                if let Some(inner) = weak.upgrade() {
                    let menu = gtk::Menu::new();
                    for (label, copy) in [("Copy", true), ("Paste", false)] {
                        let item = gtk::MenuItem::with_label(label);
                        let s = Session(inner.clone());
                        item.connect_activate(move |_| {
                            if copy {
                                s.copy();
                            } else {
                                s.paste();
                            }
                        });
                        menu.append(&item);
                    }
                    menu.connect_selection_done(|m| unsafe {
                        m.destroy();
                    });
                    menu.show_all();
                    menu.popup_at_pointer(Some(event));
                }
                glib::Propagation::Stop
            });
        session
    }
    pub fn widget(&self) -> &gtk::Widget {
        &self.0.widget
    }
    fn ptr(&self) -> *mut Terminal {
        self.widget().as_ptr().cast()
    }
    pub fn apply(&self, s: &Settings) {
        if self.0.closed.get() {
            return;
        }
        let font = pango::FontDescription::from_string(&s.font);
        let fg = gdk::RGBA::parse(&s.foreground).expect("validated color");
        let mut bg = gdk::RGBA::parse(&s.background).expect("validated color");
        let image = !s.background_image.is_empty();
        if image {
            bg.set_alpha(0.0);
        }
        let colors: Vec<gdk::ffi::GdkRGBA> = s
            .palette
            .iter()
            .map(|c| {
                let color = gdk::RGBA::parse(c).expect("validated palette");
                unsafe { *color.to_glib_none().0 }
            })
            .collect();
        // All pointers are borrowed for these synchronous setter calls.
        unsafe {
            vte_terminal_set_clear_background(self.ptr(), (!image).into());
            vte_terminal_set_font(self.ptr(), font.to_glib_none().0);
            vte_terminal_set_colors(
                self.ptr(),
                fg.to_glib_none().0,
                bg.to_glib_none().0,
                colors.as_ptr(),
                colors.len(),
            );
            vte_terminal_set_scrollback_lines(self.ptr(), s.scrollback as libc::c_long);
            vte_terminal_set_cursor_shape(
                self.ptr(),
                match s.cursor.as_str() {
                    "ibeam" => 1,
                    "underline" => 2,
                    _ => 0,
                },
            );
            vte_terminal_set_cursor_blink_mode(self.ptr(), if s.blink { 1 } else { 2 });
            vte_terminal_set_audible_bell(self.ptr(), s.audible_bell.into());
            vte_terminal_set_scroll_on_output(self.ptr(), s.scroll_on_output.into());
            vte_terminal_set_scroll_on_keystroke(self.ptr(), s.scroll_on_keystroke.into());
            vte_terminal_set_mouse_autohide(self.ptr(), 1);
        }
        self.widget().set_margin_start(s.padding);
        self.widget().set_margin_end(s.padding);
        self.widget().set_margin_top(s.padding);
        self.widget().set_margin_bottom(s.padding);
    }
    pub fn start(&self, cwd: &str, settings: &Settings) {
        self.spawn(cwd, settings, None);
    }
    pub fn start_task(
        &self,
        cwd: &str,
        settings: &Settings,
        command: &str,
        done: impl Fn(String) + 'static,
    ) {
        *self.0.completion.borrow_mut() = Some(Box::new(done));
        self.feed(&format!("[Running project task]\r\n{command}\r\n\r\n"));
        self.spawn(cwd, settings, Some(command));
    }
    fn spawn(&self, cwd: &str, settings: &Settings, command: Option<&str>) {
        if self.0.closed.get() || self.0.started.replace(true) {
            return;
        }
        self.0.pending.set(true);
        let cwd = CString::new(if Path::new(cwd).is_dir() {
            cwd.to_owned()
        } else {
            home()
        })
        .unwrap_or_default();
        let shell = CString::new(user_shell(settings.shell())).expect("validated shell");
        let mut arguments = vec![shell];
        if let Some(command) = command {
            arguments.push(CString::new("-c").unwrap());
            arguments.push(CString::new(command).expect("validated task command"));
        }
        let mut argv: Vec<_> = arguments
            .iter()
            .map(|arg| arg.as_ptr() as *mut c_char)
            .collect();
        argv.push(std::ptr::null_mut());
        // Use GLib's environment: GTK may have removed variables since process startup.
        let mut env: Vec<CString> = glib::environ()
            .iter()
            .filter(|s| !s.to_string_lossy().starts_with("TERM="))
            .filter_map(|s| {
                use std::os::unix::ffi::OsStrExt;
                CString::new(s.as_bytes()).ok()
            })
            .collect();
        env.push(CString::new("TERM=xterm-256color").unwrap());
        let mut envp: Vec<_> = env.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        envp.push(std::ptr::null_mut());
        // VTE copies argv/env/cwd. The boxed Rc retains the widget until completion,
        // including when a tab is closed before its asynchronous spawn finishes.
        unsafe {
            vte_terminal_spawn_async(
                self.ptr(),
                0,
                cwd.as_ptr(),
                argv.as_mut_ptr(),
                envp.as_mut_ptr(),
                1 << 25,
                None,
                std::ptr::null_mut(),
                None,
                -1,
                self.0.cancel.to_glib_none().0,
                Some(spawned),
                Box::into_raw(Box::new(self.0.clone())).cast(),
            );
        }
    }
    pub fn directory(&self) -> Option<String> {
        if self.0.closed.get() {
            return None;
        }
        let uri = unsafe {
            let ptr = vte_terminal_get_current_directory_uri(self.ptr());
            if ptr.is_null() {
                return None;
            }
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        };
        let path = gio::File::for_uri(&uri).path()?;
        path.is_dir().then(|| path.to_string_lossy().into_owned())
    }
    pub fn copy(&self) {
        if !self.0.closed.get() {
            unsafe {
                vte_terminal_copy_clipboard_format(self.ptr(), 1);
            }
        }
    }
    pub fn paste(&self) {
        if !self.0.closed.get() {
            unsafe {
                vte_terminal_paste_clipboard(self.ptr());
            }
        }
    }
    fn feed(&self, text: &str) {
        unsafe {
            vte_terminal_feed(self.ptr(), text.as_ptr().cast(), text.len() as isize);
        }
    }
    pub fn close(&self) {
        if self.0.closed.replace(true) {
            return;
        }
        self.0.cancel.cancel();
        if let Some(parent) = self
            .widget()
            .parent()
            .and_then(|p| p.downcast::<gtk::Container>().ok())
        {
            parent.remove(self.widget());
        }
        if !self.0.pending.get() {
            unsafe {
                self.widget().destroy();
            }
        }
    }
}
unsafe extern "C" fn spawned(
    _: *mut Terminal,
    _: c_int,
    error: *mut glib::ffi::GError,
    data: *mut c_void,
) {
    // Exactly one callback owns the Box passed to VTE.
    let inner = *Box::from_raw(data.cast::<Rc<Inner>>());
    inner.pending.set(false);
    let session = Session(inner);
    if session.0.closed.get() {
        session.widget().destroy();
    } else if !error.is_null() {
        if let Some(done) = session.0.completion.borrow_mut().take() {
            done("Launch failed".into());
        }
        session.feed(&format!(
            "Could not start shell: {}\r\n",
            CStr::from_ptr((*error).message).to_string_lossy()
        ));
    }
}
pub fn user_shell(configured: &str) -> String {
    use std::os::unix::fs::PermissionsExt;
    let mut candidates = vec![
        configured.to_owned(),
        std::env::var("SHELL").unwrap_or_default(),
    ];
    // getpwuid is used only on GTK's main thread and copied immediately.
    unsafe {
        let p = libc::getpwuid(libc::getuid());
        if !p.is_null() && !(*p).pw_shell.is_null() {
            candidates.push(CStr::from_ptr((*p).pw_shell).to_string_lossy().into_owned());
        }
    }
    candidates.extend(["/bin/bash".into(), "/bin/sh".into()]);
    candidates
        .into_iter()
        .find(|s| {
            !s.contains('\0')
                && Path::new(s).is_file()
                && std::fs::metadata(s).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
        })
        .unwrap_or_else(|| "/bin/sh".into())
}

#[cfg(test)]
impl Session {
    pub fn send(&self, text: &str) {
        unsafe extern "C" {
            fn vte_terminal_feed_child(t: *mut Terminal, text: *const c_char, len: isize);
        }
        unsafe {
            vte_terminal_feed_child(self.ptr(), text.as_ptr().cast(), text.len() as isize);
        }
    }
}
