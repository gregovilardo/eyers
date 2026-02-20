use crate::modes::WordCursor;
use crate::objects::annotation_object::AnnotationObject;
use crate::services::annotations::Annotation;
use glib::subclass::Signal;
use gtk::CustomSorter;
use gtk::ListView;
use gtk::Stack;
use gtk::glib;
use gtk::glib::property::PropertyGet;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{Box, Button, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow, gio};
use std::cell::{Cell, OnceCell};
use std::sync::OnceLock;

use crate::services::bookmarks::BookmarkEntry;

#[derive(Default, Copy, Clone)]
pub enum TocMode {
    Annotations,
    #[default]
    Chapters,
}

mod imp {

    use crate::modes::WordCursor;

    use super::*;

    #[derive(Default)]
    pub struct TocChapterRow {
        pub page_index: Cell<u16>,
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
        pub title: Label,
        pub subtitle: Label,
        pub page_index: Label,
        pub edit_button: Button,
        pub delete_button: Button,
        pub button_box: Box,
        pub annotation_id: Cell<i64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TocAnnotationRow {
        const NAME: &'static str = "TocAnnotationRow";
        type Type = super::TocAnnotationRow;
        type ParentType = Box;
    }

    impl ObjectImpl for TocAnnotationRow {}
    impl WidgetImpl for TocAnnotationRow {}
    impl BoxImpl for TocAnnotationRow {}

    #[derive(Default)]
    pub struct TocPanel {
        pub title: Label,
        pub mode: Cell<TocMode>,
        pub stack: Stack,
        pub annotations_store: OnceCell<gio::ListStore>,
        pub list_view_annotations: ListView,
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
                    Signal::builder("annotation-edit-requested")
                        .param_types([i64::static_type()])
                        .build(),
                    Signal::builder("annotation-delete-requested")
                        .param_types([i64::static_type()])
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
    pub fn new(page_index: u16, title: &str, depth: usize) -> Self {
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

    pub fn page_index(&self) -> u16 {
        self.imp().page_index.get()
    }

    pub fn depth(&self) -> usize {
        self.imp().depth.get()
    }
}

glib::wrapper! {
    pub struct TocAnnotationRow(ObjectSubclass<imp::TocAnnotationRow>)
        @extends Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Actionable;
}

impl TocAnnotationRow {
    pub fn new() -> Self {
        let row: Self = glib::Object::builder()
            .property("orientation", gtk::Orientation::Horizontal)
            .property("spacing", 4)
            .build();

        row.setup_layout();
        row
    }

    fn setup_layout(&self) {
        let imp = self.imp();
        self.add_css_class("toc-annotation-row");

        self.set_margin_start(12);
        self.set_margin_end(12);
        self.set_margin_top(4);
        self.set_margin_bottom(4);

        let sub_container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .hexpand(true)
            .build();

        imp.title.set_xalign(0.0);
        imp.title.add_css_class("toc-annotation-title");
        sub_container.append(&imp.title);

        imp.subtitle.set_xalign(0.1);
        imp.subtitle.add_css_class("toc-subtitle");
        sub_container.append(&imp.subtitle);

        self.append(&sub_container);

        imp.page_index.set_xalign(0.0);
        imp.page_index.set_hexpand(false);
        imp.page_index.add_css_class("toc-page-index");
        self.append(&imp.page_index);

        // Setup button box
        imp.button_box.set_orientation(gtk::Orientation::Vertical);
        imp.button_box.set_spacing(4);
        imp.button_box.set_hexpand(false);
        imp.button_box.set_valign(gtk::Align::Center);

        // Setup edit button
        imp.edit_button.set_icon_name("document-edit-symbolic");
        imp.edit_button.add_css_class("flat");
        imp.edit_button.set_can_shrink(true);
        imp.button_box.append(&imp.edit_button);

        // Setup delete button
        imp.delete_button.set_icon_name("edit-delete-symbolic");
        imp.delete_button.add_css_class("flat");
        imp.delete_button.set_can_shrink(true);
        imp.button_box.append(&imp.delete_button);

        self.append(&imp.button_box);
    }

    pub fn bind_data(&self, obj: &AnnotationObject) {
        let imp = self.imp();
        let data = obj.annotation();

        imp.title.set_text(&data.selected_text);
        imp.subtitle.set_text(&data.note);
        imp.page_index.set_text(&data.start_page.to_string());
        imp.annotation_id.set(data.id);
    }

    pub fn annotation_id(&self) -> i64 {
        self.imp().annotation_id.get()
    }

    pub fn edit_button(&self) -> &Button {
        &self.imp().edit_button
    }

