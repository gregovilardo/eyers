use gtk::gdk::{self, ModifierType};
use pdfium_render::prelude::PdfDocument;

use crate::modes::app_mode::{AppMode, WordCursor};
use crate::text_map::{NavDirection, TextMapCache, navigate};

use super::handler::KeyHandler;
use super::input_state::InputState;
use super::key_action::{KeyAction, ScrollDir};

/// Helper to get a digit from a key press
fn get_number_from_key(keyval: gdk::Key) -> Option<u32> {
    match keyval {
        gdk::Key::_0 | gdk::Key::KP_0 => Some(0),
        gdk::Key::_1 | gdk::Key::KP_1 => Some(1),
        gdk::Key::_2 | gdk::Key::KP_2 => Some(2),
        gdk::Key::_3 | gdk::Key::KP_3 => Some(3),
        gdk::Key::_4 | gdk::Key::KP_4 => Some(4),
        gdk::Key::_5 | gdk::Key::KP_5 => Some(5),
        gdk::Key::_6 | gdk::Key::KP_6 => Some(6),
        gdk::Key::_7 | gdk::Key::KP_7 => Some(7),
        gdk::Key::_8 | gdk::Key::KP_8 => Some(8),
        gdk::Key::_9 | gdk::Key::KP_9 => Some(9),
        _ => None,
    }
}

/// Result of key processing
pub enum KeyResult {
    /// Key was handled, execute this action
    Action(KeyAction),
    /// Key was handled, state changed but no action to execute
    StateChanged,
    /// Key was not handled by this processor
    Unhandled,
}

impl KeyResult {
    #[allow(dead_code)]
    pub fn is_handled(&self) -> bool {
        !matches!(self, KeyResult::Unhandled)
    }
}

pub fn handle_toc_key(
    handler: &KeyHandler,
    keyval: gdk::Key,
    modifiers: ModifierType,
) -> KeyResult {
    // Handle number accumulation
    if let Some(digit) = get_number_from_key(keyval) {
        handler.accumulate_digit(digit);
        return KeyResult::StateChanged;
    }

    println!("olis");
    println!("{:#?}", keyval);
    match keyval {
        gdk::Key::Escape => {
            handler.reset();
            KeyResult::Action(KeyAction::None)
        }
        gdk::Key::j | gdk::Key::Down => KeyResult::Action(KeyAction::ScrollTOC(ScrollDir::Down)),
        gdk::Key::k | gdk::Key::Up => KeyResult::Action(KeyAction::ScrollTOC(ScrollDir::Up)),
        gdk::Key::Tab => KeyResult::Action(KeyAction::ToggleTOC),
        gdk::Key::Return => KeyResult::Action(KeyAction::SelectTocRow),
        _ => KeyResult::Unhandled,
    }
}

/// Process global keys that should be handled first (before mode-specific)
pub fn handle_pre_global_key(
    handler: &KeyHandler,
    keyval: gdk::Key,
    modifiers: ModifierType,
) -> KeyResult {
    // Handle Ctrl+key combinations
    if modifiers.contains(ModifierType::CONTROL_MASK) {
        return match keyval {
            gdk::Key::d => KeyResult::Action(KeyAction::ScrollHalfPage(ScrollDir::Down)),
            gdk::Key::u => KeyResult::Action(KeyAction::ScrollHalfPage(ScrollDir::Up)),
            _ => KeyResult::Unhandled,
        };
    }

    // Handle pending states that need a character
    let input_state = handler.input_state();
    match input_state {
        InputState::PendingFForward
        | InputState::PendingFBackward
        | InputState::PendingElementForward
        | InputState::PendingElementBackward => {
            // These are handled in visual mode key handler
            return KeyResult::Unhandled;
        }
        _ => {}
    }

    // Handle PendingG state
    if matches!(input_state, InputState::PendingG) {
        return match keyval {
            gdk::Key::g => {
                // gg or [count]gg - go to start or page
                let count = handler.pending_count();
                handler.reset();
                match count {
                    Some(page) => KeyResult::Action(KeyAction::ScrollToPage { page }),
                    None => KeyResult::Action(KeyAction::ScrollToStart),
                }
            }
            _ => {
                // Any other key cancels the pending g
                handler.reset();
                KeyResult::Action(KeyAction::None)
            }
        };
    }

    // Handle number accumulation
    if let Some(digit) = get_number_from_key(keyval) {
        handler.accumulate_digit(digit);
        return KeyResult::StateChanged;
    }

    // Regular key handling
    match keyval {
        gdk::Key::Escape => {
            handler.reset();
            KeyResult::Action(KeyAction::None)
        }
        gdk::Key::Tab => KeyResult::Action(KeyAction::ToggleTOC),
        gdk::Key::g => {
            handler.set_input_state(InputState::PendingG);
            KeyResult::StateChanged
        }
        gdk::Key::G => {
            // G or [count]G - go to end or page
            let count = handler.pending_count();
            handler.reset();
            match count {
                Some(page) => KeyResult::Action(KeyAction::ScrollToPage { page }),
                None => KeyResult::Action(KeyAction::ScrollToEnd),
            }
        }
        //TODO: PendingNext para usar con annotations no sigue mucho la filosofia de vim ...
        //USAR [a  ]a jeje
        // gdk::Key::n => {
        //     handler.set_input_state(InputState::PendingNext);
        //     KeyResult::StateChanged
        // }
        _ => KeyResult::Unhandled,
    }
}

