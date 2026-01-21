use glib::subclass::Signal;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{Box, Button, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow};
use std::cell::Cell;
use std::sync::OnceLock;

use crate::services::bookmarks::BookmarkEntry;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct TocEntryRow {
        pub page_index: Cell<u16>,
        pub depth: Cell<usize>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TocEntryRow {
        const NAME: &'static str = "TocEntryRow";
        type Type = super::TocEntryRow;
        type ParentType = ListBoxRow;
    }

    impl ObjectImpl for TocEntryRow {}
    impl WidgetImpl for TocEntryRow {}
    impl ListBoxRowImpl for TocEntryRow {}

    #[derive(Default)]
    pub struct TocPanel {
        pub list_box: ListBox,
        pub close_button: Button,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TocPanel {
        const NAME: &'static str = "TocPanel";
        type Type = super::TocPanel;
        type ParentType = Box;
    }

    impl ObjectImpl for TocPanel {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_widgets();
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("chapter-selected")
                        .param_types([u32::static_type()])
                        .build(),
                ]
            })
        }
    }

    impl WidgetImpl for TocPanel {}
    impl BoxImpl for TocPanel {}
}

glib::wrapper! {
    pub struct TocEntryRow(ObjectSubclass<imp::TocEntryRow>)
        @extends ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Actionable;
}

impl TocEntryRow {
    pub fn new(page_index: u16, title: &str, depth: usize) -> Self {
        let row: TocEntryRow = glib::Object::builder().build();
        row.imp().page_index.set(page_index);
        row.imp().depth.set(depth);

        let container = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(4)
            .margin_start(12 + (depth * 16) as i32)
            .margin_end(12)
            .margin_top(4)
            .margin_bottom(4)
            .build();

        let label = Label::new(Some(title));
        label.set_xalign(0.0);
        label.set_hexpand(true);
        label.add_css_class("toc-entry");
        container.append(&label);

        row.set_child(Some(&container));
        row
    }

    pub fn page_index(&self) -> u16 {
        self.imp().page_index.get()
    }

    pub fn depth(&self) -> usize {
        self.imp().depth.get()
    }
}

glib::wrapper! {
    pub struct TocPanel(ObjectSubclass<imp::TocPanel>)
        @extends Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl TocPanel {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    fn setup_widgets(&self) {
        let imp = self.imp();

        self.set_orientation(Orientation::Vertical);
        self.set_spacing(0);
        self.set_size_request(250, -1);
        self.set_visible(false);

        let header_box = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .margin_start(12)
            .margin_end(12)
            .margin_top(12)
            .margin_bottom(8)
            .build();

        let title_label = Label::new(Some("Contents"));
        title_label.set_hexpand(true);
        title_label.add_css_class("heading");
        header_box.append(&title_label);

        imp.close_button.set_icon_name("window-close-symbolic");
        imp.close_button.add_css_class("flat");
        header_box.append(&imp.close_button);

        self.append(&header_box);

        let scrolled_window = ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();

        imp.list_box.set_selection_mode(gtk::SelectionMode::Single);
        imp.list_box.add_css_class("toc-list");
        scrolled_window.set_child(Some(&imp.list_box));

        self.append(&scrolled_window);

        self.add_css_class("toc-panel");

        let panel_weak = self.downgrade();
        imp.list_box.connect_row_activated(move |_, row| {
            if let Some(panel) = panel_weak.upgrade() {
                if let Some(entry_row) = row.downcast_ref::<TocEntryRow>() {
                    panel.emit_by_name::<()>(
                        "chapter-selected",
                        &[&(entry_row.page_index() as u32)],
                    );
                }
            }
        });
    }

    pub fn close_button(&self) -> &Button {
        &self.imp().close_button
    }

    pub fn list_box(&self) -> &ListBox {
        &self.imp().list_box
    }

    pub fn populate(&self, entries: &[BookmarkEntry]) {
        let imp = self.imp();

        while let Some(row) = imp.list_box.first_child() {
            imp.list_box.remove(&row);
        }

        if entries.is_empty() {
            let label = Label::new(Some("No chapters found"));
            label.set_margin_start(12);
            label.set_margin_end(12);
            label.set_margin_top(12);
            label.set_margin_bottom(12);
            label.set_xalign(0.0);
            label.set_opacity(0.6);
            imp.list_box.append(&label);
        } else {
            self.flatten_entries(entries, 0);
        }
    }

    fn flatten_entries(&self, entries: &[BookmarkEntry], initial_depth: usize) {
        for entry in entries {
            self.add_entry_row(entry, initial_depth);
            if !entry.children.is_empty() {
                self.flatten_entries(&entry.children, initial_depth + 1);
            }
        }
    }

    fn add_entry_row(&self, entry: &BookmarkEntry, depth: usize) {
        let imp = self.imp();

        let entry_row = TocEntryRow::new(entry.page_index, &entry.title, depth);
        imp.list_box.append(&entry_row);
    }

    pub fn select_current_chapter(&self, page: u16) {
        let imp = self.imp();
        let children = imp.list_box.observe_children();

        let mut best_match: Option<glib::Object> = None;
        let mut best_page_index: u16 = 0;

        for item in children.iter::<glib::Object>() {
            match item {
                Ok(child) => {
                    if let Some(entry_row) = child.downcast_ref::<TocEntryRow>() {
                        let entry_page = entry_row.page_index();
                        if entry_page <= page && entry_page >= best_page_index {
                            best_match = Some(child.clone());
                            best_page_index = entry_page;
                        }
                    }
                }
                Err(_) => {
                    break;
                }
            }
        }

        if let Some(row_obj) = best_match {
            if let Some(row) = row_obj.downcast_ref::<ListBoxRow>() {
                imp.list_box.select_row(Some(row));
                row.grab_focus();
            }
        }
    }

    pub fn select_first(&self) {
        if let Some(first_child) = self.imp().list_box.first_child() {
            if let Some(list_row) = first_child.downcast_ref::<ListBoxRow>() {
                self.imp().list_box.select_row(Some(list_row));
                list_row.grab_focus();
            }
        }
    }

    pub fn select_next(&self) {
        if let Some(current) = self.imp().list_box.selected_row() {
            if let Some(next_widget) = current.next_sibling() {
                if let Some(next) = next_widget.downcast_ref::<ListBoxRow>() {
                    self.imp().list_box.select_row(Some(next));
                    next.grab_focus();
                }
            }
        }
    }

    pub fn select_prev(&self) {
        if let Some(current) = self.imp().list_box.selected_row() {
            if let Some(prev_widget) = current.prev_sibling() {
                if let Some(prev) = prev_widget.downcast_ref::<ListBoxRow>() {
                    self.imp().list_box.select_row(Some(prev));
                    prev.grab_focus();
                }
            }
        }
    }

    pub fn navigate_and_close(&self) {
        if let Some(row) = self.imp().list_box.selected_row() {
            if let Some(entry_row) = row.downcast_ref::<TocEntryRow>() {
                self.emit_by_name::<()>("chapter-selected", &[&(entry_row.page_index() as u32)]);
                self.set_visible(false);
            }
        }
    }

    pub fn clear(&self) {
        let imp = self.imp();
        while let Some(row) = imp.list_box.first_child() {
            imp.list_box.remove(&row);
        }
    }
}

impl Default for TocPanel {
    fn default() -> Self {
        Self::new()
    }
}
