use crate::ui::{enqueue, State};
use gtk::prelude::*;
use termlook::model::Id;

#[derive(Clone, Copy)]
pub enum Target {
    Tab(Id),
    Pane { tab: Id, pane: Id },
    Workspace(Id),
    Quit,
}
impl State {
    pub fn request_close(&mut self, target: Target) {
        if let Some(dialog) = &self.confirmation {
            dialog.present();
            return;
        }
        let description = match target {
            Target::Tab(id) => self
                .layout
                .workspaces
                .iter()
                .flat_map(|w| &w.tabs)
                .find(|t| t.id == id)
                .map(|t| {
                    format!(
                        "Close tab “{}” and its {} terminal pane(s)?",
                        t.name,
                        t.panes.len()
                    )
                }),
            Target::Pane { tab, pane } => self
                .layout
                .workspaces
                .iter()
                .flat_map(|w| &w.tabs)
                .find(|t| t.id == tab && t.panes.iter().any(|p| p.id == pane))
                .map(|t| format!("Close this terminal pane in “{}”?", t.name)),
            Target::Workspace(id) => self.layout.workspaces.iter().find(|w| w.id == id).map(|w| {
                format!(
                    "Delete workspace “{}” and all {} tab(s)?",
                    w.name,
                    w.tabs.len()
                )
            }),
            Target::Quit => Some("Quit TermLook and close all terminal sessions?".into()),
        };
        let Some(description) = description else {
            return;
        };
        let dialog = gtk::MessageDialog::new(
            Some(&self.window),
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            gtk::MessageType::Warning,
            gtk::ButtonsType::None,
            &description,
        );
        dialog.set_title("Confirm close");
        dialog.set_secondary_text(Some(
            "Running commands in these terminals will be stopped. Unsaved work may be lost.",
        ));
        dialog.add_button("Cancel", gtk::ResponseType::Cancel);
        let close = dialog.add_button("Close sessions", gtk::ResponseType::Accept);
        close.style_context().add_class("destructive-action");
        dialog.set_default_response(gtk::ResponseType::Cancel);
        if let Some(cancel) = dialog.widget_for_response(gtk::ResponseType::Cancel) {
            cancel.grab_focus();
        }
        let weak = self.weak.clone();
        dialog.connect_response(move |d, response| {
            unsafe {
                d.destroy();
            }
            enqueue(&weak, move |s| {
                s.confirmation = None;
                if response == gtk::ResponseType::Accept {
                    s.finish_close(target);
                }
            });
        });
        self.confirmation = Some(dialog.clone());
        dialog.show_all();
    }
    fn finish_close(&mut self, target: Target) {
        match target {
            Target::Tab(id) => self.layout.close_tab(id),
            Target::Workspace(id) => self.layout.remove_workspace(id),
            Target::Pane { tab, pane } => {
                if let Some(t) = self.layout.find_tab_mut(tab) {
                    if !t.panes.iter().any(|p| p.id == pane) {
                        return;
                    }
                    if t.panes.len() == 1 {
                        self.layout.close_tab(tab);
                    } else {
                        t.close_pane(pane);
                    }
                }
            }
            Target::Quit => {
                self.shutdown();
                unsafe {
                    self.window.destroy();
                }
                if let Some(app) = self.window.application() {
                    app.quit();
                }
                return;
            }
        }
        self.render();
    }
}
