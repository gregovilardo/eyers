use crate::modes::WordCursor;
use crate::services::annotations::Annotation;
use glib::subclass::Signal;
use gtk::Stack;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{Box, Button, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow};
use std::cell::Cell;
use std::sync::OnceLock;

use crate::services::bookmarks::BookmarkEntry;

#[derive(Default, Copy, Clone)]
pub enum TocMode {
    Annotations,
    #[default]
    Chapters,
}

mod imp {

    use std::cell::RefCell;

    use crate::modes::WordCursor;

    use super::*;

    #[derive(Default)]
    pub struct TocChapterRow {
        pub page_index: Cell<u32>,
        pub depth: Cell<usize>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TocChapterRow {
        const NAME: &'static str = "TocChapterRow";
        type Type = super::TocChapterRow;
        type ParentType = ListBoxRow;
    }

    impl ObjectImpl for TocChapterRow {}
    impl WidgetImpl for TocChapterRow {}
    impl ListBoxRowImpl for TocChapterRow {}

    #[derive(Default)]
    pub struct TocAnnotationRow {
        pub page_index: Cell<u32>,
        pub annotation: RefCell<Annotation>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TocAnnotationRow {
        const NAME: &'static str = "TocAnnotationRow";
        type Type = super::TocAnnotationRow;
        type ParentType = ListBoxRow;
    }

    impl ObjectImpl for TocAnnotationRow {}
    impl WidgetImpl for TocAnnotationRow {}
    impl ListBoxRowImpl for TocAnnotationRow {}

    #[derive(Default)]
    pub struct TocPanel {
        pub title: Label,
        pub mode: Cell<TocMode>,
        pub stack: Stack,
        pub list_box_annotations: ListBox,
        pub list_box_chapters: ListBox,
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
                    Signal::builder("toc-entry-selected")
                        .param_types([u32::static_type(), WordCursor::static_type()])
                        .build(),
                ]
            })
        }
    }

    impl WidgetImpl for TocPanel {}
    impl BoxImpl for TocPanel {}
}

glib::wrapper! {
    pub struct TocChapterRow(ObjectSubclass<imp::TocChapterRow>)
        @extends ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Actionable;
}

impl TocChapterRow {
    pub fn new(page_index: u32, title: &str, depth: usize) -> Self {
        let row: TocChapterRow = glib::Object::builder().build();
        row.imp().page_index.set(page_index);
        row.imp().depth.set(depth);

        let container = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(4)
            .margin_start(12 + (depth * 16) as i32)
            .margin_end(12)
            .margin_top(4)
            .margin_bottom(4)
            .hexpand(true)
            .build();

        let label = Label::new(Some(title));
        label.set_xalign(0.0);
        label.set_hexpand(true);
        label.add_css_class("toc-title");
        container.append(&label);

        let label = Label::new(Some(&page_index.to_string()));
        label.set_xalign(0.0);
        label.set_hexpand(false);
        label.add_css_class("toc-page-index");
        container.append(&label);

        row.set_child(Some(&container));

        row
    }

    pub fn page_index(&self) -> u32 {
        self.imp().page_index.get()
    }

    pub fn depth(&self) -> usize {
        self.imp().depth.get()
    }
}

glib::wrapper! {
    pub struct TocAnnotationRow(ObjectSubclass<imp::TocAnnotationRow>)
        @extends ListBoxRow, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Actionable;
}

impl TocAnnotationRow {
    pub fn new(annotation: Annotation) -> Self {
        let page_index = &annotation.start_page;
        let title = &annotation.selected_text;
        let sub_title = &annotation.note;
        let row: TocAnnotationRow = glib::Object::builder().build();

        let container = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(4)
            .margin_start(12 as i32)
            .margin_end(12)
            .margin_top(4)
            .margin_bottom(4)
            .build();

        let sub_container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(2)
            .build();

        let label = Label::new(Some(title));
        label.set_xalign(0.0);
        label.set_hexpand(true);
        label.add_css_class("toc-annotation-title");
        sub_container.append(&label);

        let label = Label::new(Some(sub_title));
        label.set_xalign(0.1);
        label.set_hexpand(true);
        label.add_css_class("toc-subtitle");
        sub_container.append(&label);

        container.append(&sub_container);

        let label = Label::new(Some(&page_index.to_string()));
        label.set_xalign(0.0);
        label.set_hexpand(false);
        label.add_css_class("toc-page-index");
        container.append(&label);

        row.set_child(Some(&container));

        row.imp().annotation.replace(annotation);

        row
    }

