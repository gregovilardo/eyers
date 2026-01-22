/// Represents a position in the document (page + word)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WordCursor {
    pub page_index: usize,
    pub word_index: usize,
}

impl WordCursor {
    pub fn new(page_index: usize, word_index: usize) -> Self {
        Self {
            page_index,
            word_index,
        }
    }
}

/// The current mode of the application
#[derive(Debug, Clone)]
pub enum AppMode {
    /// Normal mode - scrolling only, no cursor
    Normal,
    /// Visual mode - word navigation with cursor
    Visual {
        cursor: WordCursor,
        /// Selection anchor (set when 's' is pressed)
        selection_anchor: Option<WordCursor>,
    },
}

impl Default for AppMode {
    fn default() -> Self {
        Self::Normal
    }
}

impl AppMode {
    /// Check if currently in Normal mode
    pub fn is_normal(&self) -> bool {
        matches!(self, AppMode::Normal)
    }

    /// Check if currently in Visual mode
    pub fn is_visual(&self) -> bool {
        matches!(self, AppMode::Visual { .. })
    }

    /// Get the cursor if in Visual mode
    pub fn cursor(&self) -> Option<WordCursor> {
        match self {
            AppMode::Visual { cursor, .. } => Some(*cursor),
            AppMode::Normal => None,
        }
    }

    /// Get the selection anchor if in Visual mode with active selection
    pub fn selection_anchor(&self) -> Option<WordCursor> {
        match self {
            AppMode::Visual {
                selection_anchor, ..
            } => *selection_anchor,
            AppMode::Normal => None,
        }
    }

    /// Check if there's an active selection
    pub fn has_selection(&self) -> bool {
        matches!(
            self,
            AppMode::Visual {
                selection_anchor: Some(_),
                ..
            }
        )
    }

    /// Enter Visual mode with cursor at the given position
    pub fn enter_visual(cursor: WordCursor) -> Self {
        AppMode::Visual {
            cursor,
            selection_anchor: None,
        }
    }

    /// Exit to Normal mode
    pub fn exit_to_normal() -> Self {
        AppMode::Normal
    }

    /// Update cursor position (only works in Visual mode)
    pub fn set_cursor(&mut self, new_cursor: WordCursor) {
        if let AppMode::Visual { cursor, .. } = self {
            *cursor = new_cursor;
        }
    }

    /// Toggle selection anchor (set if None, clear if Some)
    pub fn toggle_selection(&mut self) {
        if let AppMode::Visual {
            cursor,
            selection_anchor,
        } = self
        {
            if selection_anchor.is_some() {
                *selection_anchor = None;
            } else {
                *selection_anchor = Some(*cursor);
            }
        }
    }

    /// Clear selection anchor only
    pub fn clear_selection(&mut self) {
        if let AppMode::Visual {
            selection_anchor, ..
        } = self
        {
            *selection_anchor = None;
        }
    }

    /// Get the selection range as (start, end) cursors in document order
    /// Returns None if no selection is active
    pub fn selection_range(&self) -> Option<(WordCursor, WordCursor)> {
        match self {
            AppMode::Visual {
                cursor,
                selection_anchor: Some(anchor),
            } => {
                // Order by page first, then by word index
                let (start, end) = if anchor.page_index < cursor.page_index
                    || (anchor.page_index == cursor.page_index
                        && anchor.word_index <= cursor.word_index)
                {
                    (*anchor, *cursor)
                } else {
                    (*cursor, *anchor)
                };
                Some((start, end))
            }
            _ => None,
        }
    }

    /// Get mode name for display
    pub fn display_name(&self) -> &'static str {
        match self {
            AppMode::Normal => "NORMAL",
            AppMode::Visual { .. } => "VISUAL",
        }
    }
}