    pub fn delete_button(&self) -> &Button {
        &self.imp().delete_button
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

        let store = gio::ListStore::new::<AnnotationObject>();
        let _ = self.imp().annotations_store.set(store.clone());
        let sorter = self.create_annotation_sorter();
        let sort_model = gtk::SortListModel::new(Some(store), Some(sorter));
        let selection_model = gtk::SingleSelection::new(Some(sort_model));
        imp.list_view_annotations.set_model(Some(&selection_model));

        let factory = self.create_and_bind_factory();
        self.imp().list_view_annotations.set_factory(Some(&factory));
        self.imp()
            .list_view_annotations
            .set_model(Some(&selection_model));

        let stack = &self.imp().stack;
        stack.add_named(&imp.list_box_chapters, Some("chapters"));
        stack.add_named(&imp.list_view_annotations, Some("annotations"));
        // self.imp().list_view_annotations.set_can_focus(false);

        scrolled_window.set_child(Some(stack));

        self.append(&scrolled_window);
        self.add_css_class("toc-panel");

        let panel_weak = self.downgrade();
        imp.list_box_chapters.connect_row_activated(move |_, row| {
            if let Some(panel) = panel_weak.upgrade() {
                if let Some(entry_row) = row.downcast_ref::<TocChapterRow>() {
                    let null: Option<WordCursor> = None;
                    panel.emit_by_name::<()>(
                        "toc-entry-selected",
                        &[&(entry_row.page_index() as u32), &null],
                    );
                }
            }
        });

        let panel_weak = self.downgrade();
        imp.list_view_annotations
            .connect_activate(move |list_view, position| {
                if let Some(panel) = panel_weak.upgrade() {
                    let model = list_view.model().unwrap();
                    let item = model
                        .item(position)
                        .and_downcast::<AnnotationObject>()
                        .unwrap();
                    println!("{:#?}", item.annotation());
                    println!("{:#?}", item.annotation().get_start_word_cursor());
                    panel.emit_by_name::<()>(
                        "toc-entry-selected",
                        &[
                            &(item.annotation().start_page as u32),
                            &(item.annotation().get_start_word_cursor()),
                        ],
                    );
                }
            });
    }

    fn create_and_bind_factory(&self) -> gtk::SignalListItemFactory {
        let factory = gtk::SignalListItemFactory::new();
        let panel_weak = self.downgrade();

        factory.connect_setup(move |_, list_item| {
            let list_item = list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Debe ser un ListItem");
            let row_widget = TocAnnotationRow::new();

            list_item.set_child(Some(&row_widget));
        });

        factory.connect_bind(move |_, list_item| {
            let list_item = list_item
                .downcast_ref::<gtk::ListItem>()
                .expect("Debe ser un ListItem");

            let data_obj = list_item.item().and_downcast::<AnnotationObject>().unwrap();

            let row_widget = list_item
                .child()
                .and_downcast::<TocAnnotationRow>()
                .unwrap();

            row_widget.bind_data(&data_obj);

            // Connect buttons
            let annotation_id = data_obj.annotation().id;

            // Edit button
            let panel_weak_clone = panel_weak.clone();
            row_widget.edit_button().connect_clicked(move |_| {
                if let Some(panel) = panel_weak_clone.upgrade() {
                    panel.emit_by_name::<()>("annotation-edit-requested", &[&annotation_id]);
                }
            });

            // Delete button
            let panel_weak_clone = panel_weak.clone();
            row_widget.delete_button().connect_clicked(move |_| {
                if let Some(panel) = panel_weak_clone.upgrade() {
                    panel.emit_by_name::<()>("annotation-delete-requested", &[&annotation_id]);
                }
            });
        });
        factory
    }

