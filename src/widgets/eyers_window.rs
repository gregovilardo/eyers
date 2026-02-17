use gtk::gio;
use gtk::glib;
use gtk::glib::closure_local;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{ApplicationWindow, Box, Orientation, Paned, PolicyType, ScrolledWindow};
use pdfium_render::prelude::*;
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::Path;

use crate::modes::{
    AppMode, KeyAction, KeyHandler, KeyResult, ScrollDir, WordCursor, handle_normal_mode_key,
    handle_post_global_key, handle_pre_global_key, handle_visual_mode_key,
};
use crate::services::annotations::{self, Annotation};
use crate::services::dictionary::Language;
use crate::services::pdf_text::calculate_picture_offset;
use crate::text_map::{TextMapCache, find_word_on_line_starting_with};
use crate::widgets::{
    AnnotationPanel, EyersHeaderBar, HighlightRect, PdfView, SettingsWindow, StatusBar, TocPanel,
    TranslationPanel,
};

const DEFAULT_VIEWPORT_OFFSET: f64 = 0.2;

mod imp {
    use super::*;

    pub struct EyersWindow {
        pub header_bar: EyersHeaderBar,
        pub pdf_view: PdfView,
        pub toc_panel: TocPanel,
        pub scrolled_window: RefCell<Option<ScrolledWindow>>,
        pub translation_panel: TranslationPanel,
        pub annotation_panel: AnnotationPanel,
        pub pdfium: RefCell<Option<&'static Pdfium>>,
        pub paned: RefCell<Option<Paned>>,
        pub app_mode: RefCell<AppMode>,
        pub text_cache: RefCell<Option<TextMapCache>>,
        /// Toast revealer for copy feedback
        pub toast_revealer: gtk::Revealer,
        /// Toast label for displaying message
        pub toast_label: gtk::Label,
        /// Key handler for managing input state
        pub key_handler: KeyHandler,
        /// Status bar for displaying pending input
        pub status_bar: StatusBar,
        /// Dictionary language setting
        pub dictionary_language: Cell<Language>,
        /// Current PDF file path (for annotations)
        pub current_pdf_path: RefCell<Option<String>>,
        /// Loaded annotations for the current PDF
        pub annotations: RefCell<Vec<Annotation>>,
        /// Pending annotation state: (start, end) cursors being annotated
        pub pending_annotation: RefCell<Option<(WordCursor, WordCursor)>>,
    }

    impl Default for EyersWindow {
        fn default() -> Self {
            let toast_revealer = gtk::Revealer::builder()
                .transition_type(gtk::RevealerTransitionType::SlideDown)
                .transition_duration(150)
                .halign(gtk::Align::Center)
                .valign(gtk::Align::Start)
                .build();

            let toast_label = gtk::Label::new(None);

            Self {
                header_bar: EyersHeaderBar::new(),
                pdf_view: PdfView::new(),
                toc_panel: TocPanel::new(),
                scrolled_window: RefCell::new(None),
                translation_panel: TranslationPanel::new(),
                annotation_panel: AnnotationPanel::new(),
                pdfium: RefCell::new(None),
                paned: RefCell::new(None),
                app_mode: RefCell::new(AppMode::default()),
                text_cache: RefCell::new(None),
                toast_revealer,
                toast_label,
                key_handler: KeyHandler::new(),
                status_bar: StatusBar::new(),
                dictionary_language: Cell::new(Language::default()),
                current_pdf_path: RefCell::new(None),
                annotations: RefCell::new(Vec::new()),
                pending_annotation: RefCell::new(None),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for EyersWindow {
        const NAME: &'static str = "EyersWindow";
        type Type = super::EyersWindow;
        type ParentType = ApplicationWindow;
    }

    impl ObjectImpl for EyersWindow {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_widgets();
        }
    }

    impl WidgetImpl for EyersWindow {}
    impl WindowImpl for EyersWindow {}
    impl ApplicationWindowImpl for EyersWindow {}
}

glib::wrapper! {
    pub struct EyersWindow(ObjectSubclass<imp::EyersWindow>)
        @extends ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl EyersWindow {
    pub fn new(app: &gtk::Application) -> Self {
        let window: Self = glib::Object::builder()
            .property("application", app)
            .property("title", "Eyers")
            .property("default-width", 1000)
            .property("default-height", 700)
            .build();

        window.init_pdfium();
        window
    }

    fn init_pdfium(&self) {
        let bindings = Pdfium::bind_to_library(Path::new("/usr/bin/libpdfium.so"))
            .expect("Failed to bind to PDFium");

        let pdfium: &'static Pdfium =
            std::boxed::Box::leak(std::boxed::Box::new(Pdfium::new(bindings)));

        self.imp().pdfium.replace(Some(pdfium));
        self.imp().pdf_view.set_pdfium(pdfium);
    }

    fn setup_widgets(&self) {
        let imp = self.imp();

        self.set_titlebar(Some(imp.header_bar.widget()));
        self.setup_open_button();
        self.setup_settings_button();

        imp.header_bar
            .bind_property("definitions-enabled", &imp.pdf_view, "definitions-enabled")
            .sync_create()
            .build();

        imp.header_bar
            .bind_property("translate-enabled", &imp.pdf_view, "translate-enabled")
            .sync_create()
            .build();

        let scrolled_window = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Automatic)
            .vscrollbar_policy(PolicyType::Automatic)
            .vexpand(true)
            .hexpand(true)
            .child(&imp.pdf_view)
            .build();

        imp.scrolled_window.replace(Some(scrolled_window.clone()));

        let paned = Paned::builder()
            .orientation(Orientation::Horizontal)
            .build();
        paned.set_wide_handle(true);
        paned.set_start_child(Some(&scrolled_window));
        paned.set_end_child(Some(&imp.toc_panel));
        paned.set_resize_start_child(true);
        paned.set_shrink_start_child(false);
        paned.set_resize_end_child(false);
        paned.set_shrink_end_child(false);
        paned.set_position(800);

        imp.paned.replace(Some(paned.clone()));

        let main_box = Box::builder().orientation(Orientation::Vertical).build();

        main_box.append(&paned);

        imp.translation_panel.set_visible(false);
        main_box.append(&imp.translation_panel);

        imp.annotation_panel.set_visible(false);
        main_box.append(&imp.annotation_panel);

        // Set up toast notification
        self.setup_toast();

        // Create overlay for toast notifications and status bar
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&main_box));
        overlay.add_overlay(&imp.toast_revealer);
        overlay.add_overlay(&imp.status_bar);

        self.set_child(Some(&overlay));

        // Bind key handler status-text to status bar
        self.setup_key_handler_binding();

        self.setup_translation_panel();
        self.setup_annotation_panel();
        self.setup_annotate_button();
        self.setup_toc_panel();
        self.setup_keyboard_controller();
        self.setup_scroll_tracking();
        self.setup_page_indicator_label();
    }

    /// Set up binding between KeyHandler and StatusBar
    fn setup_key_handler_binding(&self) {
        let imp = self.imp();
        let status_bar = imp.status_bar.clone();

        imp.key_handler
            .connect_notify_local(Some("status-text"), move |handler, _| {
                let text = handler.status_text();
                status_bar.set_status_text(&text);
            });
    }

    fn setup_toast(&self) {
        let imp = self.imp();

        // Create toast content box
        let toast_box = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .margin_start(16)
            .margin_end(16)
            .margin_top(12)
            .margin_bottom(12)
            .build();

        toast_box.add_css_class("toast-notification");

        // Add checkmark icon
        let icon = gtk::Image::from_icon_name("object-select-symbolic");
        icon.add_css_class("toast-icon");
        toast_box.append(&icon);

        // Add label
        imp.toast_label.add_css_class("toast-label");
        imp.toast_label
            .set_ellipsize(gtk::pango::EllipsizeMode::End);
        imp.toast_label.set_max_width_chars(50);
        toast_box.append(&imp.toast_label);

        imp.toast_revealer.set_child(Some(&toast_box));
    }

