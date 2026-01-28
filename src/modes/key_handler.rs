use gtk::gdk::{self, ModifierType};

use crate::modes::app_mode::{AppMode, WordCursor};
use crate::text_map::{NavDirection, TextMapCache, navigate};

use pdfium_render::prelude::PdfDocument;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ScrollDir {
    Up,
    Down,
}

/// Result of handling a key press
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum KeyAction {
    /// No action needed
    Empty,
    OpenFile,
    ToggleHeaderBar,
    ScrollHalfPage(ScrollDir),

    //TOC key events
    ToggleTOC,
    ScrollTOC(ScrollDir),
    SelectChapter,

    ScrollViewport {
        x_percent: f64,
        y_percent: f64,
    },
    /// Enter visual mode (need to compute first visible word)
    EnterVisual,
    /// Exit to normal mode
    ExitVisual,
    /// Cursor moved to new position
    CursorMoved {
        cursor: WordCursor,
    },
    /// Toggle selection
    ToggleSelection,
    /// Clear selection (Esc with active selection)
    ClearSelection,
    /// Show definition for current word (or close if already open)
    ShowDefinition {
        cursor: WordCursor,
    },
    /// Translate current word or selection (or close panel if already open)
    Translate {
        start: WordCursor,
        end: WordCursor,
    },
    CopyToClipboard {
        start: WordCursor,
        end: WordCursor,
    },
    /// Scroll to start of document (gg in vim)
    ScrollToStart,
    /// Scroll to end of document (G in vim)
    ScrollToEnd,
    /// First 'g' pressed, waiting for second 'g'
    PendingG,
    PendingForward,
    PendingBackward,
    FindForward {
        letter: char,
    },
    FindBackward {
        letter: char,
    },
    /// Zoom in (+)
    ZoomIn,
    /// Zoom out (-)
    ZoomOut,
}

pub fn handle_pre_global_key(
    keyval: gdk::Key,
    modifiers: ModifierType,
    is_toc_visible: bool,
) -> KeyAction {
    if modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
        return match keyval {
            gtk::gdk::Key::d => KeyAction::ScrollHalfPage(ScrollDir::Down),
            gtk::gdk::Key::u => KeyAction::ScrollHalfPage(ScrollDir::Up),
            _ => KeyAction::Empty,
        };
    }

    match keyval {
        gdk::Key::Tab => KeyAction::ToggleTOC,
        gdk::Key::j | gdk::Key::Down => {
            //Don't like thiss brou
            if is_toc_visible {
                KeyAction::ScrollTOC(ScrollDir::Down)
            } else {
                KeyAction::Empty
            }
        }

        gdk::Key::k | gdk::Key::Up => {
            if is_toc_visible {
                KeyAction::ScrollTOC(ScrollDir::Up)
            } else {
                KeyAction::Empty
            }
        }
        gdk::Key::Return => KeyAction::SelectChapter,

        // gdk::Key::Escape => {
        //     imp.toc_panel.set_visible(false);
        //     return glib::Propagation::Stop;
        // }
        _ => KeyAction::Empty,
    }
}

pub fn handle_post_global_key(keyval: gdk::Key) -> KeyAction {
    match keyval {
        gdk::Key::o => KeyAction::OpenFile,
        gdk::Key::b => KeyAction::ToggleHeaderBar,
        _ => KeyAction::Empty,
    }
}

/// Handle a key press in Normal mode
pub fn handle_normal_mode_key(keyval: gdk::Key, key_action: KeyAction) -> Option<KeyAction> {
    // If 'g' was previously pressed, check for 'gg' sequence
    println!("DEBUG: input key_action = {:?}", key_action);
    println!("DEBUG: input key_action = {:?}", KeyAction::PendingG);
    if matches!(key_action, KeyAction::PendingG) {
        return match keyval {
            gdk::Key::g => KeyAction::ScrollToStart,
            _ => KeyAction::Empty, // Any other key cancels the pending 'g'
        };
    }

    match keyval {
        // Navigation - scroll viewport by 10%
        gdk::Key::h | gdk::Key::Left => KeyAction::ScrollViewport {
            x_percent: -10.0,
            y_percent: 0.0,
        },
        gdk::Key::l | gdk::Key::Right => KeyAction::ScrollViewport {
            x_percent: 10.0,
            y_percent: 0.0,
        },
        gdk::Key::k | gdk::Key::Up => KeyAction::ScrollViewport {
            x_percent: 0.0,
            y_percent: -10.0,
        },
        gdk::Key::j | gdk::Key::Down => KeyAction::ScrollViewport {
            x_percent: 0.0,
            y_percent: 10.0,
        },
        // Enter visual mode
        gdk::Key::v => KeyAction::EnterVisual,
        // First 'g' pressed - wait for second 'g'
        gdk::Key::g => KeyAction::PendingG,
        // 'G' (shift+g) - go to end of document
        gdk::Key::G => KeyAction::ScrollToEnd,
        // Zoom in
        gdk::Key::plus | gdk::Key::equal => KeyAction::ZoomIn,
        // Zoom out
        gdk::Key::minus => KeyAction::ZoomOut,
        _ => None,
    }
}

/// Handle a key press in Visual mode
pub fn handle_visual_mode_key(
    keyval: gdk::Key,
    mode: &AppMode,
    cache: &mut TextMapCache,
    document: &PdfDocument,
    key_action: KeyAction,
) -> KeyAction {
    let (cursor, has_selection) = match mode {
        AppMode::Visual {
            cursor,
            selection_anchor,
        } => (*cursor, selection_anchor.is_some()),
        AppMode::Normal => return KeyAction::Empty,
    };

    // If 'g' was previously pressed, check for 'gg' sequence
    if matches!(key_action, KeyAction::PendingG) {
        return match keyval {
            gdk::Key::g => KeyAction::ScrollToStart,
            _ => KeyAction::Empty, // Any other key cancels the pending 'g'
        };
    }

    if matches!(key_action, KeyAction::PendingForward) {
        if let Some(letter) = keyval.to_unicode() {
            return KeyAction::FindForward { letter: letter };
        }
        return KeyAction::Empty;
    }

    if matches!(key_action, KeyAction::PendingBackward) {
        if let Some(letter) = keyval.to_unicode() {
            return KeyAction::FindBackward { letter: letter };
        }
        return KeyAction::Empty;
    }

    match keyval {
        // Navigation - move cursor
        gdk::Key::o => KeyAction::OpenFile,
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
                KeyAction::Empty
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
                KeyAction::Empty
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
                KeyAction::Empty
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
                KeyAction::Empty
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
                KeyAction::Empty
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

        gdk::Key::f => KeyAction::PendingForward,

        gdk::Key::F => KeyAction::PendingBackward,

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

        // First 'g' pressed - wait for second 'g'
        gdk::Key::g => KeyAction::PendingG,

        // 'G' (shift+g) - go to end of document
        gdk::Key::G => KeyAction::ScrollToEnd,

        // Zoom in
        gdk::Key::plus | gdk::Key::equal => KeyAction::ZoomIn,

        // Zoom out
        gdk::Key::minus => KeyAction::ZoomOut,

        _ => None,
    }
}