    pub fn page_index(&self) -> usize {
        self.imp().annotation.borrow().start_page
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

        let title_label = &self.imp().title;
        title_label.set_text("Chapters");
        title_label.set_hexpand(true);
        title_label.add_css_class("heading");
        header_box.append(title_label);

        imp.close_button.set_icon_name("window-close-symbolic");
        imp.close_button.add_css_class("flat");
        header_box.append(&imp.close_button);

        self.append(&header_box);

        let scrolled_window = ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();

        imp.list_box_chapters
            .set_selection_mode(gtk::SelectionMode::Single);
        imp.list_box_chapters.add_css_class("toc-list");

        imp.list_box_annotations
            .set_selection_mode(gtk::SelectionMode::Single);
        imp.list_box_annotations.add_css_class("toc-list");

        let stack = &self.imp().stack;
        stack.add_named(&imp.list_box_chapters, Some("chapters"));
        stack.add_named(&imp.list_box_annotations, Some("annotations"));

        scrolled_window.set_child(Some(stack));

        self.append(&scrolled_window);
        self.add_css_class("toc-panel");

        let panel_weak = self.downgrade();
        imp.list_box_chapters.connect_row_activated(move |_, row| {
            if let Some(panel) = panel_weak.upgrade() {
                if let Some(entry_row) = row.downcast_ref::<TocChapterRow>() {
                    panel.emit_by_name::<()>(
                        "toc-entry-selected",
                        &[&(entry_row.page_index() as u32)],
                    );
                }
            }
        });

        let panel_weak = self.downgrade();
        imp.list_box_annotations
            .connect_row_activated(move |_, row| {
                if let Some(panel) = panel_weak.upgrade() {
                    if let Some(entry_row) = row.downcast_ref::<TocAnnotationRow>() {
                        panel.emit_by_name::<()>(
                            "toc-entry-selected",
                            &[
                                &(entry_row.page_index() as u32),
                                &(entry_row.imp().annotation.borrow().get_start_word_cursor()),
                            ],
                        );
                    }
                }
            });
    }

    pub fn set_toc_mode(&self, mode: TocMode) {
        let stack = &self.imp().stack;
        let title_label = &self.imp().title;
        self.imp().mode.set(mode);

        //This could be a signal ? no se si vale la pena
        match mode {
            TocMode::Chapters => {
                stack.set_visible_child_name("chapters");
                title_label.set_text("Chapters");
            }
            TocMode::Annotations => {
                stack.set_visible_child_name("annotations");
                title_label.set_text("Annotations");
            }
        }
    }

    pub fn toc_mode(&self) -> TocMode {
        self.imp().mode.get()
    }

    pub fn close_button(&self) -> &Button {
        &self.imp().close_button
    }

    pub fn populate_annotations(&self, entries: &[Annotation]) {
        let imp = self.imp();

        while let Some(row) = imp.list_box_annotations.first_child() {
            imp.list_box_annotations.remove(&row);
        }

        if entries.is_empty() {
            let label = Label::new(Some("No annotation found"));
            label.set_margin_start(12);
            label.set_margin_end(12);
            label.set_margin_top(12);
            label.set_margin_bottom(12);
            label.set_xalign(0.0);
            label.set_opacity(0.6);
            imp.list_box_annotations.append(&label);
        } else {
            for entry in entries {
                let entry_row = TocAnnotationRow::new(entry.clone());
                self.imp().list_box_annotations.append(&entry_row);
            }
        }
    }

