use glib::subclass::Signal;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{Box, Button, Label, ListBox, Orientation, ScrolledWindow};
use std::cell::RefCell;
use std::sync::OnceLock;

use crate::services::bookmarks::BookmarkEntry;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct TocPanel {
        pub list_box: ListBox,
        pub close_button: Button,
        pub entries: RefCell<Vec<BookmarkEntry>>,
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

        imp.entries.borrow_mut().clear();

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
            for entry in entries {
                self.append_entry(entry, 0);
            }
        }

        imp.entries.borrow_mut().extend_from_slice(entries);
    }

    fn append_entry(&self, entry: &BookmarkEntry, depth: usize) {
        let imp = self.imp();

        let row = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(4)
            .margin_start(12 + (depth * 16) as i32)
            .margin_end(12)
            .margin_top(4)
            .margin_bottom(4)
            .build();

        let label = Label::new(Some(&entry.title));
        label.set_xalign(0.0);
        label.set_hexpand(true);
        label.add_css_class("toc-entry");
        row.append(&label);

        imp.list_box.append(&row);

        if !entry.children.is_empty() {
            for child in &entry.children {
                self.append_entry(child, depth + 1);
            }
        }

        let _page_index = entry.page_index as u32;

        let panel_weak = self.downgrade();
        imp.list_box.connect_row_activated(move |_, row| {
            if let Some(panel) = panel_weak.upgrade() {
                let entries = panel.imp().entries.borrow();
                let pos = row.index();
                if let Some(entry) = entries.get(pos as usize) {
                    panel.emit_by_name::<()>("chapter-selected", &[&(entry.page_index as u32)]);
                }
            }
        });
    }

    pub fn clear(&self) {
        let imp = self.imp();
        while let Some(row) = imp.list_box.first_child() {
            imp.list_box.remove(&row);
        }
        imp.entries.borrow_mut().clear();
    }
}

impl Default for TocPanel {
    fn default() -> Self {
        Self::new()
    }
}
