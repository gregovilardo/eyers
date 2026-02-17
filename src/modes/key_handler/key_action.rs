use crate::modes::app_mode::WordCursor;
use crate::services::annotations::AnnotationId;

/// Direction for scrolling operations
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ScrollDir {
    Up,
    Down,
}

/// Represents a pure action to be executed.
/// Unlike the old KeyAction, this enum contains NO pending states -
/// those are now handled by InputState in the KeyHandler.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyAction {
    /// No action (used to signal the key was handled but nothing to do)
    None,

    // === File Operations ===
    OpenFile,
    OpenSettings,
    ExportAnnotations,

    // === UI Toggle ===
    ToggleHeaderBar,
    ToggleTOC,

    // === Scrolling ===
    ScrollHalfPage(ScrollDir),
    ScrollViewport {
        x_percent: f64,
        y_percent: f64,
    },
    ScrollToPage {
        page: u32,
    },
    ScrollToStart,
    ScrollToEnd,

    // === TOC Navigation ===
    ScrollTOC(ScrollDir),
    SelectChapter,

    // === Mode Changes ===
    EnterVisual,
    ExitVisual,

    // === Visual Mode Operations ===
    CursorMoved {
        cursor: WordCursor,
    },
    ToggleSelection,
    ClearSelection,
    ShowDefinition {
        cursor: WordCursor,
    },
    #[allow(dead_code)]
    Translate {
        start: WordCursor,
        end: WordCursor,
    },
    CopyToClipboard {
        start: WordCursor,
        end: WordCursor,
    },
    Annotate {
        cursor: WordCursor,
        selection: Option<(WordCursor, WordCursor)>,
    },

    // === Find Operations ===
    FindForward {
        letter: char,
    },
    FindBackward {
        letter: char,
    },

    SearchAnnotationForward,
    SearchAnnotationBackward,

    // === Zoom ===
    ZoomIn,
    ZoomOut,
}