/// Process global keys that should be handled last (after mode-specific)
pub fn handle_post_global_key(handler: &KeyHandler, keyval: gdk::Key) -> KeyResult {
    let result = match keyval {
        gdk::Key::o => KeyResult::Action(KeyAction::OpenFile),
        gdk::Key::b => KeyResult::Action(KeyAction::ToggleHeaderBar),
        gdk::Key::p => KeyResult::Action(KeyAction::OpenSettings),
        gdk::Key::e => KeyResult::Action(KeyAction::ExportAnnotations),
        _ => KeyResult::Unhandled,
    };

    // Reset state after post-global actions (except if unhandled)
    if let KeyResult::Action(_) = &result {
        handler.reset();
    }

    result
}

/// Process keys in Normal mode
pub fn handle_normal_mode_key(handler: &KeyHandler, keyval: gdk::Key) -> KeyResult {
    let result = match keyval {
        gdk::Key::h | gdk::Key::Left => KeyResult::Action(KeyAction::ScrollViewport {
            x_percent: -10.0,
            y_percent: 0.0,
        }),
        gdk::Key::l | gdk::Key::Right => KeyResult::Action(KeyAction::ScrollViewport {
            x_percent: 10.0,
            y_percent: 0.0,
        }),
        gdk::Key::k | gdk::Key::Up => KeyResult::Action(KeyAction::ScrollViewport {
            x_percent: 0.0,
            y_percent: -10.0,
        }),
        gdk::Key::j | gdk::Key::Down => KeyResult::Action(KeyAction::ScrollViewport {
            x_percent: 0.0,
            y_percent: 10.0,
        }),
        gdk::Key::v => KeyResult::Action(KeyAction::EnterVisual),
        gdk::Key::plus | gdk::Key::equal => KeyResult::Action(KeyAction::ZoomIn),
        gdk::Key::minus => KeyResult::Action(KeyAction::ZoomOut),
        _ => KeyResult::Unhandled,
    };

    // Reset state after normal mode actions
    if let KeyResult::Action(_) = &result {
        handler.reset();
    }

    result
}

