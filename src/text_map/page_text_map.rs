use pdfium_render::prelude::*;

use crate::text_map::word_info::{LineInfo, WordInfo};

/// Threshold for considering characters on the same line (as percentage of avg char height)
const LINE_GROUPING_THRESHOLD: f64 = 0.5;

/// Represents all text data for a single PDF page, organized for efficient navigation
#[derive(Debug)]
pub struct PageTextMap {
    /// Page index in the document
    pub page_index: usize,
    /// All words on the page, in reading order (top-to-bottom, left-to-right)
    pub words: Vec<WordInfo>,
    /// Lines of text, each containing a range of word indices
    pub lines: Vec<LineInfo>,
    /// Page dimensions in PDF points
    pub page_width: f64,
    pub page_height: f64,
}

impl PageTextMap {
    /// Build a PageTextMap by extracting all words from a PDF page
    pub fn build_from_page(page: &PdfPage, page_index: usize) -> Option<Self> {
        let text_page = page.text().ok()?;
        let page_width = page.width().value as f64;
        let page_height = page.height().value as f64;

        // Extract all characters with their bounds
        let chars = text_page.chars();
        let mut char_data: Vec<CharData> = Vec::new();

        for char_obj in chars.iter() {
            if let (Some(unicode), Ok(bounds)) = (char_obj.unicode_char(), char_obj.tight_bounds())
            {
                char_data.push(CharData {
                    char: unicode,
                    index: char_obj.index() as usize,
                    bounds,
                });
            }
        }

        if char_data.is_empty() {
            return Some(Self {
                page_index,
                words: Vec::new(),
                lines: Vec::new(),
                page_width,
                page_height,
            });
        }

        // Group characters into words
        let mut words = Self::extract_words(&char_data);

        if words.is_empty() {
            return Some(Self {
                page_index,
                words: Vec::new(),
                lines: Vec::new(),
                page_width,
                page_height,
            });
        }

        // Group words into lines and assign line indices
        let lines = Self::group_into_lines(&mut words);

        Some(Self {
            page_index,
            words,
            lines,
            page_width,
            page_height,
        })
    }

    /// Extract words from character data
    fn extract_words(char_data: &[CharData]) -> Vec<WordInfo> {
        let mut words: Vec<WordInfo> = Vec::new();
        let mut current_word_chars: Vec<&CharData> = Vec::new();

        for char_info in char_data {
            if char_info.char.is_whitespace() {
                // End current word if any
                if !current_word_chars.is_empty() {
                    if let Some(word) = Self::build_word_from_chars(&current_word_chars) {
                        words.push(word);
                    }
                    current_word_chars.clear();
                }
            } else if Self::is_word_char(char_info.char) {
                current_word_chars.push(char_info);
            } else {
                // Non-word character (punctuation, etc.) - end current word
                if !current_word_chars.is_empty() {
                    if let Some(word) = Self::build_word_from_chars(&current_word_chars) {
                        words.push(word);
                    }
                    current_word_chars.clear();
                }
                // Optionally include standalone punctuation as "words" if needed
            }
        }

        // Don't forget the last word
        if !current_word_chars.is_empty() {
            if let Some(word) = Self::build_word_from_chars(&current_word_chars) {
                words.push(word);
            }
        }

        words
    }

    /// Build a WordInfo from a sequence of characters
    fn build_word_from_chars(chars: &[&CharData]) -> Option<WordInfo> {
        if chars.is_empty() {
            return None;
        }

        let text: String = chars.iter().map(|c| c.char).collect();
        let char_start = chars.first()?.index;
        let char_end = chars.last()?.index + 1;

        // Compute bounding box as union of all character bounds
        let mut min_left = f32::MAX;
        let mut max_right = f32::MIN;
        let mut min_bottom = f32::MAX;
        let mut max_top = f32::MIN;

        for c in chars {
            min_left = min_left.min(c.bounds.left().value);
            max_right = max_right.max(c.bounds.right().value);
            min_bottom = min_bottom.min(c.bounds.bottom().value);
            max_top = max_top.max(c.bounds.top().value);
        }

        let bounds = PdfRect::new_from_values(min_bottom, min_left, max_top, max_right);

        // line_index will be set later during line grouping
        Some(WordInfo::new(text, char_start, char_end, bounds, 0))
    }