    fn setup_scroll_tracking(&self) {
        let pdf_view = self.imp().pdf_view.clone();
        if let Some(scrolled_window) = self.imp().scrolled_window.borrow().as_ref() {
            let adjustment = scrolled_window.vadjustment();

            adjustment.connect_value_changed(move |_| {
                pdf_view.schedule_page_update();
            });
        }
    }

    fn setup_translation_panel(&self) {
        let imp = self.imp();

        let panel = imp.translation_panel.clone();
        imp.translation_panel
            .close_button()
            .connect_clicked(move |_| {
                panel.set_visible(false);
                panel.clear();
            });

        let panel = imp.translation_panel.clone();
        imp.pdf_view.connect_closure(
            "translate-requested",
            false,
            glib::closure_local!(move |_view: &PdfView, text: &str| {
                panel.set_visible(true);
                panel.translate(text.to_string());
            }),
        );
    }

    fn setup_toc_panel(&self) {
        let imp = self.imp();

        let panel = imp.toc_panel.clone();
        imp.toc_panel.close_button().connect_clicked(move |_| {
            panel.set_visible(false);
        });

        let pdf_view = imp.pdf_view.clone();
        imp.toc_panel.connect_closure(
            "chapter-selected",
            false,
            glib::closure_local!(move |_panel: &TocPanel, page_index: u32| {
                pdf_view.scroll_to_page(page_index);
            }),
        );
    }

    fn setup_keyboard_controller(&self) {
        let controller = gtk::EventControllerKey::new();
        let window_weak = self.downgrade();

        controller.connect_key_pressed(move |_, key, _, modifiers| {
            if let Some(window) = window_weak.upgrade() {
                let imp = window.imp();
                let toc_visible = imp.toc_panel.is_visible();

                // Try pre-global keys first
                match handle_pre_global_key(&imp.key_handler, key, modifiers, toc_visible) {
                    KeyResult::Action(action) => {
                        if window.execute_key_action(action) {
                            return glib::Propagation::Stop;
                        }
                    }
                    KeyResult::StateChanged => {
                        return glib::Propagation::Stop;
                    }
                    KeyResult::Unhandled => {}
                }

                // Handle mode-based key events
                if window.handle_mode_key(key) {
                    return glib::Propagation::Stop;
                }

                // Try post-global keys
                match handle_post_global_key(&imp.key_handler, key) {
                    KeyResult::Action(action) => {
                        if window.execute_key_action(action) {
                            return glib::Propagation::Stop;
                        }
                    }
                    KeyResult::StateChanged => {
                        return glib::Propagation::Stop;
                    }
                    KeyResult::Unhandled => {}
                }
            }
            glib::Propagation::Proceed
        });

        self.add_controller(controller);
    }

    /// Handle key press based on current mode
    fn handle_mode_key(&self, key: gtk::gdk::Key) -> bool {
        let imp = self.imp();

        // Need document to be loaded for other mode operations
        if !imp.pdf_view.has_document() {
            return false;
        }

        let mode = imp.app_mode.borrow().clone();

        let result = match &mode {
            AppMode::Normal => handle_normal_mode_key(&imp.key_handler, key),
            AppMode::Visual { .. } => {
                let doc_borrow = imp.pdf_view.document();
                if let Some(ref doc) = *doc_borrow {
                    let mut cache = imp.text_cache.borrow_mut();
                    if let Some(ref mut cache) = *cache {
                        handle_visual_mode_key(&imp.key_handler, key, &mode, cache, doc)
                    } else {
                        KeyResult::Unhandled
                    }
                } else {
                    KeyResult::Unhandled
                }
            }
        };

        match result {
            KeyResult::Action(action) => self.execute_key_action(action),
            KeyResult::StateChanged => true,
            KeyResult::Unhandled => false,
        }
    }

    /// Execute a key action
    fn execute_key_action(&self, action: KeyAction) -> bool {
        let imp = self.imp();

        match action {
            KeyAction::None => true,

            KeyAction::ToggleTOC => {
                self.toggle_toc_panel();
                true
            }

            KeyAction::SelectChapter => {
                self.toc_panel().navigate_and_close();
                true
            }

            KeyAction::ScrollHalfPage(direction) => {
                self.scroll_half_page(direction);
                true
            }

            KeyAction::ToggleHeaderBar => {
                self.toggle_header_bar();
                true
            }

            KeyAction::ScrollTOC(ScrollDir::Down) => {
                self.toc_panel().select_next();
                true
            }

            KeyAction::ScrollTOC(ScrollDir::Up) => {
                self.toc_panel().select_prev();
                true
            }

            KeyAction::OpenFile => {
                self.show_open_dialog();
                true
            }

            KeyAction::OpenSettings => {
                self.show_settings_window();
                true
            }

            KeyAction::ScrollViewport {
                x_percent,
                y_percent,
            } => {
                self.scroll_by_percent(x_percent, y_percent);
                true
            }

            KeyAction::ScrollToPage { page } => {
                self.scroll_to_page(page);
                true
            }

            KeyAction::ScrollToStart => {
                self.scroll_to_document_start();
                true
            }

            KeyAction::ScrollToEnd => {
                self.scroll_to_document_end();
                true
            }

            KeyAction::EnterVisual => {
                if let Some(cursor) = self.compute_first_visible_word() {
                    println!(
                        "Entering VISUAL mode, cursor at page {} word {}",
                        cursor.page_index, cursor.word_index
                    );
                    let mut mode = imp.app_mode.borrow_mut();
                    *mode = AppMode::enter_visual(cursor);
                    drop(mode);
                    self.update_mode_display();
                    imp.pdf_view.set_cursor(Some(cursor));
                    self.update_highlights();
                    true
                } else {
                    println!("Could not find first visible word");
                    false
                }
            }

            KeyAction::ExitVisual => {
                println!("Exiting VISUAL mode");
                let mut mode = imp.app_mode.borrow_mut();
                *mode = AppMode::exit_to_normal();
                drop(mode);
                self.update_mode_display();
                imp.pdf_view.set_cursor(None);
                imp.pdf_view.clear_selection();
                imp.pdf_view.clear_all_highlights();
                true
            }

            KeyAction::CursorMoved { cursor } => {
                {
                    let mut mode = imp.app_mode.borrow_mut();
                    mode.set_cursor(cursor);
                }
                imp.pdf_view.set_cursor(Some(cursor));
                self.update_selection_display();
                self.ensure_cursor_visible(cursor);
                true
            }

            KeyAction::ToggleSelection => {
                {
                    let mut mode = imp.app_mode.borrow_mut();
                    mode.toggle_selection();
                }
                self.update_selection_display();
                true
            }

            KeyAction::ClearSelection => {
                {
                    let mut mode = imp.app_mode.borrow_mut();
                    mode.clear_selection();
                }
                imp.pdf_view.clear_selection();
                self.update_highlights();
                true
            }

            KeyAction::ShowDefinition { cursor } => {
                if imp.pdf_view.has_popover() {
                    imp.pdf_view.close_current_popover();
                } else {
                    self.show_definition_for_cursor(cursor);
                }
                true
            }

            KeyAction::Translate { start, end } => {
                if imp.translation_panel.is_visible() {
                    imp.translation_panel.set_visible(false);
                } else {
                    self.translate_range(start, end);
                }
                true
            }

            KeyAction::CopyToClipboard { start, end } => {
                self.copy_range_to_clipboard(start, end);
                true
            }

            KeyAction::Annotate { cursor, selection } => {
                self.handle_annotate_action(cursor, selection);
                true
            }

            KeyAction::ExportAnnotations => {
                self.show_export_annotations_dialog();
                true
            }

            KeyAction::FindForward { letter, repeat } => {
                for _ in 0..repeat {
                    //TODO: hate boolean vibecoded crap, please remove for execute_find_forward
                    let result = self.execute_find(letter, true);
                    if !result {
                        break;
                    }
                }
                true
            }

            KeyAction::FindBackward { letter, repeat } => {
                for _ in 0..repeat {
                    let result = self.execute_find(letter, false);
                    if !result {
                        break;
                    }
                }
                true
            }

            KeyAction::ZoomIn => {
                self.zoom_in();
                true
            }

            KeyAction::ZoomOut => {
                self.zoom_out();
                true
            }
        }
    }

