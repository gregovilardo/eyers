use crate::text_map::page_text_map::PageTextMap;
use crate::text_map::text_map_cache::TextMapCache;

use pdfium_render::prelude::PdfDocument;

/// Direction for cursor navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavDirection {
    Left,  // Previous word in reading order (h)
    Right, // Next word in reading order (l)
    Up,    // Closest word on line above (k)
    Down,  // Closest word on line below (j)
}

/// Result of a navigation operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavResult {
    pub page_index: usize,
    pub line_index: usize,
    pub word_index: usize,
}

/// Navigate from current position in the specified direction
/// Returns the new position, or None if navigation is not possible
pub fn navigate(
    cache: &mut TextMapCache,
    document: &PdfDocument,
    current_page: usize,
    current_word: usize,
    direction: NavDirection,
) -> Option<NavResult> {
    // First, get info we need from current page without holding borrow
    let (word_count, line_count, current_line, current_x) = {
        let text_map = cache.get_or_build(current_page, document)?;
        let word_info = text_map.get_word(current_word)?;
        (
            text_map.word_count(),
            text_map.line_count(),
            word_info.line_index,
            word_info.center_x,
        )
    };

    match direction {
        NavDirection::Left => {
            navigate_left(cache, document, current_page, current_line, current_word)
        }
        NavDirection::Right => navigate_right(
            cache,
            document,
            current_page,
            current_line,
            current_word,
            word_count,
        ),
        NavDirection::Up => navigate_up(cache, document, current_page, current_line, current_x),
        NavDirection::Down => navigate_down(
            cache,
            document,
            current_page,
            current_line,
            current_x,
            line_count,
        ),
    }
}

/// Navigate to previous word in reading order
fn navigate_left(
    cache: &mut TextMapCache,
    document: &PdfDocument,
    current_page: usize,
    _current_line: usize,
    current_word: usize,
) -> Option<NavResult> {
    if current_word > 0 {
        let current_map = cache.get_or_build(current_page, document)?;
        let word = current_map.get_word(current_word - 1)?;
        // Previous word on same page
        Some(NavResult {
            page_index: current_page,
            line_index: word.line_index,
            word_index: current_word - 1,
        })
    } else if current_page > 0 {
        // Last word of previous page
        let prev_map = cache.get_or_build(current_page - 1, document)?;
        if prev_map.word_count() > 0 {
            Some(NavResult {
                page_index: current_page - 1,
                line_index: prev_map.line_count() - 1,
                word_index: prev_map.word_count() - 1,
            })
        } else {
            None
        }
    } else {
        None
    }
}

/// Navigate to next word in reading order
fn navigate_right(
    cache: &mut TextMapCache,
    document: &PdfDocument,
    current_page: usize,
    _current_line: usize,
    current_word: usize,
    word_count: usize,
) -> Option<NavResult> {
    if current_word < word_count.saturating_sub(1) {
        let current_map = cache.get_or_build(current_page, document)?;
        let word = current_map.get_word(current_word + 1)?;
        // Next word on same page
        Some(NavResult {
            page_index: current_page,
            line_index: word.line_index,
            word_index: current_word + 1,
        })
    } else if current_page < cache.page_count() - 1 {
        // First word of next page
        let next_map = cache.get_or_build(current_page + 1, document)?;
        if next_map.word_count() > 0 {
            Some(NavResult {
                page_index: current_page + 1,
                line_index: 0,
                word_index: 0,
            })
        } else {
            None
        }
    } else {
        None
    }
}

