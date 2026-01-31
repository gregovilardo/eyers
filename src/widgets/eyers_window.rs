use gtk::gio;
use gtk::glib;
use gtk::glib::closure_local;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{ApplicationWindow, Box, Orientation, Paned, PolicyType, ScrolledWindow};
use pdfium_render::prelude::*;
use std::cell::{Cell, RefCell};
use std::path::Path;

use crate::modes::key_handler::ScrollDir;
use crate::modes::key_handler::handle_post_global_key;
use crate::modes::key_handler::handle_pre_global_key;
use crate::modes::{
    AppMode, KeyAction, WordCursor, handle_normal_mode_key, handle_visual_mode_key,
};
use crate::services::pdf_text::calculate_picture_offset;
use crate::text_map::page_text_map::CharSpacing;
use crate::text_map::page_text_map::TextToken;
use crate::text_map::{TextMapCache, find_word_on_line_starting_with};
use crate::widgets::{EyersHeaderBar, HighlightRect, PdfView, TocPanel, TranslationPanel};

const DEFAULT_VIEWPORT_OFFSET: f64 = 0.2;

mod imp {
    use super::*;

    pub struct EyersWindow {
        pub header_bar: EyersHeaderBar,
        pub pdf_view: PdfView,
        pub toc_panel: TocPanel,
        pub scrolled_window: RefCell<Option<ScrolledWindow>>,
        pub translation_panel: TranslationPanel,
        pub pdfium: RefCell<Option<&'static Pdfium>>,
        pub paned: RefCell<Option<Paned>>,
        pub app_mode: RefCell<AppMode>,
        pub text_cache: RefCell<Option<TextMapCache>>,
        /// Toast revealer for copy feedback
        pub toast_revealer: gtk::Revealer,
        /// Toast label for displaying message
        pub toast_label: gtk::Label,
        pub keyaction_state: Cell<KeyAction>,
        pub pending_number: Cell<u32>,
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
                pdfium: RefCell::new(None),
                paned: RefCell::new(None),
                app_mode: RefCell::new(AppMode::default()),
                text_cache: RefCell::new(None),
                toast_revealer,
                toast_label,
                keyaction_state: Cell::new(KeyAction::Empty),
                pending_number: Cell::new(0),
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

        // Set up toast notification
        self.setup_toast();

