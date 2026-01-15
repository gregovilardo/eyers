use pdfium_render::prelude::*;

pub const RENDER_WIDTH: i32 = 1000;
const CLICK_TOLERANCE: f64 = 5.0;

/// Data extracted from a click event on a PDF page
pub struct ClickData {
    pub pdf_x: f64,
    pub pdf_y: f64,
    pub screen_x: f64,
    pub screen_y: f64,
}

/// Word extracted from PDF text
pub struct ExtractedWord {
    pub original: String,
    pub lowercase: String,
}

/// Configuration for rendering a PDF page
pub struct PageRenderConfig {
    pub width: i32,
    pub height: i32,
    pub stride: usize,
}

pub fn calculate_click_coordinates(x: f64, y: f64, page: &PdfPage) -> ClickData {
    let page_width_pts = page.width().value as f64;
    let page_height_pts = page.height().value as f64;
    let scale = RENDER_WIDTH as f64 / page_width_pts;

    ClickData {
        pdf_x: x / scale,
        pdf_y: page_height_pts - (y / scale),
        screen_x: x,
        screen_y: y,
    }
}

pub fn create_click_rect(click: &ClickData) -> PdfRect {
    PdfRect::new_from_values(
        (click.pdf_y - CLICK_TOLERANCE) as f32,
        (click.pdf_x - CLICK_TOLERANCE) as f32,
        (click.pdf_y + CLICK_TOLERANCE) as f32,
        (click.pdf_x + CLICK_TOLERANCE) as f32,
    )
}

pub fn find_char_index_at_click(text_page: &PdfPageText, click: &ClickData) -> Option<usize> {
    let rect = create_click_rect(click);
    let chars = text_page.chars_inside_rect(rect).ok()?;
    let char_obj = chars.iter().next()?;
    Some(char_obj.index() as usize)
}

pub fn extract_word_at_index(full_text: &str, idx: usize) -> Option<ExtractedWord> {
    let chars_vec: Vec<char> = full_text.chars().collect();
    if idx >= chars_vec.len() {
        return None;
    }

    let start = find_word_start(&chars_vec, idx);
    let end = find_word_end(&chars_vec, idx);

    if start > end {
        return None;
    }
    let original: String = chars_vec[start..end].iter().collect();
    let lowercase = original.to_lowercase();
    Some(ExtractedWord {
        original,
        lowercase,
    })
}

fn find_word_start(chars: &[char], idx: usize) -> usize {
    let mut start = idx;
    while start > 0 && is_word_char(chars[start]) {
        start -= 1;
    }
    if !is_word_char(chars[start]) {
        start += 1;
    }
    start
}

fn find_word_end(chars: &[char], idx: usize) -> usize {
    let mut end = idx;
    while end < chars.len() && is_word_char(chars[end]) {
        end += 1;
    }
    end
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '\''
}

pub fn calculate_page_dimensions(bitmap: &PdfBitmap) -> PageRenderConfig {
    let width = bitmap.width();
    let height = bitmap.height();
    PageRenderConfig {
        width,
        height,
        stride: (width * 4) as usize,
    }
}

pub fn create_render_config() -> PdfRenderConfig {
    PdfRenderConfig::new()
        .set_target_width(RENDER_WIDTH)
        .set_format(PdfBitmapFormat::BGRA)
}