    fn create_annotation_sorter(&self) -> CustomSorter {
        CustomSorter::new(move |obj1, obj2| {
            let ann1 = obj1
                .downcast_ref::<AnnotationObject>()
                .expect("Objeto 1 no es AnnotationObject")
                .annotation(); // Extrae el struct Annotation

            let ann2 = obj2
                .downcast_ref::<AnnotationObject>()
                .expect("Objeto 2 no es AnnotationObject")
                .annotation(); // Extrae el struct Annotation

            // Usamos el PartialOrd de tu struct Annotation
            if ann1 < ann2 {
                gtk::Ordering::Smaller
            } else if ann1 > ann2 {
                gtk::Ordering::Larger
            } else {
                gtk::Ordering::Equal
            }
        })
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

    pub fn update_list_annotations(&self, new_annotation: Annotation) {
        let store = &self.get_store();

        for i in 0..store.n_items() {
            let item = store.item(i).and_downcast::<AnnotationObject>().unwrap();

            if item.annotation().id == new_annotation.id {
                store.remove(i);
                break;
            }
        }

        store.append(&AnnotationObject::new(new_annotation));
    }

    pub fn remove_listbox_annotation(&self, id: i64) {
        let store = &self.get_store();

        for i in 0..store.n_items() {
            let item = store
                .item(i)
                .and_downcast::<AnnotationObject>()
                .expect("el item debe ser un annotationobject");

            if item.annotation().id == id {
                println!("Nota borrada: {}", id);
                store.remove(i);
                break;
            }
        }
    }

    pub fn get_store(&self) -> &gio::ListStore {
        self.imp()
            .annotations_store
            .get()
            .expect("Store no inicializado")
    }

    pub fn toc_mode(&self) -> TocMode {
        self.imp().mode.get()
    }

    pub fn close_button(&self) -> &Button {
        &self.imp().close_button
    }

    pub fn populate_annotations(&self, entries: &[Annotation]) {
        let store = self
            .imp()
            .annotations_store
            .get()
            .expect("El store no ha sido inicializado");

        store.remove_all();

        if !entries.is_empty() {
            for entry in entries {
                let obj = AnnotationObject::new(entry.clone());
                store.append(&obj);
            }
        }

        // TODO
        // self.actualizar_estado_vacio();
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

    pub fn select_current_chapter(&self, page: u16) {
        let imp = self.imp();
        let children = imp.list_box_chapters.observe_children();

        let mut best_match: Option<glib::Object> = None;
        let mut best_page_index: u16 = 0;

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

    pub fn select_first(&self) {
        let mode = self.toc_mode();
        let imp = self.imp();
        return match mode {
            TocMode::Annotations => {
                assert!(imp.list_view_annotations.is_visible());
                if let Some(selection_model) = imp
                    .list_view_annotations
                    .model()
                    .and_downcast::<gtk::SingleSelection>()
                {
                    selection_model.set_selected(0);
                    imp.list_view_annotations.grab_focus();
                }
            }
            TocMode::Chapters => {
                assert!(imp.list_box_chapters.is_visible());
                if let Some(first_child) = imp.list_box_chapters.first_child() {
                    if let Some(list_row) = first_child.downcast_ref::<ListBoxRow>() {
                        imp.list_box_chapters.select_row(Some(list_row));
                        imp.list_box_chapters.grab_focus();
                    }
                }
            }
        };
    }

    pub fn select_last(&self) {
        let mode = self.toc_mode();
        let imp = self.imp();

        match mode {
            TocMode::Annotations => {
                assert!(imp.list_view_annotations.is_visible());
                if let Some(selection_model) = imp
                    .list_view_annotations
                    .model()
                    .and_downcast::<gtk::SingleSelection>()
                {
                    let n_items = selection_model.model().unwrap().n_items();
                    if n_items > 0 {
                        selection_model.set_selected(n_items - 1);
                        imp.list_view_annotations.scroll_to(
                            n_items - 1,
                            gtk::ListScrollFlags::SELECT | gtk::ListScrollFlags::FOCUS,
                            None,
                        );
                        imp.list_view_annotations.grab_focus();
                    }
                }
            }
            TocMode::Chapters => {
                assert!(imp.list_box_chapters.is_visible());
                if let Some(last_child) = imp.list_box_chapters.last_child() {
                    if let Some(list_row) = last_child.downcast_ref::<ListBoxRow>() {
                        imp.list_box_chapters.select_row(Some(list_row));
                        imp.list_box_chapters.grab_focus();
                    }
                }
            }
        }
    }

    pub fn select_next(&self) -> bool {
        let mode = self.toc_mode();
        let imp = self.imp();
        return match mode {
            TocMode::Annotations => {
                assert!(imp.list_view_annotations.is_visible());
                self.select_next_annotation()
            }
            TocMode::Chapters => {
                assert!(imp.list_box_chapters.is_visible());
                self.select_next_chapter()
            }
        };
    }

    fn select_next_annotation(&self) -> bool {
        let imp = self.imp();
        if let Some(selection_model) = imp
            .list_view_annotations
            .model()
            .and_downcast::<gtk::SingleSelection>()
        {
            let current_pos = selection_model.selected();
            let n_items = selection_model.model().unwrap().n_items();
            if current_pos < n_items - 1 {
                println!("seleccionando posicion {current_pos}+1");
                selection_model.select_item(current_pos + 1, true);
                selection_model.set_selected(current_pos + 1);
                imp.list_view_annotations.scroll_to(
                    current_pos + 1,
                    gtk::ListScrollFlags::SELECT | gtk::ListScrollFlags::FOCUS,
                    None,
                );
                imp.list_view_annotations.grab_focus();
                return true;
            }
        }
        false
    }

    fn select_prev_annotation(&self) -> bool {
        let imp = self.imp();
        if let Some(selection_model) = imp
            .list_view_annotations
            .model()
            .and_downcast::<gtk::SingleSelection>()
        {
            let current_pos = selection_model.selected();

            if current_pos != gtk::INVALID_LIST_POSITION && current_pos > 0 {
                let prev_pos = current_pos - 1;
                selection_model.set_selected(prev_pos);
                selection_model.select_item(prev_pos, true);
                imp.list_view_annotations.scroll_to(
                    prev_pos,
                    gtk::ListScrollFlags::SELECT | gtk::ListScrollFlags::FOCUS,
                    None,
                );
                imp.list_view_annotations.grab_focus();
                return true;
            }
        }
        false
    }

    fn select_next_chapter(&self) -> bool {
        let imp = self.imp();
        if let Some(current) = imp.list_box_chapters.selected_row() {
            if let Some(prev) = current.next_sibling().and_downcast_ref::<gtk::ListBoxRow>() {
                imp.list_box_chapters.select_row(Some(prev));
                prev.grab_focus();
                return true;
            }
        }
        false
    }

    fn select_prev_chapter(&self) -> bool {
        let imp = self.imp();
        if let Some(current) = imp.list_box_chapters.selected_row() {
            if let Some(prev) = current.prev_sibling().and_downcast_ref::<gtk::ListBoxRow>() {
                imp.list_box_chapters.select_row(Some(prev));
                prev.grab_focus();
                return true;
            }
        }
        false
    }

    pub fn select_prev(&self) -> bool {
        let mode = self.toc_mode();
        let imp = self.imp();
        return match mode {
            TocMode::Annotations => {
                assert!(imp.list_view_annotations.is_visible());
                self.select_prev_annotation()
            }
            TocMode::Chapters => {
                assert!(imp.list_box_chapters.is_visible());
                self.select_prev_chapter()
            }
        };
    }

    pub fn navigate_and_close(&self) {
        let mode = self.toc_mode();
        let imp = self.imp();
        match mode {
            TocMode::Chapters => {
                assert!(imp.list_view_annotations.is_visible());

                if let Some(row) = imp.list_box_chapters.selected_row() {
                    if let Some(entry_row) = row.downcast_ref::<TocChapterRow>() {
                        let null: Option<WordCursor> = None;
                        self.emit_by_name::<()>(
                            "toc-entry-selected",
                            &[&(entry_row.page_index() as u32), &null],
                        );
                        self.set_visible(false);
                        return;
                    }
                }
            }
            TocMode::Annotations => {
                assert!(imp.list_box_chapters.is_visible());

                if let Some(selection_model) = imp
                    .list_view_annotations
                    .model()
                    .and_downcast::<gtk::SingleSelection>()
                {
                    let position = selection_model.selected();

                    if position != gtk::INVALID_LIST_POSITION {
                        if let Some(obj) = selection_model
                            .item(position)
                            .and_downcast::<AnnotationObject>()
                        {
                            let ann = obj.annotation();
                            let cursor = Some(ann.get_start_word_cursor());

                            println!("{:#?}", ann);
                            println!("{:#?}", cursor);
                            self.emit_by_name::<()>(
                                "toc-entry-selected",
                                &[&(ann.start_page as u32), &cursor],
                            );
                            self.set_visible(false);
                        }
                    }
                }
            }
        };
    }

    pub fn get_selected_annotation_id(&self) -> Option<i64> {
        let imp = self.imp();

        if !matches!(self.toc_mode(), TocMode::Annotations) {
            return None;
        }

        let selection_model = imp
            .list_view_annotations
            .model()
            .and_downcast::<gtk::SingleSelection>()?;

        let position = selection_model.selected();
        if position == gtk::INVALID_LIST_POSITION {
            return None;
        }

        let obj = selection_model
            .item(position)
            .and_downcast::<AnnotationObject>()?;

        Some(obj.annotation().id)
    }

    pub fn clear(&self) {
        let imp = self.imp();
        while let Some(row) = imp.list_box_chapters.first_child() {
            imp.list_box_chapters.remove(&row);
        }
        self.get_store().remove_all();
    }
}

impl Default for TocPanel {
    fn default() -> Self {
        Self::new()
    }
}