    /// Scroll the viewport by a percentage
    fn scroll_by_percent(&self, x_percent: f64, y_percent: f64) {
        if let Some(scrolled) = self.imp().scrolled_window.borrow().as_ref() {
            if y_percent != 0.0 {
                let vadj = scrolled.vadjustment();
                let page_size = vadj.page_size();
                let delta = page_size * (y_percent / 100.0);
                let new_value = (vadj.value() + delta)
                    .max(vadj.lower())
                    .min(vadj.upper() - page_size);
                vadj.set_value(new_value);
            }

            if x_percent != 0.0 {
                let hadj = scrolled.hadjustment();
                let page_size = hadj.page_size();
                let delta = page_size * (x_percent / 100.0);
                let new_value = (hadj.value() + delta)
                    .max(hadj.lower())
                    .min(hadj.upper() - page_size);
                hadj.set_value(new_value);
            }
        }
    }

    /// Scroll half a page and update cursor in Visual mode
    fn scroll_half_page(&self, direction: ScrollDir) {
        let y_percent = match direction {
            ScrollDir::Up => -50.0,
            ScrollDir::Down => 50.0,
        };

        self.scroll_by_percent(0.0, y_percent);

        // In Visual mode, update cursor to word at ~20% from viewport top
        // This feels more natural than the very first word at the top edge
        if let Some(cursor) = self.compute_word_at_viewport_offset(DEFAULT_VIEWPORT_OFFSET) {
            self.move_cursor(cursor);
        }
    }

    fn scroll_to_page(&self, page_number: u32) {
        let pdf_view = &self.imp().pdf_view;
        pdf_view.scroll_to_page(page_number);
        if let Some(cursor) = self.compute_word_at_viewport_offset(DEFAULT_VIEWPORT_OFFSET) {
            self.move_cursor(cursor)
        }
    }

    /// Scroll to the start of the document (gg in vim)
    fn scroll_to_document_start(&self) {
        let imp = self.imp();

        // Scroll to page 0
        imp.pdf_view.scroll_to_page(0);

        // In Visual mode, move cursor to first word of first page
        if let Some(cursor) = self.compute_first_word_of_page(0) {
            self.move_cursor(cursor);
        }
    }

    fn scroll_to_document_end(&self) {
        let imp = self.imp();

        let doc_borrow = imp.pdf_view.document();
        let last_page = match doc_borrow.as_ref() {
            Some(doc) => {
                let page_count = doc.pages().len();
                if page_count > 0 {
                    page_count - 1
                } else {
                    return;
                }
            }
            None => return,
        };
        drop(doc_borrow);

        imp.pdf_view.scroll_to_page(last_page as u32);

        if let Some(cursor) = self.compute_last_word_of_page(last_page as usize) {
            self.move_cursor(cursor);
        }
    }

    fn move_cursor(&self, cursor: WordCursor) {
        let imp = self.imp();
        if imp.app_mode.borrow().is_visual() {
            {
                let mut mode = imp.app_mode.borrow_mut();
                mode.set_cursor(cursor);
            }
            imp.pdf_view.set_cursor(Some(cursor));
            self.update_selection_display();
            self.ensure_cursor_visible(cursor);
            self.print_cursor_word(cursor);
        }
    }

    /// Compute the first word of a specific page
    fn compute_first_word_of_page(&self, page_index: usize) -> Option<WordCursor> {
        let imp = self.imp();

        let doc_borrow = imp.pdf_view.document();
        let doc = doc_borrow.as_ref()?;

        let mut cache = imp.text_cache.borrow_mut();
        println!("first word {:#?}", cache);

        let cache = cache.as_mut()?;

        if let Some(text_map) = cache.get_or_build(page_index, doc) {
            if text_map.word_count() > 0 {
                return Some(WordCursor::new(page_index, 0));
            }
        }

        None
    }

    /// Compute the last word of a specific page
    fn compute_last_word_of_page(&self, page_index: usize) -> Option<WordCursor> {
        let imp = self.imp();

        let doc_borrow = imp.pdf_view.document();
        let doc = doc_borrow.as_ref()?;

        let mut cache = imp.text_cache.borrow_mut();
        let cache = cache.as_mut()?;

        if let Some(text_map) = cache.get_or_build(page_index, doc) {
            let word_count = text_map.word_count();
            if word_count > 0 {
                return Some(WordCursor::new(page_index, word_count - 1));
            }
        }

        None
    }

    /// Zoom in by 10%, max 300%
    fn zoom_in(&self) {
        let imp = self.imp();
        let current_zoom = imp.pdf_view.zoom_level();
        let new_zoom = (current_zoom * 1.1).min(3.0);

        if (new_zoom - current_zoom).abs() > 0.001 {
            self.apply_zoom(new_zoom);
        }
    }

    /// Zoom out by 10%, min 50%
    fn zoom_out(&self) {
        let imp = self.imp();
        let current_zoom = imp.pdf_view.zoom_level();
        let new_zoom = (current_zoom / 1.1).max(0.5);

        if (new_zoom - current_zoom).abs() > 0.001 {
            self.apply_zoom(new_zoom);
        }
    }

    /// Apply a new zoom level, preserving scroll position
    fn apply_zoom(&self, new_zoom: f64) {
        let imp = self.imp();

        // Get current scroll position as a ratio
        let scroll_ratio = if let Some(scrolled) = imp.scrolled_window.borrow().as_ref() {
            let vadj = scrolled.vadjustment();
            let upper = vadj.upper() - vadj.page_size();
            if upper > 0.0 {
                vadj.value() / upper
            } else {
                0.0
            }
        } else {
            0.0
        };

        // Apply the new zoom level (this re-renders all pages)
        imp.pdf_view.set_zoom_level(new_zoom);

        // Restore scroll position after a brief delay to allow layout to update
        let window_weak = self.downgrade();
        glib::idle_add_local_once(move || {
            if let Some(window) = window_weak.upgrade() {
                if let Some(scrolled) = window.imp().scrolled_window.borrow().as_ref() {
                    let vadj = scrolled.vadjustment();
                    let upper = vadj.upper() - vadj.page_size();
                    if upper > 0.0 {
                        vadj.set_value(scroll_ratio * upper);
                    }
                }

                // Update highlights if in visual mode
                if window.imp().app_mode.borrow().is_visual() {
                    window.update_highlights();
                }
            }
        });

        println!("Zoom: {:.0}%", new_zoom * 100.0);
    }