/// Process keys in Visual mode
pub fn handle_visual_mode_key(
    handler: &KeyHandler,
    keyval: gdk::Key,
    mode: &AppMode,
    cache: &mut TextMapCache,
    document: &PdfDocument,
) -> KeyResult {
    let (cursor, has_selection) = match mode {
        AppMode::Visual {
            cursor,
            selection_anchor,
        } => (*cursor, selection_anchor.is_some()),
        AppMode::Normal => return KeyResult::Unhandled,
    };

    let input_state = handler.input_state();

    // Handle pending find operations
    if matches!(input_state, InputState::PendingFForward) {
        if let Some(letter) = keyval.to_unicode() {
            return KeyResult::Action(KeyAction::FindForward { letter });
        }
        handler.reset();
        return KeyResult::Action(KeyAction::None);
    }

    if matches!(input_state, InputState::PendingFBackward) {
        if let Some(letter) = keyval.to_unicode() {
            return KeyResult::Action(KeyAction::FindBackward { letter });
        }
        handler.reset();
        return KeyResult::Action(KeyAction::None);
    }

    if matches!(input_state, InputState::PendingElementForward) {
        return match keyval {
            gdk::Key::a => KeyResult::Action(KeyAction::SearchAnnotationForward),
            _ => {
                handler.reset();
                KeyResult::Action(KeyAction::None)
            }
        };
    }

    if matches!(input_state, InputState::PendingElementBackward) {
        return match keyval {
            gdk::Key::a => KeyResult::Action(KeyAction::SearchAnnotationBackward),
            _ => {
                handler.reset();
                KeyResult::Action(KeyAction::None)
            }
        };
    }

    // Navigation keys with optional count
    let count = handler.count();

    let result = match keyval {
        gdk::Key::h | gdk::Key::Left => {
            if let Some(new_cursor) =
                navigate_with_count(cache, document, cursor, NavDirection::Left, count)
            {
                KeyResult::Action(KeyAction::CursorMoved { cursor: new_cursor })
            } else {
                KeyResult::Action(KeyAction::None)
            }
        }
        gdk::Key::l | gdk::Key::Right => {
            if let Some(new_cursor) =
                navigate_with_count(cache, document, cursor, NavDirection::Right, count)
            {
                KeyResult::Action(KeyAction::CursorMoved { cursor: new_cursor })
            } else {
                KeyResult::Action(KeyAction::None)
            }
        }
        gdk::Key::k | gdk::Key::Up => {
            if let Some(new_cursor) =
                navigate_with_count(cache, document, cursor, NavDirection::Up, count)
            {
                KeyResult::Action(KeyAction::CursorMoved { cursor: new_cursor })
            } else {
                KeyResult::Action(KeyAction::None)
            }
        }
        gdk::Key::j | gdk::Key::Down => {
            if let Some(new_cursor) =
                navigate_with_count(cache, document, cursor, NavDirection::Down, count)
            {
                KeyResult::Action(KeyAction::CursorMoved { cursor: new_cursor })
            } else {
                KeyResult::Action(KeyAction::None)
            }
        }

        gdk::Key::v => KeyResult::Action(KeyAction::ExitVisual),

        gdk::Key::Escape => {
            if has_selection {
                KeyResult::Action(KeyAction::ClearSelection)
            } else {
                KeyResult::Action(KeyAction::ExitVisual)
            }
        }

        gdk::Key::s => KeyResult::Action(KeyAction::ToggleSelection),

        gdk::Key::d => {
            if !has_selection {
                KeyResult::Action(KeyAction::ShowDefinition { cursor })
            } else {
                KeyResult::Action(KeyAction::None)
            }
        }

        gdk::Key::f => {
            handler.set_input_state(InputState::PendingFForward);
            KeyResult::StateChanged
        }
        gdk::Key::F => {
            handler.set_input_state(InputState::PendingFBackward);
            KeyResult::StateChanged
        }

        gdk::Key::bracketright => {
            handler.set_input_state(InputState::PendingElementForward);
            KeyResult::StateChanged
        }

        gdk::Key::bracketleft => {
            handler.set_input_state(InputState::PendingElementBackward);
            KeyResult::StateChanged
        }

        gdk::Key::y => {
            if let Some((start, end)) = mode.selection_range() {
                KeyResult::Action(KeyAction::CopyToClipboard { start, end })
            } else {
                KeyResult::Action(KeyAction::CopyToClipboard {
                    start: cursor,
                    end: cursor,
                })
            }
        }

        gdk::Key::a => KeyResult::Action(KeyAction::Annotate {
            cursor,
            selection: mode.selection_range(),
        }),

        gdk::Key::plus | gdk::Key::equal => KeyResult::Action(KeyAction::ZoomIn),
        gdk::Key::minus => KeyResult::Action(KeyAction::ZoomOut),

        _ => KeyResult::Unhandled,
    };

    // Reset state after visual mode actions (except when entering pending state)
    if let KeyResult::Action(_) = &result {
        handler.reset();
    }

    result
}

/// Navigate multiple times based on count
fn navigate_with_count(
    cache: &mut TextMapCache,
    document: &PdfDocument,
    start_cursor: WordCursor,
    direction: NavDirection,
    count: u32,
) -> Option<WordCursor> {
    let mut current = start_cursor;

    for _ in 0..count {
        if let Some(result) = navigate(
            cache,
            document,
            current.page_index,
            current.word_index,
            direction,
        ) {
            current = WordCursor::new(result.page_index, result.word_index);
        } else {
            // Stop if navigation fails
            break;
        }
    }

    // Only return if we actually moved
    if current != start_cursor {
        Some(current)
    } else {
        None
    }
}