        // Create overlay for toast notifications
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&main_box));
        overlay.add_overlay(&imp.toast_revealer);

        self.set_child(Some(&overlay));

        self.setup_translation_panel();
        self.setup_toc_panel();
        self.setup_keyboard_controller();
        self.setup_scroll_tracking();
        self.setup_page_indicator_label();
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

                let keyaction_state = window.keyaction_state();
                //None -> proceed
                //Empty -> Stop
                if let Some(action) =
                    handle_pre_global_key(key, modifiers, toc_visible, keyaction_state)
                {
                    window.set_keyaction_state(action);
                    if window.execute_key_action(action) {
                        return glib::Propagation::Stop;
                    }
                }

                // Handle mode-based key events
                if window.handle_mode_key(key) {
                    return glib::Propagation::Stop;
                }

                if let Some(action) = handle_post_global_key(key) {
                    window.set_keyaction_state(action);
                    if window.execute_key_action(action) {
                        return glib::Propagation::Stop;
                    }
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

        let keyaction_state = self.keyaction_state();

        if let Some(action) = match &mode {
            AppMode::Normal => handle_normal_mode_key(key, keyaction_state),
            AppMode::Visual { .. } => {
                let doc_borrow = imp.pdf_view.document();
                if let Some(ref doc) = *doc_borrow {
                    let mut cache = imp.text_cache.borrow_mut();
                    if let Some(ref mut cache) = *cache {
                        handle_visual_mode_key(key, &mode, cache, doc, keyaction_state)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        } {
            self.set_keyaction_state(action);
            return self.execute_key_action(action);
        };
        false
    }

    /// Execute a key action
    fn execute_key_action(&self, action: KeyAction) -> bool {
        let imp = self.imp();

        let result = match action {
            KeyAction::Empty => true,

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

            KeyAction::ScrollViewport {
                x_percent,
                y_percent,
            } => {
                self.scroll_by_percent(x_percent, y_percent);
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
                    // self.print_cursor_word(cursor);
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
                // println!(
                //     "Cursor moved to page {} word {}",
                //     cursor.page_index, cursor.word_index
                // );
                {
                    let mut mode = imp.app_mode.borrow_mut();
                    mode.set_cursor(cursor);
                }
                imp.pdf_view.set_cursor(Some(cursor));
                // Update selection display to sync anchor-to-cursor range if selection is active
                self.update_selection_display();
                self.ensure_cursor_visible(cursor);
                // self.print_cursor_word(cursor);
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
                // Toggle: if popover is open, close it; otherwise show definition
                if imp.pdf_view.has_popover() {
                    imp.pdf_view.close_current_popover();
                } else {
                    self.show_definition_for_cursor(cursor);
                }
                true
            }

            KeyAction::Translate { start, end } => {
                // Toggle: if translation panel is visible, hide it; otherwise translate
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

            KeyAction::ScrollWithGG => {
                self.scroll_with_g(self.imp().pending_number.get());
                true
            }

            KeyAction::ScrollToEnd => {
                self.scroll_to_document_end();
                true
            }

            KeyAction::PendingG => true,
            KeyAction::PendingForward => true,
            KeyAction::PendingBackward => true,
            KeyAction::PendingNumber { number } => {
                self.imp().pending_number.set(number);
                true
            }

            KeyAction::FindForward { letter } => {
                // !TODO: Change this function, horrible boolean value
                self.execute_find(letter, true);
                true
            }

            KeyAction::FindBackward { letter } => {
                self.execute_find(letter, false);
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
        };

        if !matches!(action, KeyAction::PendingNumber { number: _ }) {
            if !matches!(action, KeyAction::PendingG) {
                self.imp().pending_number.set(0);
            }
        }

        return result;
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

    fn scroll_with_g(&self, page_number: u32) {
        if page_number == 0 {
            self.scroll_to_document_start();
            return;
        }
        self.scroll_to_page(page_number);
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
                                let distance = (word.center_y() - target_pdf_y).abs();
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
        let mode = self.imp().app_mode.borrow();
        self.imp().header_bar.set_mode_text(mode.display_name());
    }

    /// Debug helper: print the word at cursor position
    fn print_cursor_word(&self, cursor: WordCursor) {
        let imp = self.imp();
        let cache = imp.text_cache.borrow();
        if let Some(ref cache) = *cache {
            if let Some(text_map) = cache.get(cursor.page_index) {
                if let Some(word) = text_map.get_word(cursor.word_index) {
                    println!("  -> Word: '{}' (line {})", word.text(), word.line_index());
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
                        word.bounds(),
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
                                word.bounds(),
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
                                word.bounds(),
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
                                    word.bounds(),
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
                                word.bounds(),
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
        let word_y_screen = page_top + (text_map.page_height - word.center_y()) * scale;

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
        let word_text = word.text();
        println!("Definition for: {}", word_text);

        // Use the definition popover
        let page_pictures = imp.pdf_view.page_pictures();
        if let Some(pic) = page_pictures.get(cursor.page_index) {
            // Calculate screen position for popover (including x_offset for centering)
            let render_width =
                crate::services::pdf_text::get_render_width_for_zoom(imp.pdf_view.zoom_level());
            let scale = render_width as f64 / text_map.page_width;
            let x_offset = calculate_picture_offset(pic);
            let screen_x = word.center_x() * scale + x_offset;
            let screen_y = (text_map.page_height - word.center_y()) * scale;

            let popover = crate::widgets::DefinitionPopover::new();
            popover.show_at(pic, screen_x, screen_y);
            popover.fetch_and_display(word_text.clone(), word_text.to_lowercase());

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
                        text_parts.push(word.text());
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
                        text_parts.push(word.text());
                    }
                }
            }

            // Middle pages: all words
            for page_idx in (first.page_index + 1)..last.page_index {
                if let Some(text_map) = cache.get(page_idx) {
                    for idx in 0..text_map.word_count() {
                        if let Some(word) = text_map.get_word(idx) {
                            text_parts.push(word.text());
                        }
                    }
                }
            }

            // Last page: from start to end.word_index
            if let Some(text_map) = cache.get(last.page_index) {
                for idx in 0..=last.word_index {
                    if let Some(word) = text_map.get_word(idx) {
                        text_parts.push(word.text());
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
        let mut text_parts: Vec<&TextToken> = Vec::new();

        if start.page_index == end.page_index {
            // Same page
            if let Some(text_map) = cache.get(start.page_index) {
                let word_start = start.word_index.min(end.word_index);
                let word_end = start.word_index.max(end.word_index);

                for idx in word_start..=word_end {
                    if let Some(word) = text_map.get_word(idx) {
                        text_parts.push(word);
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
                        text_parts.push(word);
                    }
                }
            }

            // Middle pages
            for page_idx in (first.page_index + 1)..last.page_index {
                if let Some(text_map) = cache.get(page_idx) {
                    for idx in 0..text_map.word_count() {
                        if let Some(word) = text_map.get_word(idx) {
                            text_parts.push(word);
                        }
                    }
                }
            }

            // Last page
            if let Some(text_map) = cache.get(last.page_index) {
                for idx in 0..=last.word_index {
                    if let Some(word) = text_map.get_word(idx) {
                        text_parts.push(word);
                    }
                }
            }
        }

        let res: Vec<String> = text_parts.iter().map(|token| token.to_string()).collect();
        res.join("")
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

    /// Open a PDF file from a path (public API for CLI usage)
    pub fn open_file(&self, path: &Path) {
        if let Err(e) = self.imp().pdf_view.load_pdf(path.to_path_buf()) {
            eprintln!("{}", e);
            return;
        }

        self.init_text_cache();
        self.extract_and_populate_bookmarks();

        // Reset to Normal mode when loading new PDF
        {
            let mut mode = self.imp().app_mode.borrow_mut();
            *mode = AppMode::exit_to_normal();
        }
        self.update_mode_display();
        self.pdf_view().set_cursor(None);
        self.pdf_view().clear_selection();
        self.pdf_view().clear_all_highlights();
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

    pub fn set_keyaction_state(&self, keyaction: KeyAction) {
        self.imp().keyaction_state.set(keyaction);
    }
    pub fn keyaction_state(&self) -> KeyAction {
        self.imp().keyaction_state.get()
    }
}
