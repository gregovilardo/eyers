use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{ApplicationWindow, Box, Orientation, Paned, PolicyType, ScrolledWindow};
use pdfium_render::prelude::*;
use std::cell::RefCell;
use std::path::Path;

use crate::modes::{
    handle_normal_mode_key, handle_visual_mode_key, AppMode, KeyAction, WordCursor,
};
use crate::text_map::TextMapCache;
use crate::widgets::{EyersHeaderBar, HighlightRect, PdfView, TocPanel, TranslationPanel};

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
    }

    impl Default for EyersWindow {
        fn default() -> Self {
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
        let bindings =
            Pdfium::bind_to_library(Path::new("./libpdfium.so")).expect("Failed to bind to PDFium");
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

        self.set_child(Some(&main_box));

        self.setup_translation_panel();
        self.setup_toc_panel();
        self.setup_keyboard_controller();
        self.setup_scroll_tracking();
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
                pdf_view.scroll_to_page(page_index as u16);
            }),
        );
    }

    fn setup_keyboard_controller(&self) {
        let controller = gtk::EventControllerKey::new();
        let window_weak = self.downgrade();

        controller.connect_key_pressed(move |_, key, _, _| {
            if let Some(window) = window_weak.upgrade() {
                let imp = window.imp();
                let toc_visible = imp.toc_panel.is_visible();

                if key == gtk::gdk::Key::Tab {
                    window.toggle_toc_panel();
                    return glib::Propagation::Stop;
                }

                if toc_visible {
                    match key {
                        gtk::gdk::Key::j | gtk::gdk::Key::Down => {
                            imp.toc_panel.select_next();
                            return glib::Propagation::Stop;
                        }
                        gtk::gdk::Key::k | gtk::gdk::Key::Up => {
                            imp.toc_panel.select_prev();
                            return glib::Propagation::Stop;
                        }
                        gtk::gdk::Key::Return => {
                            imp.toc_panel.navigate_and_close();
                            return glib::Propagation::Stop;
                        }
                        gtk::gdk::Key::Escape => {
                            imp.toc_panel.set_visible(false);
                            return glib::Propagation::Stop;
                        }
                        _ => {}
                    }
                } else {
                    // Handle mode-based key events
                    if window.handle_mode_key(key) {
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

        // Need document to be loaded for mode operations
        if !imp.pdf_view.has_document() {
            return false;
        }

        let mode = imp.app_mode.borrow().clone();

        let action = match &mode {
            AppMode::Normal => handle_normal_mode_key(key),
            AppMode::Visual { .. } => {
                let doc_borrow = imp.pdf_view.document();
                if let Some(ref doc) = *doc_borrow {
                    let mut cache = imp.text_cache.borrow_mut();
                    if let Some(ref mut cache) = *cache {
                        handle_visual_mode_key(key, &mode, cache, doc)
                    } else {
                        return false;
                    }
                } else {
                    return false;
                }
            }
        };

        self.execute_key_action(action)
    }

    /// Execute a key action
    fn execute_key_action(&self, action: KeyAction) -> bool {
        let imp = self.imp();

        match action {
            KeyAction::None => false,

            KeyAction::Scroll {
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
                    self.print_cursor_word(cursor);
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
                println!(
                    "Cursor moved to page {} word {}",
                    cursor.page_index, cursor.word_index
                );
                {
                    let mut mode = imp.app_mode.borrow_mut();
                    mode.set_cursor(cursor);
                }
                imp.pdf_view.set_cursor(Some(cursor));
                self.update_highlights();
                self.ensure_cursor_visible(cursor);
                self.print_cursor_word(cursor);
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
                self.show_definition_for_cursor(cursor);
                true
            }

            KeyAction::Translate { start, end } => {
                self.translate_range(start, end);
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

        let mut cache = imp.text_cache.borrow_mut();
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
                        let scale = crate::services::pdf_text::RENDER_WIDTH as f64 / page_width_pts;

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

        // Build a map of page_index -> (cursor_rect, selection_rects)
        let mut page_highlights: std::collections::HashMap<
            usize,
            (Option<HighlightRect>, Vec<HighlightRect>),
        > = std::collections::HashMap::new();

        // Add cursor highlight
        if let Some(cursor) = cursor {
            if let Some(text_map) = cache.get(cursor.page_index) {
                if let Some(word) = text_map.get_word(cursor.word_index) {
                    let rect = HighlightRect::from_pdf_bounds(
                        &word.bounds,
                        text_map.page_width,
                        text_map.page_height,
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
                    for idx in first.word_index..=last.word_index {
                        if let Some(word) = text_map.get_word(idx) {
                            let rect = HighlightRect::from_pdf_bounds(
                                &word.bounds,
                                text_map.page_width,
                                text_map.page_height,
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
                    for idx in first.word_index..text_map.word_count() {
                        if let Some(word) = text_map.get_word(idx) {
                            let rect = HighlightRect::from_pdf_bounds(
                                &word.bounds,
                                text_map.page_width,
                                text_map.page_height,
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
                        for idx in 0..text_map.word_count() {
                            if let Some(word) = text_map.get_word(idx) {
                                let rect = HighlightRect::from_pdf_bounds(
                                    &word.bounds,
                                    text_map.page_width,
                                    text_map.page_height,
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
                    for idx in 0..=last.word_index {
                        if let Some(word) = text_map.get_word(idx) {
                            let rect = HighlightRect::from_pdf_bounds(
                                &word.bounds,
                                text_map.page_width,
                                text_map.page_height,
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
        let doc = match doc_borrow.as_ref() {
            Some(d) => d,
            None => return,
        };

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
        let scale = crate::services::pdf_text::RENDER_WIDTH as f64 / text_map.page_width;
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
        // For now, we'll use the translation panel to show the definition
        // TODO: Use the definition popover positioned at the word
        let word_text = word.text.clone();
        println!("Definition for: {}", word_text);

        // Use the definition popover
        if let Some(picture) = imp.pdf_view.page_picture(cursor.page_index as u16) {
            let page_pictures = imp.pdf_view.page_pictures();
            if let Some(pic) = page_pictures.get(cursor.page_index) {
                // Calculate screen position for popover
                let scale = crate::services::pdf_text::RENDER_WIDTH as f64 / text_map.page_width;
                let screen_x = word.center_x * scale;
                let screen_y = (text_map.page_height - word.center_y) * scale;

                let popover = crate::widgets::DefinitionPopover::new();
                popover.show_at(pic, screen_x, screen_y);
                popover.fetch_and_display(word_text.clone(), word_text.to_lowercase());

                imp.pdf_view.set_current_popover(Some(popover));
            }
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

        if let Err(e) = self.imp().pdf_view.load_pdf(path) {
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
        self.imp().pdf_view.set_cursor(None);
        self.imp().pdf_view.clear_selection();
        self.imp().pdf_view.clear_all_highlights();
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
}
