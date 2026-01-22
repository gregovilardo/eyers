use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use pdfium_render::prelude::PdfRect;
use std::cell::RefCell;

use crate::services::pdf_text::RENDER_WIDTH;

/// A rectangle in screen coordinates for highlighting
#[derive(Debug, Clone, Copy)]
pub struct HighlightRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl HighlightRect {
    /// Create a HighlightRect from PDF bounds and page dimensions
    ///
    /// PDF coordinates: origin at bottom-left, y increases upward
    /// Screen coordinates: origin at top-left, y increases downward
    pub fn from_pdf_bounds(bounds: &PdfRect, page_width: f64, page_height: f64) -> Self {
        let scale = RENDER_WIDTH as f64 / page_width;

        // PDF coords -> screen coords
        // screen_x = pdf_x * scale
        // screen_y = (page_height - pdf_top) * scale (flip y-axis)
        let x = bounds.left().value as f64 * scale;
        let y = (page_height - bounds.top().value as f64) * scale;
        let width = (bounds.right().value - bounds.left().value) as f64 * scale;
        let height = (bounds.top().value - bounds.bottom().value) as f64 * scale;

        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Highlight data for a page
#[derive(Debug, Clone, Default)]
pub struct PageHighlights {
    /// Cursor highlight (single word)
    pub cursor: Option<HighlightRect>,
    /// Selection highlights (multiple words, one rect per word)
    pub selection: Vec<HighlightRect>,
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct HighlightOverlay {
        pub highlights: RefCell<PageHighlights>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for HighlightOverlay {
        const NAME: &'static str = "HighlightOverlay";
        type Type = super::HighlightOverlay;
        type ParentType = gtk::DrawingArea;
    }

    impl ObjectImpl for HighlightOverlay {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_drawing();
        }
    }

    impl WidgetImpl for HighlightOverlay {}
    impl DrawingAreaImpl for HighlightOverlay {}
}

glib::wrapper! {
    pub struct HighlightOverlay(ObjectSubclass<imp::HighlightOverlay>)
        @extends gtk::DrawingArea, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl HighlightOverlay {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    fn setup_drawing(&self) {
        // Make the overlay transparent to clicks
        self.set_can_target(false);

        // Set up the draw function
        let overlay_weak = self.downgrade();
        self.set_draw_func(move |_area, cr, _width, _height| {
            if let Some(overlay) = overlay_weak.upgrade() {
                overlay.draw(cr);
            }
        });
    }

    fn draw(&self, cr: &gtk::cairo::Context) {
        let highlights = self.imp().highlights.borrow();

        // Draw selection highlights first (behind cursor)
        for rect in &highlights.selection {
            self.draw_selection_rect(cr, rect);
        }

        // Draw cursor highlight on top
        if let Some(cursor_rect) = &highlights.cursor {
            println!(
                "Drawing cursor at ({}, {}) size {}x{}",
                cursor_rect.x, cursor_rect.y, cursor_rect.width, cursor_rect.height
            );
            self.draw_cursor_rect(cr, cursor_rect);
        }
    }

    fn draw_cursor_rect(&self, cr: &gtk::cairo::Context, rect: &HighlightRect) {
        // Blue with ~40% opacity for cursor
        cr.set_source_rgba(0.2, 0.4, 0.8, 0.4);
        cr.rectangle(rect.x, rect.y, rect.width, rect.height);
        let _ = cr.fill();

        // Add a subtle border
        cr.set_source_rgba(0.2, 0.4, 0.8, 0.7);
        cr.set_line_width(1.5);
        cr.rectangle(rect.x, rect.y, rect.width, rect.height);
        let _ = cr.stroke();
    }

    fn draw_selection_rect(&self, cr: &gtk::cairo::Context, rect: &HighlightRect) {
        // Lighter blue with ~25% opacity for selection
        cr.set_source_rgba(0.3, 0.5, 0.9, 0.25);
        cr.rectangle(rect.x, rect.y, rect.width, rect.height);
        let _ = cr.fill();
    }

    /// Set the cursor highlight
    pub fn set_cursor(&self, rect: Option<HighlightRect>) {
        self.imp().highlights.borrow_mut().cursor = rect;
        self.queue_draw();
    }

    /// Set the selection highlights
    pub fn set_selection(&self, rects: Vec<HighlightRect>) {
        self.imp().highlights.borrow_mut().selection = rects;
        self.queue_draw();
    }

    /// Clear all highlights
    pub fn clear(&self) {
        let mut highlights = self.imp().highlights.borrow_mut();
        highlights.cursor = None;
        highlights.selection.clear();
        self.queue_draw();
    }

    /// Update both cursor and selection at once
    pub fn set_highlights(&self, cursor: Option<HighlightRect>, selection: Vec<HighlightRect>) {
        let mut highlights = self.imp().highlights.borrow_mut();
        highlights.cursor = cursor;
        highlights.selection = selection;
        drop(highlights);
        self.queue_draw();
    }
}

impl Default for HighlightOverlay {
    fn default() -> Self {
        Self::new()
    }
}
