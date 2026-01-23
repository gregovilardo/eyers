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

    /// Group words into lines based on y-coordinate proximity
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

        // Sort words by y-center (descending, since PDF y=0 is at bottom)
        // This gives us top-to-bottom order
        let mut word_indices: Vec<usize> = (0..words.len()).collect();
        word_indices.sort_by(|&a, &b| words[b].center_y.total_cmp(&words[a].center_y));

        // Group into lines
        let mut lines: Vec<LineInfo> = Vec::new();
        let mut current_line_indices: Vec<usize> = Vec::new();
        let mut current_line_y: Option<f64> = None;

        for &word_idx in &word_indices {
            let word_y = words[word_idx].center_y;

            match current_line_y {
                Some(line_y) if (word_y - line_y).abs() <= threshold => {
                    // Same line
                    current_line_indices.push(word_idx);
                }
                _ => {
                    // New line - save previous if exists
                    if !current_line_indices.is_empty() {
                        // Sort words in line by x (left to right)
                        current_line_indices
                            .sort_by(|&a, &b| words[a].center_x.total_cmp(&words[b].center_x));

                        let line_y_avg = current_line_indices
                            .iter()
                            .map(|&i| words[i].center_y)
                            .sum::<f64>()
                            / current_line_indices.len() as f64;

                        lines.push(LineInfo::new(0, 0, line_y_avg)); // word ranges set later
                    }

                    current_line_indices = vec![word_idx];
                    current_line_y = Some(word_y);
                }
            }
        }

        // Don't forget the last line
        if !current_line_indices.is_empty() {
            let line_y_avg = current_line_indices
                .iter()
                .map(|&i| words[i].center_y)
                .sum::<f64>()
                / current_line_indices.len() as f64;

            lines.push(LineInfo::new(0, 0, line_y_avg));
        }

        // Now reorder words into reading order and assign line indices
        Self::reorder_words_by_reading_order(words, &mut lines, &word_indices, threshold)
    }

    /// Reorder words vec into reading order (top-to-bottom, left-to-right)
    /// and set line indices on each word
    fn reorder_words_by_reading_order(
        words: &mut [WordInfo],
        _lines: &mut Vec<LineInfo>,
        _word_indices: &[usize],
        threshold: f64,
    ) -> Vec<LineInfo> {
        if words.is_empty() {
            return Vec::new();
        }

        // Create a list of (original_index, word) pairs sorted by reading order
        let mut indexed_words: Vec<(usize, &WordInfo)> = words.iter().enumerate().collect();

        indexed_words.sort_by(|(_, a), (_, b)| {
            // Discretize y into line buckets so its a total order and sort don't panic
            let line_a = (a.center_y / threshold).floor() as i64;
            let line_b = (b.center_y / threshold).floor() as i64;

            line_b
                .cmp(&line_a) // descending by line
                .then_with(|| a.center_x.total_cmp(&b.center_x)) // then ascending by x
        });

        // Build reordering map: new_index -> old_index
        let reorder_map: Vec<usize> = indexed_words.iter().map(|(old_idx, _)| *old_idx).collect();

        // Create reordered words
        let mut reordered: Vec<WordInfo> = reorder_map
            .iter()
            .map(|&old_idx| words[old_idx].clone())
            .collect();

        // Now group into lines and assign line_index
        let mut final_lines: Vec<LineInfo> = Vec::new();
        let mut current_line_start: usize = 0;
        let mut current_line_y: Option<f64> = None;

        for (new_idx, word) in reordered.iter_mut().enumerate() {
            match current_line_y {
                Some(line_y) if (word.center_y - line_y).abs() <= threshold => {
                    // Same line
                    word.line_index = final_lines.len();
                }
                _ => {
                    // New line
                    if let Some(prev_y) = current_line_y {
                        final_lines.push(LineInfo::new(current_line_start, new_idx, prev_y));
                    }
                    current_line_start = new_idx;
                    current_line_y = Some(word.center_y);
                    word.line_index = final_lines.len();
                }
            }
        }

        // Last line
        if let Some(line_y) = current_line_y {
            final_lines.push(LineInfo::new(current_line_start, reordered.len(), line_y));
        }

        // Copy reordered words back
        words.clone_from_slice(&reordered);

        final_lines
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
