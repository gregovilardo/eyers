use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{Box, GestureClick, Orientation, Picture};
use pdfium_render::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;

use crate::services::pdf_text::{
    self, calculate_click_coordinates, calculate_page_dimensions, create_render_config,
    extract_word_at_index, find_char_index_at_click,
};
use crate::widgets::DefinitionPopover;

mod imp {
    use super::*;

    pub struct PdfView {
        pub document: RefCell<Option<PdfDocument<'static>>>,
        pub pdfium: RefCell<Option<&'static Pdfium>>,
        pub current_popover: RefCell<Option<DefinitionPopover>>,
    }

    impl Default for PdfView {
        fn default() -> Self {
            Self {
                document: RefCell::new(None),
                pdfium: RefCell::new(None),
                current_popover: RefCell::new(None),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PdfView {
        const NAME: &'static str = "PdfView";
        type Type = super::PdfView;
        type ParentType = Box;
    }

    impl ObjectImpl for PdfView {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_widgets();
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
    }

    fn render_pages(&self) {
        let doc_borrow = self.imp().document.borrow();
        let doc = match doc_borrow.as_ref() {
            Some(d) => d,
            None => return,
        };

        for (index, page) in doc.pages().iter().enumerate() {
            if let Some(picture) = self.render_single_page(&page) {
                self.setup_page_gesture(&picture, index);
                self.append(&picture);
            }
        }
    }

    fn render_single_page(&self, page: &PdfPage) -> Option<Picture> {
        let config = create_render_config();
        let bitmap = page.render_with_config(&config).ok()?;

        let dimensions = calculate_page_dimensions(&bitmap);
        let texture = self.create_texture_from_bitmap(&bitmap, &dimensions);

        Some(
            Picture::builder()
                .can_shrink(false)
                .paintable(&texture)
                .build(),
        )
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
        let picture_clone = picture.clone();

        gesture.connect_pressed(move |_, _, x, y| {
            if let Some(view) = view_weak.upgrade() {
                view.handle_page_click(x, y, page_index, &picture_clone);
            }
        });

        picture.add_controller(gesture);
    }

    fn handle_page_click(&self, x: f64, y: f64, page_index: usize, picture: &Picture) {
        // Close any existing popover first
        self.close_current_popover();

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
        self.process_click_on_page(&page, &click, picture);
    }

    fn process_click_on_page(
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