    pub fn populate_chapters(&self, entries: &[BookmarkEntry]) {
        let imp = self.imp();

        while let Some(row) = imp.list_box_chapters.first_child() {
            imp.list_box_chapters.remove(&row);
        }

        if entries.is_empty() {
            let label = Label::new(Some("No chapters found"));
            label.set_margin_start(12);
            label.set_margin_end(12);
            label.set_margin_top(12);
            label.set_margin_bottom(12);
            label.set_xalign(0.0);
            label.set_opacity(0.6);
            imp.list_box_chapters.append(&label);
        } else {
            self.flatten_chapters_entries(entries, 0);
        }
    }

    fn flatten_chapters_entries(&self, entries: &[BookmarkEntry], initial_depth: usize) {
        for entry in entries {
            self.add_chapter_row(entry, initial_depth);
            if !entry.children.is_empty() {
                self.flatten_chapters_entries(&entry.children, initial_depth + 1);
            }
        }
    }

    fn add_chapter_row(&self, entry: &BookmarkEntry, depth: usize) {
        let imp = self.imp();

        let entry_row = TocChapterRow::new(entry.page_index, &entry.title, depth);
        imp.list_box_chapters.append(&entry_row);
    }

    pub fn select_current_chapter(&self, page: u32) {
        let imp = self.imp();
        let children = imp.list_box_chapters.observe_children();

        let mut best_match: Option<glib::Object> = None;
        let mut best_page_index: u32 = 0;

        for item in children.iter::<glib::Object>() {
            match item {
                Ok(child) => {
                    if let Some(entry_row) = child.downcast_ref::<TocChapterRow>() {
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
                imp.list_box_chapters.select_row(Some(row));
                row.grab_focus();
            }
        }
    }

    fn get_current_list_box(&self) -> &ListBox {
        match self.imp().mode.get() {
            TocMode::Chapters => &self.imp().list_box_chapters,
            TocMode::Annotations => &self.imp().list_box_annotations,
        }
    }

    pub fn select_first(&self) {
        let list_box = self.get_current_list_box();

        if let Some(first_child) = list_box.first_child() {
            if let Some(list_row) = first_child.downcast_ref::<ListBoxRow>() {
                list_box.select_row(Some(list_row));
                list_row.grab_focus();
            }
        }
    }

    pub fn select_next(&self) {
        let list_box = self.get_current_list_box();

        if let Some(current) = list_box.selected_row() {
            if let Some(next_widget) = current.next_sibling() {
                if let Some(next) = next_widget.downcast_ref::<ListBoxRow>() {
                    list_box.select_row(Some(next));
                    next.grab_focus();
                }
            }
        }
    }

    pub fn select_prev(&self) {
        let list_box = self.get_current_list_box();

        if let Some(current) = list_box.selected_row() {
            if let Some(prev_widget) = current.prev_sibling() {
                if let Some(prev) = prev_widget.downcast_ref::<ListBoxRow>() {
                    list_box.select_row(Some(prev));
                    prev.grab_focus();
                }
            }
        }
    }

    pub fn navigate_and_close(&self) {
        let list_box = self.get_current_list_box();

        if let Some(row) = list_box.selected_row() {
            if let Some(entry_row) = row.downcast_ref::<TocChapterRow>() {
                self.emit_by_name::<()>("toc-entry-selected", &[&(entry_row.page_index() as u32)]);
                self.set_visible(false);
            }
            if let Some(entry_row) = row.downcast_ref::<TocAnnotationRow>() {
                let cursor = Some(entry_row.imp().annotation.borrow().get_start_word_cursor());
                self.emit_by_name::<()>(
                    "toc-entry-selected",
                    &[&(entry_row.page_index() as u32), &cursor],
                );
                self.set_visible(false);
            }
        }
    }

    pub fn clear(&self) {
        let imp = self.imp();
        while let Some(row) = imp.list_box_chapters.first_child() {
            imp.list_box_chapters.remove(&row);
        }
        while let Some(row) = imp.list_box_annotations.first_child() {
            imp.list_box_annotations.remove(&row);
        }
    }

    pub fn clear_current_listbox(&self) {
        let list_box = self.get_current_list_box();

        while let Some(row) = list_box.first_child() {
            list_box.remove(&row);
        }
    }
}

impl Default for TocPanel {
    fn default() -> Self {
        Self::new()
    }
}
