use gtk::gdk;

use crate::modes::app_mode::{AppMode, WordCursor};
use crate::text_map::{navigate, NavDirection, TextMapCache};

use pdfium_render::prelude::PdfDocument;

/// Result of handling a key press
#[derive(Debug)]
pub enum KeyAction {
    /// No action needed
    None,
    /// Scroll viewport by percentage (x%, y%)
    Scroll { x_percent: f64, y_percent: f64 },
    /// Enter visual mode (need to compute first visible word)
    EnterVisual,
    /// Exit to normal mode
    ExitVisual,
    /// Cursor moved to new position
    CursorMoved { cursor: WordCursor },
    /// Toggle selection
    ToggleSelection,
    /// Clear selection (Esc with active selection)
    ClearSelection,
    /// Show definition for current word (or close if already open)
    ShowDefinition { cursor: WordCursor },
    /// Translate current word or selection (or close panel if already open)
    Translate { start: WordCursor, end: WordCursor },
    /// Start find forward (f key pressed, waiting for target char)
    StartFindForward,
    /// Start find backward (F key pressed, waiting for target char)
    StartFindBackward,
    /// Copy text to clipboard (start and end cursor for range, or same cursor for single word)
    CopyToClipboard { start: WordCursor, end: WordCursor },
}

/// Handle a key press in Normal mode
pub fn handle_normal_mode_key(keyval: gdk::Key) -> KeyAction {
    match keyval {
        // Navigation - scroll viewport by 10%
        gdk::Key::h | gdk::Key::Left => KeyAction::Scroll {
            x_percent: -10.0,
            y_percent: 0.0,
        },
        gdk::Key::l | gdk::Key::Right => KeyAction::Scroll {
            x_percent: 10.0,
            y_percent: 0.0,
        },
        gdk::Key::k | gdk::Key::Up => KeyAction::Scroll {
            x_percent: 0.0,
            y_percent: -10.0,
        },
        gdk::Key::j | gdk::Key::Down => KeyAction::Scroll {
            x_percent: 0.0,
            y_percent: 10.0,
        },
        // Enter visual mode
        gdk::Key::v => KeyAction::EnterVisual,
        // Note: 'o' for OpenFile is handled directly in handle_mode_key before document check
        _ => KeyAction::None,
    }
}

/// Handle a key press in Visual mode
pub fn handle_visual_mode_key(
    keyval: gdk::Key,
    mode: &AppMode,
    cache: &mut TextMapCache,
    document: &PdfDocument,
) -> KeyAction {
    let (cursor, has_selection) = match mode {
        AppMode::Visual {
            cursor,
            selection_anchor,
        } => (*cursor, selection_anchor.is_some()),
        AppMode::Normal => return KeyAction::None,
    };

    match keyval {
        // Navigation - move cursor
        gdk::Key::h | gdk::Key::Left => {
            if let Some(result) = navigate(
                cache,
                document,
                cursor.page_index,
                cursor.word_index,
                NavDirection::Left,
            ) {
                KeyAction::CursorMoved {
                    cursor: WordCursor::new(result.page_index, result.word_index),
                }
            } else {
                KeyAction::None
            }
        }
        gdk::Key::l | gdk::Key::Right => {
            if let Some(result) = navigate(
                cache,
                document,
                cursor.page_index,
                cursor.word_index,
                NavDirection::Right,
            ) {
                KeyAction::CursorMoved {
                    cursor: WordCursor::new(result.page_index, result.word_index),
                }
            } else {
                KeyAction::None
            }
        }
        gdk::Key::k | gdk::Key::Up => {
            if let Some(result) = navigate(
                cache,
                document,
                cursor.page_index,
                cursor.word_index,
                NavDirection::Up,
            ) {
                KeyAction::CursorMoved {
                    cursor: WordCursor::new(result.page_index, result.word_index),
                }
            } else {
                KeyAction::None
            }
        }
        gdk::Key::j | gdk::Key::Down => {
            if let Some(result) = navigate(
                cache,
                document,
                cursor.page_index,
                cursor.word_index,
                NavDirection::Down,
            ) {
                KeyAction::CursorMoved {
                    cursor: WordCursor::new(result.page_index, result.word_index),
                }
            } else {
                KeyAction::None
            }
        }

        // Exit visual mode
        gdk::Key::v => KeyAction::ExitVisual,

        // Escape - clear selection first, then exit
        gdk::Key::Escape => {
            if has_selection {
                KeyAction::ClearSelection
            } else {
                KeyAction::ExitVisual
            }
        }

        // Toggle selection
        gdk::Key::s => KeyAction::ToggleSelection,

        // Show definition (only if no selection)
        gdk::Key::d => {
            if !has_selection {
                KeyAction::ShowDefinition { cursor }
            } else {
                KeyAction::None
            }
        }

        // Translate (current word or selection)
        gdk::Key::t => {
            if let Some((start, end)) = mode.selection_range() {
                KeyAction::Translate { start, end }
            } else {
                // Translate just the cursor word
                KeyAction::Translate {
                    start: cursor,
                    end: cursor,
                }
            }
        }

        // Note: 'o' for OpenFile is handled directly in handle_mode_key before document check

        // Find forward (f + char)
        gdk::Key::f => KeyAction::StartFindForward,

        // Find backward (F + char)
        gdk::Key::F => KeyAction::StartFindBackward,

        // Copy to clipboard (selection or cursor word)
        gdk::Key::c => {
            if let Some((start, end)) = mode.selection_range() {
                KeyAction::CopyToClipboard { start, end }
            } else {
                // Copy just the cursor word
                KeyAction::CopyToClipboard {
                    start: cursor,
                    end: cursor,
                }
            }
        }

        _ => KeyAction::None,
    }
}
