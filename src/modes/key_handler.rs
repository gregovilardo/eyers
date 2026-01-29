use gtk::gdk::{self, ModifierType};

use crate::modes::app_mode::{AppMode, WordCursor};
use crate::text_map::{NavDirection, TextMapCache, navigate};

use pdfium_render::prelude::PdfDocument;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ScrollDir {
    Up,
    Down,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum KeyAction {
    Empty,
    OpenFile,
    ToggleHeaderBar,
    ScrollHalfPage(ScrollDir),

    //TOC key events
    ToggleTOC,
    ScrollTOC(ScrollDir),
    SelectChapter,

    ScrollViewport { x_percent: f64, y_percent: f64 },
    EnterVisual,
    ExitVisual,
    CursorMoved { cursor: WordCursor },
    ToggleSelection,
    ClearSelection,
    ShowDefinition { cursor: WordCursor },
    Translate { start: WordCursor, end: WordCursor },
    CopyToClipboard { start: WordCursor, end: WordCursor },
    ScrollToStart,
    ScrollToEnd,
    PendingG,
    PendingForward,
    PendingBackward,
    // PendingNumber { number: u32 },
    FindForward { letter: char },
    FindBackward { letter: char },
    ZoomIn,
    ZoomOut,
}

pub fn handle_pre_global_key(
    keyval: gdk::Key,
    modifiers: ModifierType,
    is_toc_visible: bool,
    key_action: KeyAction,
) -> Option<KeyAction> {
    if modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
        return match keyval {
            gtk::gdk::Key::d => Some(KeyAction::ScrollHalfPage(ScrollDir::Down)),
            gtk::gdk::Key::u => Some(KeyAction::ScrollHalfPage(ScrollDir::Up)),
            _ => None,
        };
    }

    match key_action {
        KeyAction::PendingForward => return None,
        KeyAction::PendingBackward => return None,
        _ => {}
    }

    if matches!(key_action, KeyAction::PendingG) {
        return match keyval {
            gdk::Key::g => Some(KeyAction::ScrollToStart),
            _ => Some(KeyAction::Empty), // Any other key cancels the pending 'g'
        };
    }

    match keyval {
        gdk::Key::Tab => Some(KeyAction::ToggleTOC),
        gdk::Key::j | gdk::Key::Down => {
            //Don't like thiss brou
            if is_toc_visible {
                Some(KeyAction::ScrollTOC(ScrollDir::Down))
            } else {
                None
            }
        }

        gdk::Key::k | gdk::Key::Up => {
            if is_toc_visible {
                Some(KeyAction::ScrollTOC(ScrollDir::Up))
            } else {
                None
            }
        }
        gdk::Key::Return => Some(KeyAction::SelectChapter),
        gdk::Key::g => Some(KeyAction::PendingG),
        gdk::Key::G => Some(KeyAction::ScrollToEnd),

        // gdk::Key::Escape => {
        //     imp.toc_panel.set_visible(false);
        //     return glib::Propagation::Stop;
        // }
        _ => None,
    }
}

pub fn handle_post_global_key(keyval: gdk::Key) -> Option<KeyAction> {
    match keyval {
        gdk::Key::o => Some(KeyAction::OpenFile),
        gdk::Key::b => Some(KeyAction::ToggleHeaderBar),
        _ => None,
    }
}

pub fn handle_normal_mode_key(keyval: gdk::Key, key_action: KeyAction) -> Option<KeyAction> {
    match keyval {
        gdk::Key::h | gdk::Key::Left => Some(KeyAction::ScrollViewport {
            x_percent: -10.0,
            y_percent: 0.0,
        }),
        gdk::Key::l | gdk::Key::Right => Some(KeyAction::ScrollViewport {
            x_percent: 10.0,
            y_percent: 0.0,
        }),
        gdk::Key::k | gdk::Key::Up => Some(KeyAction::ScrollViewport {
            x_percent: 0.0,
            y_percent: -10.0,
        }),
        gdk::Key::j | gdk::Key::Down => Some(KeyAction::ScrollViewport {
            x_percent: 0.0,
            y_percent: 10.0,
        }),
        gdk::Key::v => Some(KeyAction::EnterVisual),
        gdk::Key::plus | gdk::Key::equal => Some(KeyAction::ZoomIn),
        gdk::Key::minus => Some(KeyAction::ZoomOut),
        _ => None,
    }
}

