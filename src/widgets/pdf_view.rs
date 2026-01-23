use glib::subclass::Signal;
use glib::Properties;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{Box, GestureClick, Orientation, Overlay, Picture};
use pdfium_render::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::OnceLock;

use crate::modes::WordCursor;
use crate::services::bookmarks;
use crate::services::pdf_text::{
    self, calculate_click_coordinates_with_offset, calculate_page_dimensions,
    calculate_picture_offset, create_render_config_with_zoom, extract_word_at_index,
    find_char_index_at_click,
};
use crate::widgets::DefinitionPopover;
use crate::widgets::HighlightOverlay;

/// Represents a selection point in the PDF
#[derive(Clone, Debug)]
pub struct SelectionPoint {
    pub page_index: usize,
    pub char_index: usize,
    pub word_start: usize,
    pub word_end: usize,
    pub word: String,
}

mod imp {
    use super::*;

    #[derive(Properties)]
    #[properties(wrapper_type = super::PdfView)]
    pub struct PdfView {
        pub document: RefCell<Option<PdfDocument<'static>>>,
        pub pdfium: RefCell<Option<&'static Pdfium>>,
        pub current_popover: RefCell<Option<DefinitionPopover>>,
        pub bookmarks: RefCell<Option<Vec<bookmarks::BookmarkEntry>>>,
        pub(super) page_pictures: RefCell<Vec<Picture>>,
        pub(super) page_overlays: RefCell<Vec<Overlay>>,
        pub(super) highlight_overlays: RefCell<Vec<HighlightOverlay>>,
        /// Tracks which pages have been rendered at current zoom level
        pub(super) rendered_pages: RefCell<HashSet<usize>>,
        pub selection_start: RefCell<Option<SelectionPoint>>,
        pub current_page: Cell<u16>,
        pub pending_update: Cell<bool>,
        pub visual_cursor: RefCell<Option<WordCursor>>,
        pub visual_selection: RefCell<Option<(WordCursor, WordCursor)>>,
        /// Current zoom level (1.0 = 100%)
        pub zoom_level: Cell<f64>,
        #[property(get, set, default = false)]
        pub definitions_enabled: Cell<bool>,
        #[property(get, set, default = false)]
        pub translate_enabled: Cell<bool>,
    }

    impl Default for PdfView {
        fn default() -> Self {
            Self {
                document: RefCell::new(None),
                pdfium: RefCell::new(None),
                current_popover: RefCell::new(None),
                bookmarks: RefCell::new(None),
                page_pictures: RefCell::new(Vec::new()),
                page_overlays: RefCell::new(Vec::new()),
                highlight_overlays: RefCell::new(Vec::new()),
                rendered_pages: RefCell::new(HashSet::new()),
                selection_start: RefCell::new(None),
                current_page: Cell::new(0),
                pending_update: Cell::new(false),
                visual_cursor: RefCell::new(None),
                visual_selection: RefCell::new(None),
                zoom_level: Cell::new(1.0),
                definitions_enabled: Cell::new(false),
                translate_enabled: Cell::new(false),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PdfView {
        const NAME: &'static str = "PdfView";
        type Type = super::PdfView;
        type ParentType = Box;
    }

    #[glib::derived_properties]
    impl ObjectImpl for PdfView {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_widgets();
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![Signal::builder("translate-requested")
                    .param_types([String::static_type()])
                    .build()]
            })
        }
    }

    impl WidgetImpl for PdfView {}
    impl BoxImpl for PdfView {}
}

