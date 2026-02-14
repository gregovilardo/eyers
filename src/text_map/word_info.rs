use pdfium_render::prelude::PdfRect;

/// Information about a single word extracted from a PDF page
#[derive(Debug, Clone)]
pub struct WordInfo {
    /// The word text
    pub text: String,
    /// Start character index in the page's full text
    pub char_start: usize,
    /// End character index (exclusive) in the page's full text
    pub char_end: usize,
    /// Bounding box in PDF coordinate space (origin at bottom-left)
    pub bounds: PdfRect,
    /// Center point (x, y) in PDF coordinates for navigation
    pub center_x: f64,
    pub center_y: f64,
    /// Which line this word belongs to (for j/k navigation)
    pub line_index: usize,
    pub surround_left: Option<String>,
}

impl WordInfo {
    /// Create a new WordInfo with computed center
    pub fn new(
        text: String,
        char_start: usize,
        char_end: usize,
        bounds: PdfRect,
        line_index: usize,
        surround_left: Option<String>,
    ) -> Self {
        let center_x = (bounds.left().value as f64 + bounds.right().value as f64) / 2.0;
        let center_y = (bounds.bottom().value as f64 + bounds.top().value as f64) / 2.0;

        Self {
            text,
            char_start,
            char_end,
            bounds,
            center_x,
            center_y,
            line_index,
            surround_left,
        }
    }
}

/// Information about a line of text on a page
#[derive(Debug, Clone)]
pub struct LineInfo {
    /// Range of word indices that belong to this line (in the page's words Vec)
    pub word_start: usize,
    pub word_end: usize,
    /// Average y-center of words on this line (PDF coordinates)
    pub y_center: f64,
}

impl LineInfo {
    pub fn new(word_start: usize, word_end: usize, y_center: f64) -> Self {
        Self {
            word_start,
            word_end,
            y_center,
        }
    }

    /// Returns the number of words on this line
    pub fn word_count(&self) -> usize {
        self.word_end - self.word_start
    }
}