pub fn handle_visual_mode_key(
    keyval: gdk::Key,
    mode: &AppMode,
    cache: &mut TextMapCache,
    document: &PdfDocument,
    key_action: KeyAction,
) -> Option<KeyAction> {
    let (cursor, has_selection) = match mode {
        AppMode::Visual {
            cursor,
            selection_anchor,
        } => (*cursor, selection_anchor.is_some()),
        AppMode::Normal => return None,
    };

    // !TODO: Aca si apretas TAB para mayuscula devuelve empty
    // es caseinsensitive por lo que no haria falta pero puede ser molesto
    if matches!(key_action, KeyAction::PendingForward) {
        if let Some(letter) = keyval.to_unicode() {
            return Some(KeyAction::FindForward { letter: letter });
        }
        return Some(KeyAction::Empty);
    }

    if matches!(key_action, KeyAction::PendingBackward) {
        if let Some(letter) = keyval.to_unicode() {
            return Some(KeyAction::FindBackward { letter: letter });
        }
        return Some(KeyAction::Empty);
    }

    match keyval {
        gdk::Key::h | gdk::Key::Left => {
            if let Some(result) = navigate(
                cache,
                document,
                cursor.page_index,
                cursor.word_index,
                NavDirection::Left,
            ) {
                Some(KeyAction::CursorMoved {
                    cursor: WordCursor::new(result.page_index, result.word_index),
                })
            } else {
                Some(KeyAction::Empty)
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
                Some(KeyAction::CursorMoved {
                    cursor: WordCursor::new(result.page_index, result.word_index),
                })
            } else {
                Some(KeyAction::Empty)
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
                Some(KeyAction::CursorMoved {
                    cursor: WordCursor::new(result.page_index, result.word_index),
                })
            } else {
                Some(KeyAction::Empty)
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
                Some(KeyAction::CursorMoved {
                    cursor: WordCursor::new(result.page_index, result.word_index),
                })
            } else {
                Some(KeyAction::Empty)
            }
        }

        gdk::Key::v => Some(KeyAction::ExitVisual),

        gdk::Key::Escape => {
            if has_selection {
                Some(KeyAction::ClearSelection)
            } else {
                Some(KeyAction::ExitVisual)
            }
        }

        gdk::Key::s => Some(KeyAction::ToggleSelection),

        gdk::Key::d => {
            if !has_selection {
                Some(KeyAction::ShowDefinition { cursor })
            } else {
                Some(KeyAction::Empty)
            }
        }

        gdk::Key::t => {
            if let Some((start, end)) = mode.selection_range() {
                Some(KeyAction::Translate { start, end })
            } else {
                // Translate just the cursor word
                Some(KeyAction::Translate {
                    start: cursor,
                    end: cursor,
                })
            }
        }

        gdk::Key::f => Some(KeyAction::PendingForward),
        gdk::Key::F => Some(KeyAction::PendingBackward),

        gdk::Key::c => {
            if let Some((start, end)) = mode.selection_range() {
                Some(KeyAction::CopyToClipboard { start, end })
            } else {
                // Copy just the cursor word
                Some(KeyAction::CopyToClipboard {
                    start: cursor,
                    end: cursor,
                })
            }
        }

        gdk::Key::plus | gdk::Key::equal => Some(KeyAction::ZoomIn),
        gdk::Key::minus => Some(KeyAction::ZoomOut),

        _ => None,
    }
}