/// Navigate to closest word on line above
fn navigate_up(
    cache: &mut TextMapCache,
    document: &PdfDocument,
    current_page: usize,
    current_line: usize,
    current_x: f64,
) -> Option<NavResult> {
    if current_line > 0 {
        // Find closest word on line above (same page)
        let target_line = current_line - 1;
        let text_map = cache.get_or_build(current_page, document)?;
        let word_idx = find_closest_word_on_line(text_map, target_line, current_x)?;
        Some(NavResult {
            page_index: current_page,
            line_index: target_line,
            word_index: word_idx,
        })
    } else if current_page > 0 {
        // Last line of previous page
        let prev_map = cache.get_or_build(current_page - 1, document)?;
        if prev_map.line_count() > 0 {
            let target_line = prev_map.line_count() - 1;
            let word_idx = find_closest_word_on_line(prev_map, target_line, current_x)?;
            Some(NavResult {
                page_index: current_page - 1,
                line_index: target_line,
                word_index: word_idx,
            })
        } else {
            None
        }
    } else {
        None
    }
}

/// Navigate to closest word on line below
fn navigate_down(
    cache: &mut TextMapCache,
    document: &PdfDocument,
    current_page: usize,
    current_line: usize,
    current_x: f64,
    line_count: usize,
) -> Option<NavResult> {
    if current_line < line_count.saturating_sub(1) {
        // Find closest word on line below (same page)
        let target_line = current_line + 1;
        let text_map = cache.get_or_build(current_page, document)?;
        let word_idx = find_closest_word_on_line(text_map, target_line, current_x)?;
        Some(NavResult {
            page_index: current_page,
            line_index: target_line,
            word_index: word_idx,
        })
    } else if current_page < cache.page_count() - 1 {
        // First line of next page
        let next_map = cache.get_or_build(current_page + 1, document)?;
        if next_map.line_count() > 0 {
            let word_idx = find_closest_word_on_line(next_map, 0, current_x)?;
            Some(NavResult {
                page_index: current_page + 1,
                line_index: 0,
                word_index: word_idx,
            })
        } else {
            None
        }
    } else {
        None
    }
}

/// Find the word on a given line that is closest to the target x coordinate
fn find_closest_word_on_line(
    text_map: &PageTextMap,
    line_index: usize,
    target_x: f64,
) -> Option<usize> {
    let word_range = text_map.word_indices_on_line(line_index);

    if word_range.is_empty() {
        return None;
    }

    let mut closest_idx = word_range.start;
    let mut closest_dist = f64::MAX;

    for word_idx in word_range {
        if let Some(word) = text_map.get_word(word_idx) {
            let dist = (word.center_x - target_x).abs();
            if dist < closest_dist {
                closest_dist = dist;
                closest_idx = word_idx;
            }
        }
    }

    Some(closest_idx)
}

/// Find a word on the same line that starts with the given character (case-insensitive)
/// Searches forward or backward from current word position
/// Returns None if no matching word found on the same line
pub fn find_word_on_line_starting_with(
    cache: &mut TextMapCache,
    document: &PdfDocument,
    page_index: usize,
    current_word: usize,
    target_char: char,
    forward: bool,
) -> Option<NavResult> {
    let text_map = cache.get_or_build(page_index, document)?;
    let current_word_info = text_map.get_word(current_word)?;
    let line_index = current_word_info.line_index;

    let word_range = text_map.word_indices_on_line(line_index);
    if word_range.is_empty() {
        return None;
    }

    let target_lower = target_char.to_lowercase().next()?;

    if forward {
        // Search forward from current_word + 1 to end of line
        for word_idx in (current_word + 1)..word_range.end {
            if let Some(word) = text_map.get_word(word_idx) {
                if let Some(first_char) = word.text.chars().next() {
                    if first_char.to_lowercase().next() == Some(target_lower) {
                        return Some(NavResult {
                            page_index,
                            line_index,
                            word_index: word_idx,
                        });
                    }
                }
            }
        }
    } else {
        // Search backward from current_word - 1 to start of line
        for word_idx in (word_range.start..current_word).rev() {
            if let Some(word) = text_map.get_word(word_idx) {
                if let Some(first_char) = word.text.chars().next() {
                    if first_char.to_lowercase().next() == Some(target_lower) {
                        return Some(NavResult {
                            page_index,
                            line_index,
                            word_index: word_idx,
                        });
                    }
                }
            }
        }
    }

    None
}
