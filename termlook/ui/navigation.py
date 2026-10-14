"""Navigation widgets own all row/header references; models own none."""
from .gtk import Gdk, Gtk, Pango
from .widgets import button


def title_button(name, select, rename, dispatch):
    widget = button(name, 'Double-click to rename', lambda *_: dispatch(select))
    label = widget.get_child()
    label.set_ellipsize(Pango.EllipsizeMode.END)
    label.set_max_width_chars(20)
    label.set_xalign(0)

    def pressed(_widget, event):
        if event.button == 1 and event.type == Gdk.EventType.DOUBLE_BUTTON_PRESS:
            dispatch(rename)
            return True
        return False

    widget.connect('button-press-event', pressed)
    return widget


class Navigation:
    def __init__(self, sidebar, tabs, controller, dispatch):
        self.sidebar = sidebar
        self.tabs = tabs
        self.controller = controller
        self.dispatch = dispatch
        self.rows = {}
        self.headers = {}
        self.sidebar_signature = None
        self.tabs_signature = None
        self.handler = sidebar.connect('row-selected', self._selected)

    def _selected(self, _list, row):
        if row:
            self.dispatch(lambda: self.controller.select_workspace(row.workspace_id))

    def render(self, layout):
        # Selection-only changes must not destroy the button receiving a double click.
        sidebar_signature = tuple((w.id, w.name) for w in layout.workspaces)
        tabs_signature = (layout.selected.id, tuple((t.id, t.name) for t in layout.selected.tabs))
        if sidebar_signature != self.sidebar_signature:
            self.sidebar.handler_block(self.handler)
            try:
                for row in self.sidebar.get_children():
                    row.destroy()
                self.rows.clear()
                for workspace in layout.workspaces:
                    wid = workspace.id
                    row = Gtk.ListBoxRow()
                    row.workspace_id = wid
                    box = Gtk.Box()
                    box.pack_start(title_button(workspace.name,
                        lambda wid=wid: self.controller.select_workspace(wid),
                        lambda wid=wid: self.controller.rename_workspace(wid), self.dispatch), True, True, 0)
                    box.pack_end(button('×', 'Delete workspace',
                        lambda _, wid=wid: self.dispatch(lambda: self.controller.remove_workspace(wid))), False, False, 0)
                    row.add(box)
                    self.sidebar.add(row)
                    self.rows[wid] = row
                self.sidebar.show_all()
                self.sidebar_signature = sidebar_signature
            finally:
                self.sidebar.handler_unblock(self.handler)
        if tabs_signature != self.tabs_signature:
            for child in self.tabs.get_children():
                child.destroy()
            self.headers.clear()
            for tab in layout.selected.tabs:
                tid = tab.id
                box = Gtk.Box()
                box.get_style_context().add_class('terminal-tab')
                box.pack_start(title_button(tab.name,
                    lambda tid=tid: self.controller.select_tab(tid),
                    lambda tid=tid: self.controller.rename_tab(tid), self.dispatch), False, False, 0)
                # box.pack_start(button('×', 'Close tab',
                #     lambda _, tid=tid: self.dispatch(lambda: self.controller.close_tab(tid))), False, False, 0)
                self.tabs.pack_start(box, False, False, 0)
                self.headers[tid] = box
            self.tabs.show_all()
            self.tabs_signature = tabs_signature
        self.sidebar.handler_block(self.handler)
        try:
            self.sidebar.select_row(self.rows[layout.selected.id])
        finally:
            self.sidebar.handler_unblock(self.handler)
        for tab in layout.selected.tabs:
            header = self.headers[tab.id]
            header.set_tooltip_text(tab.cwd)
            context = header.get_style_context()
            if tab is layout.selected.selected:
                context.add_class('active')
            else:
                context.remove_class('active')