glib::wrapper! {
    pub struct PdfView(ObjectSubclass<imp::PdfView>)
        @extends Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl PdfView {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    fn setup_widgets(&self) {
        self.set_orientation(Orientation::Vertical);
        self.set_spacing(10);
        self.setup_scroll_tracking();
    }

    pub fn set_pdfium(&self, pdfium: &'static Pdfium) {
        self.imp().pdfium.replace(Some(pdfium));
    }

    pub fn load_pdf(&self, path: PathBuf) -> Result<(), String> {
        self.clear();
        self.close_current_popover();
        self.imp().selection_start.replace(None);

        let pdfium = self
            .imp()
            .pdfium
            .borrow()
            .ok_or_else(|| "Pdfium not initialized".to_string())?;

        let document = pdfium
            .load_pdf_from_file(&path, None)
            .map_err(|e| format!("Failed to open PDF: {}", e))?;

        let entries = bookmarks::extract_bookmarks(&document);
        self.imp().bookmarks.replace(Some(entries));

        self.imp().document.replace(Some(document));
        self.render_pages();

        Ok(())
    }

    fn clear(&self) {
        while let Some(child) = self.first_child() {
            self.remove(&child);
        }
        self.imp().page_pictures.borrow_mut().clear();
        self.imp().page_overlays.borrow_mut().clear();
        self.imp().highlight_overlays.borrow_mut().clear();
        self.imp().rendered_pages.borrow_mut().clear();
    }

    /// Calculate page dimensions at current zoom level without rendering
    fn calculate_page_size(&self, page: &PdfPage) -> (i32, i32) {
        let zoom = self.imp().zoom_level.get();
        let render_width = crate::services::pdf_text::get_render_width_for_zoom(zoom);
        let page_width_pts = page.width().value as f64;
        let page_height_pts = page.height().value as f64;
        let scale = render_width as f64 / page_width_pts;
        let height = (page_height_pts * scale) as i32;
        (render_width, height)
    }

    /// Create a placeholder Picture with the correct size (no pixel allocation)
    fn create_placeholder(&self, width: i32, height: i32) -> Picture {
        // Just set size request - no pixel buffer needed
        let picture = Picture::builder()
            .can_shrink(false)
            .width_request(width)
            .height_request(height)
            .build();

        // Add CSS class for styling (gray background)
        picture.add_css_class("pdf-placeholder");
        picture
    }

    /// Set up page structure with placeholders (fast - no rendering)
    fn render_pages(&self) {
        let doc_borrow = self.imp().document.borrow();
        let doc = match doc_borrow.as_ref() {
            Some(d) => d,
            None => return,
        };

        let mut page_pictures = Vec::new();
        let mut page_overlays = Vec::new();
        let mut highlight_overlays = Vec::new();

        for (index, page) in doc.pages().iter().enumerate() {
            let (width, height) = self.calculate_page_size(&page);

            // Create placeholder picture
            let picture = self.create_placeholder(width, height);

            // Create highlight overlay with correct size
            let highlight = HighlightOverlay::new();
            highlight.set_content_width(width);
            highlight.set_content_height(height);

            // Wrap in overlay
            let overlay = Overlay::new();
            overlay.set_child(Some(&picture));
            overlay.add_overlay(&highlight);

            self.setup_page_gesture(&picture, index);
            self.append(&overlay);

            page_pictures.push(picture);
            page_overlays.push(overlay);
            highlight_overlays.push(highlight);
        }

        self.imp().page_pictures.replace(page_pictures);
        self.imp().page_overlays.replace(page_overlays);
        self.imp().highlight_overlays.replace(highlight_overlays);
        self.imp().rendered_pages.borrow_mut().clear();

        drop(doc_borrow);

        // Render visible pages immediately
        self.render_visible_pages();
    }

    /// Render only the pages that are currently visible (plus a small buffer)
    pub fn render_visible_pages(&self) {
        let visible_range = match self.get_visible_page_range() {
            Some(range) => range,
            None => return,
        };

        let doc_borrow = self.imp().document.borrow();
        let doc = match doc_borrow.as_ref() {
            Some(d) => d,
            None => return,
        };

        let mut rendered = self.imp().rendered_pages.borrow_mut();
        let page_pictures = self.imp().page_pictures.borrow();
        let page_overlays = self.imp().page_overlays.borrow();
        let highlight_overlays = self.imp().highlight_overlays.borrow();

        // Render pages in visible range that haven't been rendered yet
        for page_index in visible_range {
            if rendered.contains(&page_index) {
                continue; // Already rendered
            }

            if let Ok(page) = doc.pages().get(page_index as u16) {
                if let Some(picture) = page_pictures.get(page_index) {
                    if let Some(overlay) = page_overlays.get(page_index) {
                        if let Some(highlight) = highlight_overlays.get(page_index) {
                            // Render the page
                            self.render_page_content(
                                &page, page_index, picture, overlay, highlight,
                            );
                            rendered.insert(page_index);
                        }
                    }
                }
            }
        }
    }

    /// Get the range of pages currently visible (with buffer)
    fn get_visible_page_range(&self) -> Option<std::ops::RangeInclusive<usize>> {
        let scrolled = self.find_scrolled_window()?;
        let adjustment = scrolled.vadjustment();
        let scroll_y = adjustment.value();
        let viewport_height = adjustment.page_size();

        let page_pictures = self.imp().page_pictures.borrow();
        if page_pictures.is_empty() {
            return None;
        }

        let spacing = 10.0;
        let mut first_visible: Option<usize> = None;
        let mut last_visible: Option<usize> = None;

        for (index, picture) in page_pictures.iter().enumerate() {
            let nat_size = picture.preferred_size().1;
            let picture_height = nat_size.height() as f64;

            let page_top = index as f64 * (picture_height + spacing);
            let page_bottom = page_top + picture_height;

            // Check if page intersects with viewport
            if page_bottom > scroll_y && page_top < scroll_y + viewport_height {
                if first_visible.is_none() {
                    first_visible = Some(index);
                }
                last_visible = Some(index);
            }
        }

        let first = first_visible.unwrap_or(0);
        let last = last_visible.unwrap_or(0);

        // Add buffer of 1 page on each side
        let buffer = 1;
        let start = first.saturating_sub(buffer);
        let end = (last + buffer).min(page_pictures.len() - 1);

        Some(start..=end)
    }

    /// Render actual content for a specific page
    fn render_page_content(
        &self,
        page: &PdfPage,
        page_index: usize,
        picture: &Picture,
        _overlay: &Overlay,
        highlight: &HighlightOverlay,
    ) {
        let zoom = self.imp().zoom_level.get();
        let config = create_render_config_with_zoom(zoom);

        let bitmap = match page.render_with_config(&config) {
            Ok(b) => b,
            Err(_) => return,
        };

        let dimensions = calculate_page_dimensions(&bitmap);
        let texture = self.create_texture_from_bitmap(&bitmap, &dimensions);

        // Update the picture's paintable and remove placeholder styling
        picture.set_paintable(Some(&texture));
        picture.remove_css_class("pdf-placeholder");

        // Update highlight overlay size (in case it changed)
        highlight.set_content_width(dimensions.width);
        highlight.set_content_height(dimensions.height);

        println!("Rendered page {}", page_index);
    }

    fn create_texture_from_bitmap(
        &self,
        bitmap: &PdfBitmap,
        config: &pdf_text::PageRenderConfig,
    ) -> gtk::gdk::MemoryTexture {
        let bytes = bitmap.as_raw_bytes();
        let bytes_glib = glib::Bytes::from(&bytes);

        gtk::gdk::MemoryTexture::new(
            config.width,
            config.height,
            gtk::gdk::MemoryFormat::B8g8r8a8,
            &bytes_glib,
            config.stride,
        )
    }

    fn setup_page_gesture(&self, picture: &Picture, page_index: usize) {
        let gesture = GestureClick::new();
        let view_weak = self.downgrade();

        gesture.connect_pressed(move |_, _, x, y| {
            if let Some(view) = view_weak.upgrade() {
                view.handle_page_click(x, y, page_index);
            }
        });

        picture.add_controller(gesture);
    }

    fn handle_page_click(&self, x: f64, y: f64, page_index: usize) {
        // Close any existing popover first
        self.close_current_popover();

        if self.definitions_enabled() {
            self.handle_definition_click(x, y, page_index);
        } else if self.translate_enabled() {
            self.handle_translate_click(x, y, page_index);
        }
    }

    fn handle_definition_click(&self, x: f64, y: f64, page_index: usize) {
        let doc_borrow = self.imp().document.borrow();
        let doc = match doc_borrow.as_ref() {
            Some(d) => d,
            None => return,
        };

        let page = match doc.pages().get(page_index as u16) {
            Ok(p) => p,
            Err(_) => return,
        };

        let page_pictures = self.imp().page_pictures.borrow();
        let picture = match page_pictures.get(page_index) {
            Some(p) => p,
            None => return,
        };

        let offset = calculate_picture_offset(picture);
        let zoom = self.zoom_level();
        let click = calculate_click_coordinates_with_offset(x, y, &page, offset, zoom);

        self.process_definition_click(&page, &click, picture);
    }

    fn process_definition_click(
        &self,
        page: &PdfPage,
        click: &pdf_text::ClickData,
        picture: &Picture,
    ) {
        let text_page = match page.text() {
            Ok(tp) => tp,
            Err(_) => return,
        };

        let char_idx = match find_char_index_at_click(&text_page, click) {
            Some(idx) => idx,
            None => {
                println!("No character found near click.");
                return;
            }
        };

        let full_text = text_page.all();
        if let Some(word) = extract_word_at_index(&full_text, char_idx) {
            let popover = DefinitionPopover::new();
            popover.show_at(picture, click.screen_x, click.screen_y);
            popover.fetch_and_display(word.original, word.lowercase);

            self.imp().current_popover.replace(Some(popover));
        }
    }

    fn handle_translate_click(&self, x: f64, y: f64, page_index: usize) {
        let doc_borrow = self.imp().document.borrow();
        let doc = match doc_borrow.as_ref() {
            Some(d) => d,
            None => return,
        };

        let page = match doc.pages().get(page_index as u16) {
            Ok(p) => p,
            Err(_) => return,
        };

        let page_pictures = self.imp().page_pictures.borrow();
        let picture = match page_pictures.get(page_index) {
            Some(p) => p,
            None => return,
        };

        let offset = calculate_picture_offset(picture);
        let zoom = self.zoom_level();
        let click = calculate_click_coordinates_with_offset(x, y, &page, offset, zoom);

        let text_page = match page.text() {
            Ok(tp) => tp,
            Err(_) => return,
        };

        let char_idx = match find_char_index_at_click(&text_page, &click) {
            Some(idx) => idx,
            None => {
                println!("No character found near click.");
                return;
            }
        };

        let full_text = text_page.all();
        let word_info = match extract_word_at_index(&full_text, char_idx) {
            Some(w) => w,
            None => return,
        };

        // Find word boundaries
        let chars: Vec<char> = full_text.chars().collect();
        let (word_start, word_end) = find_word_boundaries(&chars, char_idx);

        let selection_point = SelectionPoint {
            page_index,
            char_index: char_idx,
            word_start,
            word_end,
            word: word_info.original.clone(),
        };

        let has_start = self.imp().selection_start.borrow().is_some();

        if !has_start {
            // First click: select single word
            self.imp()
                .selection_start
                .replace(Some(selection_point.clone()));
            self.emit_by_name::<()>("translate-requested", &[&word_info.original]);
        } else {
            // Second click: select range and turn off translate mode
            let start = self.imp().selection_start.borrow().clone().unwrap();

            // Only support same-page selection for now
            if start.page_index == page_index {
                let (range_start, range_end) = if start.word_start <= word_start {
                    (start.word_start, word_end)
                } else {
                    (word_start, start.word_end)
                };

                // Extract text in range
                let text_in_range: String = chars[range_start..range_end].iter().collect();
                let text_in_range = text_in_range.trim().to_string();

                // Emit translation request
                self.emit_by_name::<()>("translate-requested", &[&text_in_range]);
            }

            // Clear selection start
            self.imp().selection_start.replace(None);

            // Turn off translate mode
            self.set_translate_enabled(false);
        }
    }

    pub fn close_current_popover(&self) {
        if let Some(popover) = self.imp().current_popover.take() {
            popover.popdown();
            popover.unparent();
        }
    }

    pub fn scroll_to_page(&self, page_index: u16) {
        println!("scrolling to page {}", page_index);
        if let Some(scrolled) = self.find_scrolled_window() {
            //TODO: find if you can stop the scroll of mouse so it can set value of adjustment
            //right
            let adjustment = scrolled.vadjustment();
            let page_pictures = self.page_pictures();

            if let Some(picture) = page_pictures.get(page_index as usize) {
                let widget = picture.upcast_ref::<gtk::Widget>();
                let natural_size = widget.preferred_size().1;
                let page_height = natural_size.height() as f64;
                let spacing = 10.0;
                let page_size = adjustment.page_size();

                let target_y = page_height * page_index as f64 + spacing * page_index as f64;
                let max_value = adjustment.upper() - page_size;

                let new_value = if target_y < 0.0 {
                    0.0
                } else if target_y > max_value {
                    max_value
                } else {
                    target_y
                };

                adjustment.set_value(new_value);
            }
        }
    }

    pub fn page_picture(&self, page_index: u16) -> Option<Picture> {
        self.imp()
            .page_pictures
            .borrow()
            .get(page_index as usize)
            .cloned()
    }

    pub fn page_pictures(&self) -> std::cell::Ref<'_, Vec<Picture>> {
        self.imp().page_pictures.borrow()
    }

    pub fn has_document(&self) -> bool {
        self.imp().document.borrow().is_some()
    }

    pub fn current_page(&self) -> u16 {
        self.imp().current_page.get()
    }

    fn setup_scroll_tracking(&self) {
        let view_weak = self.downgrade();

        let scroll_controller =
            gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);

        scroll_controller.connect_scroll(move |_, _, _| {
            if let Some(view) = view_weak.upgrade() {
                view.schedule_page_update();
            }
            glib::Propagation::Proceed
        });

        self.add_controller(scroll_controller);
    }

    pub(crate) fn schedule_page_update(&self) {
        let imp = self.imp();

        if imp.pending_update.get() {
            return;
        }

        imp.pending_update.set(true);

        let view_weak = self.downgrade();
        glib::timeout_add_local_once(std::time::Duration::from_millis(100), move || {
            if let Some(view) = view_weak.upgrade() {
                view.imp().pending_update.set(false);
                view.update_current_page();
                // Render any newly visible pages after scrolling
                view.render_visible_pages();
            }
        });
    }

    fn update_current_page(&self) {
        if let Some(page_index) = self.calculate_current_page_from_scroll() {
            self.imp().current_page.set(page_index);
        }
    }

    fn find_scrolled_window(&self) -> Option<gtk::ScrolledWindow> {
        self.parent()?.parent()?.downcast().ok()
    }

    fn calculate_current_page_from_scroll(&self) -> Option<u16> {
        let scrolled = self.find_scrolled_window()?;

        let adjustment = scrolled.vadjustment();
        let scroll_y = adjustment.value();
        let viewport_height = adjustment.page_size();
        let visible_start = scroll_y;
        let visible_end = scroll_y + viewport_height;

        let page_pictures = self.imp().page_pictures.borrow();
        let spacing = 10.0;

        for (index, picture) in page_pictures.iter().enumerate() {
            let nat_size = picture.preferred_size().1;
            let picture_height = nat_size.height() as f64;

            let page_top = index as f64 * (picture_height + spacing);
            let page_bottom = page_top + picture_height;

            if page_bottom > visible_start && page_top < visible_end {
                return Some(index as u16);
            }
        }

        if !page_pictures.is_empty() {
            return Some((page_pictures.len() - 1) as u16);
        }
        None
    }

    pub fn bookmarks(&self) -> Vec<bookmarks::BookmarkEntry> {
        self.imp().bookmarks.borrow().clone().unwrap_or_default()
    }

    /// Get a reference to the document
    pub fn document(&self) -> std::cell::Ref<'_, Option<PdfDocument<'static>>> {
        self.imp().document.borrow()
    }

    /// Get the highlight overlay for a specific page
    pub fn highlight_overlay(&self, page_index: usize) -> Option<HighlightOverlay> {
        self.imp()
            .highlight_overlays
            .borrow()
            .get(page_index)
            .cloned()
    }

    /// Get all highlight overlays
    pub fn highlight_overlays(&self) -> std::cell::Ref<'_, Vec<HighlightOverlay>> {
        self.imp().highlight_overlays.borrow()
    }

    /// Set the visual cursor position
    pub fn set_cursor(&self, cursor: Option<WordCursor>) {
        self.imp().visual_cursor.replace(cursor);
        // Note: actual highlight drawing is done by EyersWindow via update_highlights()
    }

    /// Get the current visual cursor
    pub fn cursor(&self) -> Option<WordCursor> {
        *self.imp().visual_cursor.borrow()
    }

    /// Set the visual selection range
    pub fn set_selection(&self, selection: Option<(WordCursor, WordCursor)>) {
        self.imp().visual_selection.replace(selection);
        // Note: actual highlight drawing is done by EyersWindow via update_highlights()
    }

    /// Clear the visual selection
    pub fn clear_selection(&self) {
        self.imp().visual_selection.replace(None);
        // Note: actual highlight drawing is done by EyersWindow via update_highlights()
    }

    /// Get the current visual selection
    pub fn selection(&self) -> Option<(WordCursor, WordCursor)> {
        *self.imp().visual_selection.borrow()
    }

    /// Clear all highlight overlays
    pub fn clear_all_highlights(&self) {
        for overlay in self.imp().highlight_overlays.borrow().iter() {
            overlay.clear();
        }
    }

    /// Set the current popover (for external use)
    pub fn set_current_popover(&self, popover: Option<DefinitionPopover>) {
        // Close existing popover first
        self.close_current_popover();
        self.imp().current_popover.replace(popover);
    }

    /// Check if there's a popover currently open
    pub fn has_popover(&self) -> bool {
        self.imp().current_popover.borrow().is_some()
    }

    /// Get the current zoom level
    pub fn zoom_level(&self) -> f64 {
        self.imp().zoom_level.get()
    }

    /// Set the zoom level and update page sizes
    pub fn set_zoom_level(&self, zoom: f64) {
        let clamped_zoom = zoom.clamp(0.5, 3.0);
        self.imp().zoom_level.set(clamped_zoom);
        self.update_page_sizes_for_zoom();
    }

    /// Update all page sizes for the new zoom level (fast - no rendering)
    /// Then render only visible pages
    fn update_page_sizes_for_zoom(&self) {
        let doc_borrow = self.imp().document.borrow();
        let doc = match doc_borrow.as_ref() {
            Some(d) => d,
            None => return,
        };

        let page_pictures = self.imp().page_pictures.borrow();
        let highlight_overlays = self.imp().highlight_overlays.borrow();

        // Update sizes for all pages (fast - just size request changes)
        for (index, page) in doc.pages().iter().enumerate() {
            let (width, height) = self.calculate_page_size(&page);

            if let Some(picture) = page_pictures.get(index) {
                // Just update size request - no pixel allocation
                picture.set_width_request(width);
                picture.set_height_request(height);
                // Clear any existing paintable so it shows as placeholder
                picture.set_paintable(gtk::gdk::Paintable::NONE);
                picture.add_css_class("pdf-placeholder");
            }

            if let Some(highlight) = highlight_overlays.get(index) {
                highlight.set_content_width(width);
                highlight.set_content_height(height);
            }
        }

        // Mark all pages as needing re-render
        self.imp().rendered_pages.borrow_mut().clear();

        drop(doc_borrow);
        drop(page_pictures);
        drop(highlight_overlays);

        // Render only visible pages
        self.render_visible_pages();
    }
}

impl Default for PdfView {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to find word boundaries around a character index
fn find_word_boundaries(chars: &[char], idx: usize) -> (usize, usize) {
    let mut start = idx;
    let mut end = idx;

    // Find start
    while start > 0 && is_word_char(chars[start]) {
        start -= 1;
    }
    if !is_word_char(chars[start]) {
        start += 1;
    }

    // Find end
    while end < chars.len() && is_word_char(chars[end]) {
        end += 1;
    }

    (start, end)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '\''
}