    /// Group words into lines based on y-coordinate proximity and reorder into reading order.
    ///
    /// Strategy:
    /// 1. Sort words by y-center descending (top-to-bottom)
    /// 2. Cluster into lines using threshold - assign each word a line_index
    /// 3. Sort by (line_index, center_x) - this is a proper total order (transitive)
    /// 4. Reorder words array and build LineInfo with correct word ranges
    fn group_into_lines(words: &mut [WordInfo]) -> Vec<LineInfo> {
        if words.is_empty() {
            return Vec::new();
        }

        // Calculate average character height for threshold
        let avg_height: f64 = words
            .iter()
            .map(|w| (w.bounds.top().value - w.bounds.bottom().value) as f64)
            .sum::<f64>()
            / words.len() as f64;

        let threshold = avg_height * LINE_GROUPING_THRESHOLD;

        // Step 1: Sort word indices by y-center descending (top first in PDF coords)
        let mut word_indices: Vec<usize> = (0..words.len()).collect();
        word_indices.sort_by(|&a, &b| words[b].center_y.total_cmp(&words[a].center_y));

        // Step 2: Cluster into lines and assign line_index to each word
        // Also track line y-centers for LineInfo
        let mut line_y_centers: Vec<f64> = Vec::new();
        let mut current_line_y: Option<f64> = None;
        let mut current_line_idx: usize = 0;

        for &word_idx in &word_indices {
            let word_y = words[word_idx].center_y;

            match current_line_y {
                Some(line_y) if (word_y - line_y).abs() <= threshold => {
                    // Same line - assign current line index
                    words[word_idx].line_index = current_line_idx;
                }
                _ => {
                    // New line
                    if current_line_y.is_some() {
                        current_line_idx += 1;
                    }
                    line_y_centers.push(word_y);
                    current_line_y = Some(word_y);
                    words[word_idx].line_index = current_line_idx;
                }
            }
        }

        // Step 3: Sort by (line_index, center_x) - proper total order, no transitivity issues
        word_indices.sort_by(|&a, &b| {
            let line_cmp = words[a].line_index.cmp(&words[b].line_index);
            if line_cmp != std::cmp::Ordering::Equal {
                line_cmp
            } else {
                words[a].center_x.total_cmp(&words[b].center_x)
            }
        });

        // Step 4: Reorder words array according to sorted indices
        let reordered: Vec<WordInfo> = word_indices
            .iter()
            .map(|&old_idx| words[old_idx].clone())
            .collect();
        words.clone_from_slice(&reordered);

        // Step 5: Build LineInfo with correct word ranges
        let mut lines: Vec<LineInfo> = Vec::new();
        let mut current_line_start: usize = 0;
        let mut prev_line_index: Option<usize> = None;

        for (new_idx, word) in words.iter().enumerate() {
            match prev_line_index {
                Some(prev_idx) if word.line_index == prev_idx => {
                    // Same line, continue
                }
                _ => {
                    // New line - finalize previous
                    if let Some(prev_idx) = prev_line_index {
                        let line_y = line_y_centers.get(prev_idx).copied().unwrap_or(0.0);
                        lines.push(LineInfo::new(current_line_start, new_idx, line_y));
                    }
                    current_line_start = new_idx;
                    prev_line_index = Some(word.line_index);
                }
            }
        }

        // Finalize last line
        if let Some(prev_idx) = prev_line_index {
            let line_y = line_y_centers.get(prev_idx).copied().unwrap_or(0.0);
            lines.push(LineInfo::new(current_line_start, words.len(), line_y));
        }

        lines
    }

    /// Check if a character should be part of a word
    fn is_word_char(c: char) -> bool {
        c.is_alphanumeric() || c == '\'' || c == '-'
    }

    /// Get the word at a specific index
    pub fn get_word(&self, index: usize) -> Option<&WordInfo> {
        self.words.get(index)
    }

    /// Get the line at a specific index
    pub fn get_line(&self, index: usize) -> Option<&LineInfo> {
        self.lines.get(index)
    }

    /// Get all words on a specific line
    pub fn words_on_line(&self, line_index: usize) -> &[WordInfo] {
        if let Some(line) = self.lines.get(line_index) {
            &self.words[line.word_start..line.word_end]
        } else {
            &[]
        }
    }

    /// Get word indices for a specific line
    pub fn word_indices_on_line(&self, line_index: usize) -> std::ops::Range<usize> {
        if let Some(line) = self.lines.get(line_index) {
            line.word_start..line.word_end
        } else {
            0..0
        }
    }

    /// Find the first word whose bounds intersect with the given rect
    /// Used for finding first visible word in viewport
    pub fn first_word_in_rect(&self, rect_top: f64, rect_bottom: f64) -> Option<usize> {
        // In PDF coords, top > bottom
        // We want the first word (in reading order) that overlaps with the viewport
        for (idx, word) in self.words.iter().enumerate() {
            let word_top = word.bounds.top().value as f64;
            let word_bottom = word.bounds.bottom().value as f64;

            // Check if word overlaps with rect vertically
            if word_top >= rect_bottom && word_bottom <= rect_top {
                return Some(idx);
            }
        }
        None
    }

    /// Total number of words on this page
    pub fn word_count(&self) -> usize {
        self.words.len()
    }

    /// Total number of lines on this page
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

/// Internal struct for character extraction
struct CharData {
    char: char,
    index: usize,
    bounds: PdfRect,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_word_char() {
        assert!(PageTextMap::is_word_char('a'));
        assert!(PageTextMap::is_word_char('Z'));
        assert!(PageTextMap::is_word_char('5'));
        assert!(PageTextMap::is_word_char('\''));
        assert!(PageTextMap::is_word_char('-'));
        assert!(!PageTextMap::is_word_char(' '));
        assert!(!PageTextMap::is_word_char('.'));
        assert!(!PageTextMap::is_word_char(','));
    }
}
