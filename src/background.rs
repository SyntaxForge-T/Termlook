//! One bounded image shared behind the entire terminal stack, never per pane.
use gdk::prelude::*;
use gtk::gdk_pixbuf::Pixbuf;
use gtk::prelude::*;
use std::{cell::RefCell, rc::Rc};
use termlook::settings::Settings;

pub fn load(settings: &Settings) -> Result<Option<Pixbuf>, String> {
    if settings.background_image.is_empty() {
        return Ok(None);
    }
    let path = std::path::Path::new(&settings.background_image);
    let metadata = std::fs::metadata(path).map_err(|e| format!("Background image: {e}"))?;
    if !metadata.is_file() || metadata.len() > 64 * 1024 * 1024 {
        return Err("Choose an image file smaller than 64 MiB".into());
    }
    let (_, width, height) =
        Pixbuf::file_info(path).ok_or("Unsupported or damaged background image")?;
    if width <= 0 || height <= 0 || i64::from(width) * i64::from(height) > 100_000_000 {
        return Err("Background image must be at most 100 megapixels".into());
    }
    Pixbuf::from_file_at_scale(path, 3840, 2160, true)
        .map(Some)
        .map_err(|e| format!("Background image: {e}"))
}
struct Paint {
    image: Option<Pixbuf>,
    color: gdk::RGBA,
    dim: f64,
    mode: String,
}
pub struct Backdrop {
    pub widget: gtk::Overlay,
    area: gtk::DrawingArea,
    paint: Rc<RefCell<Paint>>,
}
impl Backdrop {
    pub fn new(stack: &gtk::Stack, settings: &Settings, image: Option<Pixbuf>) -> Self {
        let widget = gtk::Overlay::new();
        let area = gtk::DrawingArea::new();
        area.set_hexpand(true);
        area.set_vexpand(true);
        widget.add(&area);
        widget.add_overlay(stack);
        stack.set_hexpand(true);
        stack.set_vexpand(true);
        stack.style_context().add_class("terminal-stack");
        let paint = Rc::new(RefCell::new(Paint {
            image,
            color: gdk::RGBA::parse(&settings.background).unwrap(),
            dim: f64::from(settings.background_dim) / 100.0,
            mode: settings.background_fit.clone(),
        }));
        let data = paint.clone();
        area.connect_draw(move |area, cr| {
            let p = data.borrow();
            let w = f64::from(area.allocated_width());
            let h = f64::from(area.allocated_height());
            cr.set_source_rgb(p.color.red(), p.color.green(), p.color.blue());
            let _ = cr.paint();
            if let Some(image) = &p.image {
                let iw = f64::from(image.width());
                let ih = f64::from(image.height());
                let (sx, sy) = match p.mode.as_str() {
                    "stretch" => (w / iw, h / ih),
                    "contain" => {
                        let s = (w / iw).min(h / ih);
                        (s, s)
                    }
                    _ => {
                        let s = (w / iw).max(h / ih);
                        (s, s)
                    }
                };
                if sx > 0.0 && sy > 0.0 && cr.save().is_ok() {
                    cr.translate((w - iw * sx) / 2.0, (h - ih * sy) / 2.0);
                    cr.scale(sx, sy);
                    cr.set_source_pixbuf(image, 0.0, 0.0);
                    let _ = cr.paint();
                    let _ = cr.restore();
                }
                cr.set_source_rgba(p.color.red(), p.color.green(), p.color.blue(), p.dim);
                let _ = cr.paint();
            }
            glib::Propagation::Proceed
        });
        Self {
            widget,
            area,
            paint,
        }
    }
    pub fn apply(&self, settings: &Settings, image: Option<Pixbuf>) {
        *self.paint.borrow_mut() = Paint {
            image,
            color: gdk::RGBA::parse(&settings.background).unwrap(),
            dim: f64::from(settings.background_dim) / 100.0,
            mode: settings.background_fit.clone(),
        };
        self.area.queue_draw();
    }
}