    /// Compute a word at a given offset from the top of the viewport
    /// `offset_percent` is 0.0 for top, 1.0 for bottom (e.g., 0.20 = 20% from top)
    fn compute_word_at_viewport_offset(&self, offset_percent: f64) -> Option<WordCursor> {
        let imp = self.imp();

        let scrolled = imp.scrolled_window.borrow();
        let scrolled = scrolled.as_ref()?;
        let vadj = scrolled.vadjustment();
        let scroll_y = vadj.value();
        let viewport_height = vadj.page_size();

        // Target position in screen coordinates (absolute, not relative to page)
        let target_y = scroll_y + viewport_height * offset_percent;

        let doc_borrow = imp.pdf_view.document();
        let doc = doc_borrow.as_ref()?;

        let mut cache = imp.text_cache.borrow_mut();
        let cache = cache.as_mut()?;

        let page_pictures = imp.pdf_view.page_pictures();
        let spacing = 10.0;

        for (page_index, picture) in page_pictures.iter().enumerate() {
            let nat_size = picture.preferred_size().1;
            let picture_height = nat_size.height() as f64;

            let page_top = page_index as f64 * (picture_height + spacing);
            let page_bottom = page_top + picture_height;

            // Check if the target Y falls within this page
            if target_y >= page_top && target_y < page_bottom {
                if let Some(text_map) = cache.get_or_build(page_index, doc) {
                    if text_map.word_count() > 0 {
                        let page_width_pts = text_map.page_width;
                        let page_height_pts = text_map.page_height;
                        let render_width = crate::services::pdf_text::get_render_width_for_zoom(
                            imp.pdf_view.zoom_level(),
                        );
                        let scale = render_width as f64 / page_width_pts;

                        // Convert target_y to position within page (screen coords relative to page)
                        let target_y_in_page = target_y - page_top;

                        // Convert to PDF coords (y is flipped)
                        let target_pdf_y = page_height_pts - (target_y_in_page / scale);

                        // Find word closest to this y-coordinate
                        // We'll search for a word whose center_y is closest to target_pdf_y
                        let mut best_word_idx: Option<usize> = None;
                        let mut best_distance = f64::MAX;

                        for idx in 0..text_map.word_count() {
                            if let Some(word) = text_map.get_word(idx) {
                                let distance = (word.center_y - target_pdf_y).abs();
                                if distance < best_distance {
                                    best_distance = distance;
                                    best_word_idx = Some(idx);
                                }
                            }
                        }

                        if let Some(word_idx) = best_word_idx {
                            return Some(WordCursor::new(page_index, word_idx));
                        }

                        // Fallback to first word
                        return Some(WordCursor::new(page_index, 0));
                    }
                }
            }
        }

        // If target falls outside all pages (e.g., in spacing), find nearest page
        // and return first visible word
        self.compute_first_visible_word()
    }

    /// Compute the first visible word in the current viewport
    fn compute_first_visible_word(&self) -> Option<WordCursor> {
        let imp = self.imp();

        let scrolled = imp.scrolled_window.borrow();
        let scrolled = scrolled.as_ref()?;
        let vadj = scrolled.vadjustment();
        let scroll_y = vadj.value();
        let viewport_height = vadj.page_size();

        let doc_borrow = imp.pdf_view.document();
        let doc = doc_borrow.as_ref()?;

        let mut cache = imp.text_cache.try_borrow_mut().ok()?;
        let cache = cache.as_mut()?;

        // Find which page is at the top of the viewport
        let page_pictures = imp.pdf_view.page_pictures();
        let spacing = 10.0;

        for (page_index, picture) in page_pictures.iter().enumerate() {
            let nat_size = picture.preferred_size().1;
            let picture_height = nat_size.height() as f64;

            let page_top = page_index as f64 * (picture_height + spacing);
            let page_bottom = page_top + picture_height;

            // Check if this page is visible
            if page_bottom > scroll_y && page_top < scroll_y + viewport_height {
                // Get or build text map for this page
                if let Some(text_map) = cache.get_or_build(page_index, doc) {
                    if text_map.word_count() > 0 {
                        // Calculate viewport rect in PDF coordinates
                        let page_width_pts = text_map.page_width;
                        let page_height_pts = text_map.page_height;
                        let render_width = crate::services::pdf_text::get_render_width_for_zoom(
                            imp.pdf_view.zoom_level(),
                        );
                        let scale = render_width as f64 / page_width_pts;

                        // Visible portion of this page in screen coords
                        let visible_top_screen = (scroll_y - page_top).max(0.0);
                        let visible_bottom_screen =
                            ((scroll_y + viewport_height) - page_top).min(picture_height);

                        // Convert to PDF coords (y is flipped)
                        let visible_top_pdf = page_height_pts - (visible_top_screen / scale);
                        let visible_bottom_pdf = page_height_pts - (visible_bottom_screen / scale);

                        // Find first word in this rect
                        if let Some(word_index) =
                            text_map.first_word_in_rect(visible_top_pdf, visible_bottom_pdf)
                        {
                            return Some(WordCursor::new(page_index, word_index));
                        }

                        // If no word found in viewport, just use first word
                        return Some(WordCursor::new(page_index, 0));
                    }
                }
            }
        }

        None
    }

    /// Update the mode label in the header bar
    fn update_mode_display(&self) {
        let imp = self.imp();
        let mode = imp.app_mode.borrow();
        imp.header_bar.set_mode_text(mode.display_name());

        // Enable/disable annotate button based on mode
        let is_visual = mode.is_visual();
        imp.header_bar.annotate_button().set_sensitive(is_visual);
    }

    /// Debug helper: print the word at cursor position
    fn print_cursor_word(&self, cursor: WordCursor) {
        let imp = self.imp();
        let cache = imp.text_cache.borrow();
        if let Some(ref cache) = *cache {
            if let Some(text_map) = cache.get(cursor.page_index) {
                if let Some(word) = text_map.get_word(cursor.word_index) {
                    println!("  -> Word: '{}' (line {})", word.text, word.line_index);
                }
            }
        }
    }

    /// Update selection display based on current mode
    fn update_selection_display(&self) {
        let mode = self.imp().app_mode.borrow();
        if let Some((start, end)) = mode.selection_range() {
            self.imp().pdf_view.set_selection(Some((start, end)));
        } else {
            self.imp().pdf_view.clear_selection();
        }
        drop(mode);
        self.update_highlights();
    }

    /// Update all highlight overlays based on current cursor and selection
    fn update_highlights(&self) {
        let imp = self.imp();

        // Clear all existing highlights first
        imp.pdf_view.clear_all_highlights();

        let cursor = imp.pdf_view.cursor();
        let selection = imp.pdf_view.selection();

        let cache = imp.text_cache.borrow();
        let cache = match cache.as_ref() {
            Some(c) => c,
            None => return,
        };

        // Get page pictures for calculating offsets
        let page_pictures = imp.pdf_view.page_pictures();

        // Get effective render width based on zoom level
        let render_width =
            crate::services::pdf_text::get_render_width_for_zoom(imp.pdf_view.zoom_level());

        // Helper closure to get x_offset for a page
        let get_x_offset = |page_index: usize| -> f64 {
            page_pictures
                .get(page_index)
                .map(|pic| calculate_picture_offset(pic))
                .unwrap_or(0.0)
        };

        // Build a map of page_index -> (cursor_rect, selection_rects)
        let mut page_highlights: std::collections::HashMap<
            usize,
            (Option<HighlightRect>, Vec<HighlightRect>),
        > = std::collections::HashMap::new();

        // Add cursor highlight
        if let Some(cursor) = cursor {
            if let Some(text_map) = cache.get(cursor.page_index) {
                if let Some(word) = text_map.get_word(cursor.word_index) {
                    let x_offset = get_x_offset(cursor.page_index);
                    let rect = HighlightRect::from_pdf_bounds(
                        &word.bounds,
                        text_map.page_width,
                        text_map.page_height,
                        x_offset,
                        render_width,
                    );
                    page_highlights
                        .entry(cursor.page_index)
                        .or_insert((None, Vec::new()))
                        .0 = Some(rect);
                }
            }
        }

        // Add selection highlights
        if let Some((start, end)) = selection {
            let (first, last) =
                if (start.page_index, start.word_index) <= (end.page_index, end.word_index) {
                    (start, end)
                } else {
                    (end, start)
                };

            if first.page_index == last.page_index {
                // Same page selection
                if let Some(text_map) = cache.get(first.page_index) {
                    let x_offset = get_x_offset(first.page_index);
                    for idx in first.word_index..=last.word_index {
                        if let Some(word) = text_map.get_word(idx) {
                            let rect = HighlightRect::from_pdf_bounds(
                                &word.bounds,
                                text_map.page_width,
                                text_map.page_height,
                                x_offset,
                                render_width,
                            );
                            page_highlights
                                .entry(first.page_index)
                                .or_insert((None, Vec::new()))
                                .1
                                .push(rect);
                        }
                    }
                }
            } else {
                // Cross-page selection
                // First page: from first.word_index to end
                if let Some(text_map) = cache.get(first.page_index) {
                    let x_offset = get_x_offset(first.page_index);
                    for idx in first.word_index..text_map.word_count() {
                        if let Some(word) = text_map.get_word(idx) {
                            let rect = HighlightRect::from_pdf_bounds(
                                &word.bounds,
                                text_map.page_width,
                                text_map.page_height,
                                x_offset,
                                render_width,
                            );
                            page_highlights
                                .entry(first.page_index)
                                .or_insert((None, Vec::new()))
                                .1
                                .push(rect);
                        }
                    }
                }

                // Middle pages
                for page_idx in (first.page_index + 1)..last.page_index {
                    if let Some(text_map) = cache.get(page_idx) {
                        let x_offset = get_x_offset(page_idx);
                        for idx in 0..text_map.word_count() {
                            if let Some(word) = text_map.get_word(idx) {
                                let rect = HighlightRect::from_pdf_bounds(
                                    &word.bounds,
                                    text_map.page_width,
                                    text_map.page_height,
                                    x_offset,
                                    render_width,
                                );
                                page_highlights
                                    .entry(page_idx)
                                    .or_insert((None, Vec::new()))
                                    .1
                                    .push(rect);
                            }
                        }
                    }
                }

                // Last page: from 0 to last.word_index
                if let Some(text_map) = cache.get(last.page_index) {
                    let x_offset = get_x_offset(last.page_index);
                    for idx in 0..=last.word_index {
                        if let Some(word) = text_map.get_word(idx) {
                            let rect = HighlightRect::from_pdf_bounds(
                                &word.bounds,
                                text_map.page_width,
                                text_map.page_height,
                                x_offset,
                                render_width,
                            );
                            page_highlights
                                .entry(last.page_index)
                                .or_insert((None, Vec::new()))
                                .1
                                .push(rect);
                        }
                    }
                }
            }
        }

        // Apply highlights to overlays
        for (page_index, (cursor_rect, selection_rects)) in page_highlights {
            if let Some(overlay) = imp.pdf_view.highlight_overlay(page_index) {
                overlay.set_highlights(cursor_rect, selection_rects);
            }
        }
    }

