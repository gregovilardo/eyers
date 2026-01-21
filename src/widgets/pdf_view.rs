use glib::Properties;
use glib::subclass::Signal;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{Box, GestureClick, Orientation, Picture};
use pdfium_render::prelude::*;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::sync::OnceLock;

use crate::services::pdf_text::{
    self, calculate_click_coordinates, calculate_page_dimensions, create_render_config,
    extract_word_at_index, find_char_index_at_click,
};
use crate::widgets::DefinitionPopover;

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

    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::PdfView)]
    pub struct PdfView {
        pub document: RefCell<Option<PdfDocument<'static>>>,
        pub pdfium: RefCell<Option<&'static Pdfium>>,
        pub current_popover: RefCell<Option<DefinitionPopover>>,

        // Pictures for each page (for popover positioning)
        pub(super) page_pictures: RefCell<Vec<Picture>>,

        // Selection state for translate mode
        pub selection_start: RefCell<Option<SelectionPoint>>,

        #[property(get, set, default = false)]
        pub definitions_enabled: Cell<bool>,

        #[property(get, set, default = false)]
        pub translate_enabled: Cell<bool>,
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
                vec![
                    Signal::builder("translate-requested")
                        .param_types([String::static_type()])
                        .build(),
                ]
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

        self.imp().document.replace(Some(document));
        self.render_pages();

        Ok(())
    }

    fn clear(&self) {
        while let Some(child) = self.first_child() {
            self.remove(&child);
        }
        self.imp().page_pictures.borrow_mut().clear();
    }

    fn render_pages(&self) {
        let doc_borrow = self.imp().document.borrow();
        let doc = match doc_borrow.as_ref() {
            Some(d) => d,
            None => return,
        };

        let mut page_pictures = Vec::new();

        for (index, page) in doc.pages().iter().enumerate() {
            if let Some(picture) = self.render_single_page(&page, index) {
                self.append(&picture);
                page_pictures.push(picture);
            }
        }

        self.imp().page_pictures.replace(page_pictures);
    }

    fn render_single_page(&self, page: &PdfPage, page_index: usize) -> Option<Picture> {
        let config = create_render_config();
        let bitmap = page.render_with_config(&config).ok()?;

        let dimensions = calculate_page_dimensions(&bitmap);
        let texture = self.create_texture_from_bitmap(&bitmap, &dimensions);

        let picture = Picture::builder()
            .can_shrink(false)
            .paintable(&texture)
            .build();

        self.setup_page_gesture(&picture, page_index);

        Some(picture)
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

        let click = calculate_click_coordinates(x, y, &page);

        let page_pictures = self.imp().page_pictures.borrow();
        let picture = match page_pictures.get(page_index) {
            Some(p) => p,
            None => return,
        };

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

        let click = calculate_click_coordinates(x, y, &page);

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
