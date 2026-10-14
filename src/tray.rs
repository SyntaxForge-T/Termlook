//! Optional AppIndicator: load it only when installed and a panel host exists.
use glib::translate::*;
use gtk::prelude::*;
use libloading::Library;
use std::{
    cell::{Cell, RefCell},
    ffi::CString,
    rc::Rc,
};

type New = unsafe extern "C" fn(
    *const libc::c_char,
    *const libc::c_char,
    i32,
) -> *mut glib::gobject_ffi::GObject;
type Status = unsafe extern "C" fn(*mut glib::gobject_ffi::GObject, i32);
type Menu = unsafe extern "C" fn(*mut glib::gobject_ffi::GObject, *mut gtk::ffi::GtkMenu);
struct Indicator {
    object: glib::Object,
    status: Status,
    // GObject keeps registered class vtables for the process lifetime.
    _library: std::mem::ManuallyDrop<Library>,
}
impl Indicator {
    fn new(menu: &gtk::Menu) -> Option<Self> {
        for name in ["libayatana-appindicator3.so.1", "libappindicator3.so.1"] {
            // Symbols have the stable AppIndicator C ABI. The library outlives the object.
            unsafe {
                let Ok(library) = Library::new(name) else {
                    continue;
                };
                let new = *library.get::<New>(b"app_indicator_new\0").ok()?;
                let status = *library.get::<Status>(b"app_indicator_set_status\0").ok()?;
                let set_menu = *library.get::<Menu>(b"app_indicator_set_menu\0").ok()?;
                let id = CString::new("termlook").unwrap();
                let icon = CString::new("utilities-terminal").unwrap();
                let ptr = new(id.as_ptr(), icon.as_ptr(), 0);
                if ptr.is_null() {
                    return None;
                }
                let object: glib::Object = from_glib_full(ptr);
                set_menu(ptr, menu.to_glib_none().0);
                status(ptr, 1);
                return Some(Self {
                    object,
                    status,
                    _library: std::mem::ManuallyDrop::new(library),
                });
            }
        }
        None
    }
    fn active(&self, active: bool) {
        unsafe {
            (self.status)(self.object.as_ptr(), active.into());
        }
    }
}
pub struct Tray {
    unwatch: Option<Box<dyn FnOnce()>>,
    receiver: Option<glib::SourceId>,
    indicator: Rc<RefCell<Option<Indicator>>>,
    pub available: Rc<Cell<bool>>,
    menu: gtk::Menu,
}
impl Tray {
    #[allow(deprecated)] // GLib main-context channel bridges Send D-Bus callbacks to GTK.
    pub fn new(menu: gtk::Menu, unavailable: impl Fn() + 'static) -> Self {
        let indicator = Rc::new(RefCell::new(None::<Indicator>));
        let available = Rc::new(Cell::new(false));
        let (sender, receiver) = glib::MainContext::channel(glib::Priority::DEFAULT);
        let i = indicator.clone();
        let a = available.clone();
        let m = menu.clone();
        let receiver = receiver.attach(None, move |present| {
            if present && i.borrow().is_none() {
                *i.borrow_mut() = Indicator::new(&m);
            }
            if let Some(indicator) = i.borrow().as_ref() {
                indicator.active(present);
            }
            let was = a.replace(present && i.borrow().is_some());
            if was && !a.get() {
                unavailable();
            }
            glib::ControlFlow::Continue
        });
        let appeared = sender.clone();
        let watch = gio::bus_watch_name(
            gio::BusType::Session,
            "org.kde.StatusNotifierWatcher",
            gio::BusNameWatcherFlags::NONE,
            move |_, _, _| {
                let _ = appeared.send(true);
            },
            move |_, _| {
                let _ = sender.send(false);
            },
        );
        Self {
            unwatch: Some(Box::new(move || gio::bus_unwatch_name(watch))),
            receiver: Some(receiver),
            indicator,
            available,
            menu,
        }
    }
}
impl Drop for Tray {
    fn drop(&mut self) {
        if let Some(unwatch) = self.unwatch.take() {
            unwatch();
        }
        if let Some(r) = self.receiver.take() {
            r.remove();
        }
        if let Some(i) = self.indicator.borrow_mut().take() {
            i.active(false);
        }
        unsafe {
            self.menu.destroy();
        }
    }
}