    /// Ensure the cursor is visible, auto-scrolling if needed
    fn ensure_cursor_visible(&self, cursor: WordCursor) {
        let imp = self.imp();

        let scrolled = imp.scrolled_window.borrow();
        let scrolled = match scrolled.as_ref() {
            Some(s) => s,
            None => return,
        };

        let doc_borrow = imp.pdf_view.document();
        if doc_borrow.is_none() {
            return;
        }

        let cache = imp.text_cache.borrow();
        let cache = match cache.as_ref() {
            Some(c) => c,
            None => return,
        };

        let text_map = match cache.get(cursor.page_index) {
            Some(tm) => tm,
            None => return,
        };

        let word = match text_map.get_word(cursor.word_index) {
            Some(w) => w,
            None => return,
        };

        // Calculate word position in screen coordinates
        let page_pictures = imp.pdf_view.page_pictures();
        let picture = match page_pictures.get(cursor.page_index) {
            Some(p) => p,
            None => return,
        };

        let nat_size = picture.preferred_size().1;
        let picture_height = nat_size.height() as f64;
        let spacing = 10.0;

        let page_top = cursor.page_index as f64 * (picture_height + spacing);

        // Convert word center to screen coords
        let render_width =
            crate::services::pdf_text::get_render_width_for_zoom(imp.pdf_view.zoom_level());
        let scale = render_width as f64 / text_map.page_width;
        let word_y_screen = page_top + (text_map.page_height - word.center_y) * scale;

        // Get viewport info
        let vadj = scrolled.vadjustment();
        let scroll_y = vadj.value();
        let viewport_height = vadj.page_size();

        // 20% margin
        let margin = viewport_height * 0.2;
        let visible_top = scroll_y + margin;
        let visible_bottom = scroll_y + viewport_height - margin;

        // Auto-scroll if cursor is outside the comfortable zone
        if word_y_screen < visible_top {
            // Scroll up
            let new_scroll = word_y_screen - margin;
            vadj.set_value(new_scroll.max(vadj.lower()));
        } else if word_y_screen > visible_bottom {
            // Scroll down
            let new_scroll = word_y_screen - viewport_height + margin;
            vadj.set_value(new_scroll.min(vadj.upper() - viewport_height));
        }
    }

    /// Show definition for the word at cursor position
    fn show_definition_for_cursor(&self, cursor: WordCursor) {
        let imp = self.imp();

        let cache = imp.text_cache.borrow();
        let cache = match cache.as_ref() {
            Some(c) => c,
            None => return,
        };

        let text_map = match cache.get(cursor.page_index) {
            Some(tm) => tm,
            None => return,
        };

        let word = match text_map.get_word(cursor.word_index) {
            Some(w) => w,
            None => return,
        };

        // Show definition using existing mechanism
        let word_text = word.text.clone();
        println!("Definition for: {}", word_text);

        // Use the definition popover
        let page_pictures = imp.pdf_view.page_pictures();
        if let Some(pic) = page_pictures.get(cursor.page_index) {
            // Calculate screen position for popover (including x_offset for centering)
            let render_width =
                crate::services::pdf_text::get_render_width_for_zoom(imp.pdf_view.zoom_level());
            let scale = render_width as f64 / text_map.page_width;
            let x_offset = calculate_picture_offset(pic);
            let screen_x = word.center_x * scale + x_offset;
            let screen_y = (text_map.page_height - word.center_y) * scale;

            let popover = crate::widgets::DefinitionPopover::new();
            popover.show_at(pic, screen_x, screen_y);
            popover.fetch_and_display(
                word_text.clone(),
                word_text.to_lowercase(),
                imp.dictionary_language.get(),
            );

            imp.pdf_view.set_current_popover(Some(popover));
        }
    }

    /// Translate the text between start and end cursors
    fn translate_range(&self, start: WordCursor, end: WordCursor) {
        let imp = self.imp();

        let cache = imp.text_cache.borrow();
        let cache = match cache.as_ref() {
            Some(c) => c,
            None => return,
        };

        let mut text_parts: Vec<String> = Vec::new();

        // Collect text from start to end (possibly across pages)
        if start.page_index == end.page_index {
            // Same page
            if let Some(text_map) = cache.get(start.page_index) {
                let word_start = start.word_index.min(end.word_index);
                let word_end = start.word_index.max(end.word_index);

                for idx in word_start..=word_end {
                    if let Some(word) = text_map.get_word(idx) {
                        text_parts.push(word.text.clone());
                    }
                }
            }
        } else {
            // Cross-page selection
            let (first, last) = if start.page_index < end.page_index {
                (start, end)
            } else {
                (end, start)
            };

            // First page: from start.word_index to end of page
            if let Some(text_map) = cache.get(first.page_index) {
                for idx in first.word_index..text_map.word_count() {
                    if let Some(word) = text_map.get_word(idx) {
                        text_parts.push(word.text.clone());
                    }
                }
            }

            // Middle pages: all words
            for page_idx in (first.page_index + 1)..last.page_index {
                if let Some(text_map) = cache.get(page_idx) {
                    for idx in 0..text_map.word_count() {
                        if let Some(word) = text_map.get_word(idx) {
                            text_parts.push(word.text.clone());
                        }
                    }
                }
            }

            // Last page: from start to end.word_index
            if let Some(text_map) = cache.get(last.page_index) {
                for idx in 0..=last.word_index {
                    if let Some(word) = text_map.get_word(idx) {
                        text_parts.push(word.text.clone());
                    }
                }
            }
        }

        let text = text_parts.join(" ");
        if !text.is_empty() {
            imp.translation_panel.set_visible(true);
            imp.translation_panel.translate(text);
        }
    }

    /// Execute a find operation (f/F + char)
    fn execute_find(&self, target_char: char, forward: bool) -> bool {
        let imp = self.imp();

        // Only works in Visual mode
        let cursor = match imp.app_mode.borrow().cursor() {
            Some(c) => c,
            None => return false,
        };

        // Find the target word - scope the borrows
        let new_cursor = {
            let doc_borrow = imp.pdf_view.document();
            let doc = match doc_borrow.as_ref() {
                Some(d) => d,
                None => return false,
            };

            let mut cache = imp.text_cache.borrow_mut();
            let cache = match cache.as_mut() {
                Some(c) => c,
                None => return false,
            };

            // Find word on same line starting with target_char
            find_word_on_line_starting_with(
                cache,
                doc,
                cursor.page_index,
                cursor.word_index,
                target_char,
                forward,
            )
            .map(|result| WordCursor::new(result.page_index, result.word_index))
        };

        // Update cursor if found
        if let Some(new_cursor) = new_cursor {
            {
                let mut mode = imp.app_mode.borrow_mut();
                mode.set_cursor(new_cursor);
            }
            imp.pdf_view.set_cursor(Some(new_cursor));
            self.update_selection_display();
            self.ensure_cursor_visible(new_cursor);
            self.print_cursor_word(new_cursor);
            true
        } else {
            // No match found, do nothing
            false
        }
    }

    /// Copy text range to clipboard and show feedback popup
    fn copy_range_to_clipboard(&self, start: WordCursor, end: WordCursor) {
        let imp = self.imp();

        // Extract text with scoped borrow
        let text = {
            let cache = imp.text_cache.borrow();
            match cache.as_ref() {
                Some(c) => self.extract_text_range(c, start, end),
                None => return,
            }
        };

        if !text.is_empty() {
            let clipboard = self.clipboard();
            clipboard.set_text(&text);
            self.show_copy_feedback(&text);
        }
    }

    /// Extract text from a cursor range (reusable helper)
    fn extract_text_range(
        &self,
        cache: &TextMapCache,
        start: WordCursor,
        end: WordCursor,
    ) -> String {
        let mut text_parts: Vec<String> = Vec::new();

        if start.page_index == end.page_index {
            // Same page
            if let Some(text_map) = cache.get(start.page_index) {
                let word_start = start.word_index.min(end.word_index);
                let word_end = start.word_index.max(end.word_index);

                for idx in word_start..=word_end {
                    if let Some(word) = text_map.get_word(idx) {
                        if let Some(surr_left) = &word.surround_left {
                            if idx != word_start {
                                text_parts.push(surr_left.clone());
                            }
                        }
                        text_parts.push(word.text.clone());
                    }
                }
            }
        } else {
            // Cross-page selection
            let (first, last) = if start.page_index < end.page_index {
                (start, end)
            } else {
                (end, start)
            };

            // First page
            if let Some(text_map) = cache.get(first.page_index) {
                for idx in first.word_index..text_map.word_count() {
                    if let Some(word) = text_map.get_word(idx) {
                        text_parts.push(word.text.clone());
                    }
                }
            }

            // Middle pages
            for page_idx in (first.page_index + 1)..last.page_index {
                if let Some(text_map) = cache.get(page_idx) {
                    for idx in 0..text_map.word_count() {
                        if let Some(word) = text_map.get_word(idx) {
                            text_parts.push(word.text.clone());
                        }
                    }
                }
            }

            // Last page
            if let Some(text_map) = cache.get(last.page_index) {
                for idx in 0..=last.word_index {
                    if let Some(word) = text_map.get_word(idx) {
                        text_parts.push(word.text.clone());
                    }
                }
            }
        }

        text_parts.join("")
    }

    /// Show a brief toast notification when text is copied
    fn show_copy_feedback(&self, text: &str) {
        let imp = self.imp();

        // Format the message with a preview of copied text
        let preview = if text.len() > 40 {
            format!("Copied: \"{}...\"", &text[..37])
        } else {
            format!("Copied: \"{}\"", text)
        };

        imp.toast_label.set_text(&preview);

        // Show the toast
        imp.toast_revealer.set_reveal_child(true);

        // Auto-hide after 1.5 seconds
        let revealer = imp.toast_revealer.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(1500), move || {
            revealer.set_reveal_child(false);
        });
    }

    fn toggle_toc_panel(&self) {
        let imp = self.imp();
        let is_visible = imp.toc_panel.is_visible();
        imp.toc_panel.set_visible(!is_visible);

        if !is_visible {
            imp.toc_panel.grab_focus();
            let current_page = imp.pdf_view.current_page();
            imp.toc_panel.select_current_chapter(current_page);
        }
    }

    fn toggle_header_bar(&self) {
        let imp = self.imp();
        let header = imp.header_bar.widget();
        let is_visible = header.is_visible();
        header.set_visible(!is_visible);
    }

    fn setup_open_button(&self) {
        let window_weak = self.downgrade();

        self.imp()
            .header_bar
            .open_button()
            .connect_clicked(move |_| {
                if let Some(window) = window_weak.upgrade() {
                    window.show_open_dialog();
                }
            });
    }

    fn setup_settings_button(&self) {
        let window_weak = self.downgrade();

        self.imp()
            .header_bar
            .settings_button()
            .connect_clicked(move |_| {
                if let Some(window) = window_weak.upgrade() {
                    window.show_settings_window();
                }
            });
    }

    fn show_settings_window(&self) {
        let settings = SettingsWindow::new(self);
        settings.set_language(self.imp().dictionary_language.get());

        let window_weak = self.downgrade();
        settings
            .language_dropdown()
            .connect_selected_notify(move |dropdown| {
                if let Some(window) = window_weak.upgrade() {
                    let lang = match dropdown.selected() {
                        1 => Language::Spanish,
                        _ => Language::English,
                    };
                    window.imp().dictionary_language.set(lang);
                    window.imp().pdf_view.set_dictionary_language(lang);
                }
            });

        settings.present();
    }

    fn show_open_dialog(&self) {
        let dialog = gtk::FileDialog::builder().title("Select a PDF").build();
        let window_weak = self.downgrade();

        dialog.open(Some(self), None::<&gio::Cancellable>, move |result| {
            if let Some(window) = window_weak.upgrade() {
                window.handle_file_dialog_result(result);
            }
        });
    }

    fn handle_file_dialog_result(&self, result: Result<gio::File, glib::Error>) {
        let file = match result {
            Ok(f) => f,
            Err(_) => return,
        };

        let path = match file.path() {
            Some(p) => p,
            None => return,
        };

        self.open_file(&path);
    }

    /// Show export annotations confirmation dialog
    fn show_export_annotations_dialog(&self) {
        let pdf_path = match self.imp().current_pdf_path.borrow().as_ref() {
            Some(p) => p.clone(),
            None => {
                eprintln!("No PDF loaded, cannot export annotations");
                return;
            }
        };

        // Check if there are any annotations to export
        let annotations = match annotations::load_annotations_for_pdf(&pdf_path) {
            Ok(anns) => anns,
            Err(e) => {
                eprintln!("Failed to load annotations: {}", e);
                return;
            }
        };

        if annotations.is_empty() {
            // Show a dialog saying there are no annotations
            let dialog = gtk::AlertDialog::builder()
                .message("No Annotations")
                .detail("There are no annotations to export for this PDF.")
                .buttons(["OK"])
                .build();
            dialog.show(Some(self));
            return;
        }

        // Show confirmation dialog
        let dialog = gtk::AlertDialog::builder()
            .message("Export Annotations")
            .detail(&format!(
                "Export {} annotation(s) to a Markdown file?",
                annotations.len()
            ))
            .buttons(["Cancel", "Export"])
            .default_button(1)
            .cancel_button(0)
            .build();

        let window_weak = self.downgrade();
        dialog.choose(Some(self), None::<&gio::Cancellable>, move |result| {
            if let Some(window) = window_weak.upgrade() {
                if let Ok(choice) = result {
                    if choice == 1 {
                        // User chose "Export"
                        window.show_export_file_chooser();
                    }
                }
            }
        });
    }

    /// Show file chooser for saving exported annotations
    fn show_export_file_chooser(&self) {
        let pdf_path = match self.imp().current_pdf_path.borrow().as_ref() {
            Some(p) => p.clone(),
            None => return,
        };

        // Generate default filename from PDF name
        let pdf_name = Path::new(&pdf_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("annotations");
        let default_filename = format!("{}_annotations.md", pdf_name);

        let dialog = gtk::FileDialog::builder()
            .title("Save Annotations")
            .initial_name(&default_filename)
            .build();

        let window_weak = self.downgrade();
        dialog.save(Some(self), None::<&gio::Cancellable>, move |result| {
            if let Some(window) = window_weak.upgrade() {
                window.handle_export_save_result(result);
            }
        });
    }

    /// Handle the result of the export file save dialog
    fn handle_export_save_result(&self, result: Result<gio::File, glib::Error>) {
        let file = match result {
            Ok(f) => f,
            Err(_) => return, // User cancelled
        };

        let save_path = match file.path() {
            Some(p) => p,
            None => return,
        };

        let pdf_path = match self.imp().current_pdf_path.borrow().as_ref() {
            Some(p) => p.clone(),
            None => return,
        };

        // Get PDF name for the markdown header
        let pdf_name = Path::new(&pdf_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown PDF");

        // Generate markdown content
        let markdown = match annotations::export_to_markdown(&pdf_path, pdf_name) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Failed to generate markdown: {}", e);
                self.show_export_error(&format!("Failed to generate markdown: {}", e));
                return;
            }
        };

        // Write to file
        if let Err(e) = fs::write(&save_path, &markdown) {
            eprintln!("Failed to write file: {}", e);
            self.show_export_error(&format!("Failed to write file: {}", e));
            return;
        }

        // Show success message
        let dialog = gtk::AlertDialog::builder()
            .message("Export Successful")
            .detail(&format!("Annotations saved to:\n{}", save_path.display()))
            .buttons(["OK"])
            .build();
        dialog.show(Some(self));
    }

    /// Show an error dialog for export failures
    fn show_export_error(&self, message: &str) {
        let dialog = gtk::AlertDialog::builder()
            .message("Export Failed")
            .detail(message)
            .buttons(["OK"])
            .build();
        dialog.show(Some(self));
    }

    /// Open a PDF file from a path (public API for CLI usage)
    pub fn open_file(&self, path: &Path) {
        if let Err(e) = self.imp().pdf_view.load_pdf(path.to_path_buf()) {
            eprintln!("{}", e);
            return;
        }

        // Store the PDF path for annotations
        self.imp()
            .current_pdf_path
            .replace(Some(path.to_string_lossy().to_string()));

        self.init_text_cache();
        self.extract_and_populate_bookmarks();

        // Load annotations for this PDF
        self.reload_annotations();

        // Reset to Normal mode when loading new PDF
        {
            let mut mode = self.imp().app_mode.borrow_mut();
            *mode = AppMode::exit_to_normal();
        }
        self.update_mode_display();
        self.pdf_view().set_cursor(None);
        self.pdf_view().clear_selection();
        self.pdf_view().clear_all_highlights();

        // Update annotation highlights after a brief delay to ensure pages are rendered
        let window_weak = self.downgrade();
        glib::idle_add_local_once(move || {
            if let Some(window) = window_weak.upgrade() {
                window.update_annotation_highlights();
            }
        });
    }

    fn setup_page_indicator_label(&self) {
        let header_bar = self.header_bar().clone();
        self.pdf_view().connect_closure(
            "current-page-updated",
            false,
            closure_local!(|_pdf_view: &PdfView, current_page: u32, total_pages: u32| {
                let page_indicator_text = format!("[{current_page}/{total_pages}]");
                header_bar.set_pages_indicator_text(&page_indicator_text);
            }),
        );
    }

    /// Initialize the text cache for the loaded document
    fn init_text_cache(&self) {
        let imp = self.imp();

        if let Some(ref doc) = *imp.pdf_view.document() {
            let page_count = doc.pages().len() as usize;
            let cache = TextMapCache::new(page_count);
            imp.text_cache.replace(Some(cache));
        }
    }

    fn extract_and_populate_bookmarks(&self) {
        let bookmarks = self.imp().pdf_view.bookmarks();
        self.imp().toc_panel.populate(&bookmarks);
    }

    pub fn header_bar(&self) -> &EyersHeaderBar {
        &self.imp().header_bar
    }

    pub fn pdf_view(&self) -> &PdfView {
        &self.imp().pdf_view
    }

    pub fn toc_panel(&self) -> &TocPanel {
        &self.imp().toc_panel
    }

    pub fn translation_panel(&self) -> &TranslationPanel {
        &self.imp().translation_panel
    }

    pub fn key_handler(&self) -> &KeyHandler {
        &self.imp().key_handler
    }

    // ============ Annotation Methods ============

    fn setup_annotation_panel(&self) {
        let imp = self.imp();

        // Handle save
        let window_weak = self.downgrade();
        imp.annotation_panel.connect_closure(
            "save-requested",
            false,
            glib::closure_local!(move |_panel: &AnnotationPanel, note: &str| {
                if let Some(window) = window_weak.upgrade() {
                    window.save_current_annotation(note);
                }
            }),
        );

        // Handle cancel
        let window_weak = self.downgrade();
        imp.annotation_panel.connect_closure(
            "cancel-requested",
            false,
            glib::closure_local!(move |_panel: &AnnotationPanel| {
                if let Some(window) = window_weak.upgrade() {
                    window.close_annotation_panel();
                }
            }),
        );

        // Handle delete
        let window_weak = self.downgrade();
        imp.annotation_panel.connect_closure(
            "delete-requested",
            false,
            glib::closure_local!(move |_panel: &AnnotationPanel, id: i64| {
                if let Some(window) = window_weak.upgrade() {
                    window.delete_annotation(id);
                }
            }),
        );
    }

    fn setup_annotate_button(&self) {
        let window_weak = self.downgrade();
        self.imp()
            .header_bar
            .annotate_button()
            .connect_clicked(move |_| {
                if let Some(window) = window_weak.upgrade() {
                    // Trigger annotation from button click
                    let imp = window.imp();
                    let mode = imp.app_mode.borrow();
                    if let Some(cursor) = mode.cursor() {
                        let selection = mode.selection_range();
                        drop(mode);
                        window.handle_annotate_action(cursor, selection);
                    }
                }
            });
    }

    /// Handle the annotate action (from 'a' key or button)
    fn handle_annotate_action(
        &self,
        cursor: WordCursor,
        selection: Option<(WordCursor, WordCursor)>,
    ) {
        let imp = self.imp();

        let pdf_path = match imp.current_pdf_path.borrow().as_ref() {
            Some(p) => p.clone(),
            None => return,
        };

        // Determine the range to annotate
        let (start, end) = selection.unwrap_or((cursor, cursor));

        // Check if there's an existing annotation at cursor position (for editing)
        // Also check for overlapping annotations with the selection
        let existing_annotation = if selection.is_some() {
            // Selection mode: check for overlaps
            annotations::find_overlapping_annotations(
                &pdf_path,
                start.page_index,
                start.word_index,
                end.page_index,
                end.word_index,
            )
            .ok()
            .and_then(|v| v.into_iter().next())
        } else {
            // No selection: check if cursor is on an existing annotation
            annotations::find_annotation_at_position(
                &pdf_path,
                cursor.page_index,
                cursor.word_index,
            )
            .ok()
            .flatten()
        };

        // Get the selected text
        let selected_text = {
            let cache = imp.text_cache.borrow();
            match cache.as_ref() {
                Some(c) => self.extract_text_range(c, start, end),
                None => return,
            }
        };

        // Store the pending annotation range
        imp.pending_annotation.replace(Some((start, end)));

        // Setup the panel
        imp.annotation_panel.set_selected_text(&selected_text);

        if let Some(ann) = existing_annotation {
            // Editing existing annotation
            imp.annotation_panel.set_annotation_id(Some(ann.id));
            imp.annotation_panel.set_note(&ann.note);
        } else {
            // New annotation
            imp.annotation_panel.set_annotation_id(None);
            imp.annotation_panel.set_note("");
        }

        // Show panel and focus input
        imp.annotation_panel.set_visible(true);
        imp.annotation_panel.focus_input();
    }

    fn save_current_annotation(&self, note: &str) {
        let imp = self.imp();

        let pdf_path = match imp.current_pdf_path.borrow().as_ref() {
            Some(p) => p.clone(),
            None => return,
        };

        let (start, end) = match imp.pending_annotation.borrow().as_ref() {
            Some((s, e)) => (*s, *e),
            None => return,
        };

        // Get the selected text
        let selected_text = {
            let cache = imp.text_cache.borrow();
            match cache.as_ref() {
                Some(c) => self.extract_text_range(c, start, end),
                None => return,
            }
        };

        let annotation_id = imp.annotation_panel.annotation_id();

        // Save or update
        let result = if let Some(id) = annotation_id {
            // Update existing
            annotations::update_annotation(
                id,
                start.page_index,
                start.word_index,
                end.page_index,
                end.word_index,
                &selected_text,
                note,
            )
            .map(|_| id)
        } else {
            // Create new
            annotations::save_annotation(
                &pdf_path,
                start.page_index,
                start.word_index,
                end.page_index,
                end.word_index,
                &selected_text,
                note,
            )
        };

        match result {
            Ok(_) => {
                println!("Annotation saved successfully");
                self.close_annotation_panel();
                self.reload_annotations();
                self.update_annotation_highlights();
            }
            Err(e) => {
                eprintln!("Failed to save annotation: {}", e);
            }
        }
    }

    fn delete_annotation(&self, id: i64) {
        match annotations::delete_annotation(id) {
            Ok(_) => {
                println!("Annotation deleted successfully");
                self.close_annotation_panel();
                self.reload_annotations();
                self.update_annotation_highlights();
            }
            Err(e) => {
                eprintln!("Failed to delete annotation: {}", e);
            }
        }
    }

    fn close_annotation_panel(&self) {
        let imp = self.imp();
        imp.annotation_panel.set_visible(false);
        imp.annotation_panel.clear();
        imp.pending_annotation.replace(None);
    }

    /// Reload annotations from the database for the current PDF
    fn reload_annotations(&self) {
        let imp = self.imp();

        let pdf_path = match imp.current_pdf_path.borrow().as_ref() {
            Some(p) => p.clone(),
            None => {
                imp.annotations.replace(Vec::new());
                return;
            }
        };

        match annotations::load_annotations_for_pdf(&pdf_path) {
            Ok(anns) => {
                println!("Loaded {} annotations", anns.len());
                imp.annotations.replace(anns);
            }
            Err(e) => {
                eprintln!("Failed to load annotations: {}", e);
                imp.annotations.replace(Vec::new());
            }
        }
    }

    /// Update annotation highlights on all pages
    fn update_annotation_highlights(&self) {
        let imp = self.imp();

        let annotations = imp.annotations.borrow();
        if annotations.is_empty() {
            // Clear all annotation highlights
            for overlay in imp.pdf_view.highlight_overlays().iter() {
                overlay.set_annotations(Vec::new());
            }
            return;
        }

        // We need mutable access to cache and document access
        let doc_borrow = imp.pdf_view.document();
        let doc = match doc_borrow.as_ref() {
            Some(d) => d,
            None => return,
        };

        let mut cache = imp.text_cache.borrow_mut();
        let cache = match cache.as_mut() {
            Some(c) => c,
            None => return,
        };

        let page_pictures = imp.pdf_view.page_pictures();
        let render_width =
            crate::services::pdf_text::get_render_width_for_zoom(imp.pdf_view.zoom_level());

        // Build annotation highlights per page
        let mut page_ann_rects: std::collections::HashMap<usize, Vec<HighlightRect>> =
            std::collections::HashMap::new();

        for ann in annotations.iter() {
            // Handle same-page and cross-page annotations
            if ann.start_page == ann.end_page {
                // Same page - use get_or_build to ensure the text map exists
                if let Some(text_map) = cache.get_or_build(ann.start_page, doc) {
                    let x_offset = page_pictures
                        .get(ann.start_page)
                        .map(|pic| calculate_picture_offset(pic))
                        .unwrap_or(0.0);

                    for idx in ann.start_word..=ann.end_word {
                        if let Some(word) = text_map.get_word(idx) {
                            let rect = HighlightRect::from_pdf_bounds(
                                &word.bounds,
                                text_map.page_width,
                                text_map.page_height,
                                x_offset,
                                render_width,
                            );
                            page_ann_rects
                                .entry(ann.start_page)
                                .or_insert_with(Vec::new)
                                .push(rect);
                        }
                    }
                }
            } else {
                // Cross-page annotation
                // First page
                if let Some(text_map) = cache.get_or_build(ann.start_page, doc) {
                    let x_offset = page_pictures
                        .get(ann.start_page)
                        .map(|pic| calculate_picture_offset(pic))
                        .unwrap_or(0.0);

                    for idx in ann.start_word..text_map.word_count() {
                        if let Some(word) = text_map.get_word(idx) {
                            let rect = HighlightRect::from_pdf_bounds(
                                &word.bounds,
                                text_map.page_width,
                                text_map.page_height,
                                x_offset,
                                render_width,
                            );
                            page_ann_rects
                                .entry(ann.start_page)
                                .or_insert_with(Vec::new)
                                .push(rect);
                        }
                    }
                }

                // Middle pages
                for page_idx in (ann.start_page + 1)..ann.end_page {
                    if let Some(text_map) = cache.get_or_build(page_idx, doc) {
                        let x_offset = page_pictures
                            .get(page_idx)
                            .map(|pic| calculate_picture_offset(pic))
                            .unwrap_or(0.0);

                        for idx in 0..text_map.word_count() {
                            if let Some(word) = text_map.get_word(idx) {
                                let rect = HighlightRect::from_pdf_bounds(
                                    &word.bounds,
                                    text_map.page_width,
                                    text_map.page_height,
                                    x_offset,
                                    render_width,
                                );
                                page_ann_rects
                                    .entry(page_idx)
                                    .or_insert_with(Vec::new)
                                    .push(rect);
                            }
                        }
                    }
                }

                // Last page
                if let Some(text_map) = cache.get_or_build(ann.end_page, doc) {
                    let x_offset = page_pictures
                        .get(ann.end_page)
                        .map(|pic| calculate_picture_offset(pic))
                        .unwrap_or(0.0);

                    for idx in 0..=ann.end_word {
                        if let Some(word) = text_map.get_word(idx) {
                            let rect = HighlightRect::from_pdf_bounds(
                                &word.bounds,
                                text_map.page_width,
                                text_map.page_height,
                                x_offset,
                                render_width,
                            );
                            page_ann_rects
                                .entry(ann.end_page)
                                .or_insert_with(Vec::new)
                                .push(rect);
                        }
                    }
                }
            }
        }

        // Apply annotation highlights to overlays
        let overlays = imp.pdf_view.highlight_overlays();
        for (page_index, overlay) in overlays.iter().enumerate() {
            let rects = page_ann_rects.remove(&page_index).unwrap_or_default();
            overlay.set_annotations(rects);
        }
    }

    pub fn annotation_panel(&self) -> &AnnotationPanel {
        &self.imp().annotation_panel
    }
}
