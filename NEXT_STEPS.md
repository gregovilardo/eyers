# Eyers - Next Steps

## 1. Language Selector

Currently, the translation feature is hardcoded to translate from English to Spanish. A language selector would allow users to choose their source and target languages dynamically.

**What you'll learn:**
- GTK4 `DropDown` widget for single-selection lists
- `StringList` model for simple string-based options
- Connecting selection changes to update application behavior
- Potentially fetching available languages from LibreTranslate's `/languages` endpoint
- Updating the `TranslationPanel` to expose language selection and pass it to the translation service

**Considerations:**
- Should both source and target languages be selectable, or just target?
- Where should the selectors live - in the header bar, or within the translation panel itself?
- Should the selected languages persist between sessions (ties into GSettings)?


## 2. Resizable Translation Panel

The translation panel currently has a fixed height ratio. A resizable panel would let users drag to adjust how much screen space the panel occupies.

**What you'll learn:**
- GTK4 `Paned` widget, which provides a draggable divider between two children
- Orientation handling (vertical pane for top/bottom split)
- Setting minimum sizes and initial position
- How `Paned` differs from manually managing sizes with `Box`

**Considerations:**
- `Paned` replaces the current `Box` layout in `EyersWindow` - the PDF view goes in one pane, translation panel in the other
- You'll need to handle the case where the panel is hidden (collapse the pane or use a different approach)
- Should the panel size persist between sessions?


## 3. Drag-to-Select Text

The current two-click selection is functional but not intuitive. Drag-to-select would let users click and drag to highlight text, similar to standard text selection in other applications.

**What you'll learn:**
- `GestureDrag` for tracking press, drag motion, and release events
- Real-time coordinate tracking and updating highlights during the drag
- Performance considerations when updating UI frequently during motion events
- Hit-testing against character bounding boxes during drag

**Considerations:**
- This is significantly more complex than two-click selection
- You'll need to handle edge cases: dragging backwards, dragging outside the widget bounds
- Highlight updates during drag should be efficient (avoid recreating all highlight boxes on every motion event)
- May want to keep two-click as an alternative or remove it entirely


## 4. Cross-Page Selection

Currently, text selection is limited to a single page. Cross-page selection would allow highlighting text that spans multiple pages.

**What you'll learn:**
- Rethinking data structures to track selections across page boundaries
- Coordinating highlights across multiple `Overlay` widgets
- Managing selection state that spans multiple coordinate systems (each page has its own)
- Potentially restructuring how pages and their highlights are stored

**Considerations:**
- This adds complexity to both the selection logic and the highlight rendering
- Works best when combined with drag-to-select (item 3)
- Need to handle the visual feedback when selection spans pages not currently visible
- The text extraction logic in `pdf_text.rs` may need updates to concatenate text across pages


## 5. GSettings Integration

GSettings is GNOME's standard system for storing application preferences. This would allow Eyers to remember user choices between sessions.

**What you'll learn:**
- Creating a GSettings schema XML file
- Compiling schemas with `glib-compile-schemas`
- Using `gio::Settings` in Rust to read/write preferences
- Binding settings directly to GObject properties for automatic persistence
- Proper schema installation paths for development vs. installed applications

**Considerations:**
- Requires a schema file (`.gschema.xml`) and compilation step
- What should be persisted? Window size, panel height, selected languages, last opened file?
- Schema changes require recompilation - plan the schema carefully
- For development, you'll need to set `GSETTINGS_SCHEMA_DIR` environment variable


## 6. Code Quality Improvements

Before adding new features, it may be worth refining the existing implementation.

**Potential areas:**
- **Error handling**: Currently, API failures may not be communicated clearly to the user. Adding proper error states to the UI.
- **Async/await**: Converting the blocking HTTP calls in threads to proper async using `gtk::gio::spawn_blocking` or async runtime integration.
- **Highlight optimization**: Currently creates one `Box` per character. Could merge adjacent characters into larger rectangular regions.
- **CSS management**: The inline CSS approach adds providers globally. Could be refined to use widget-specific styling.
- **Code documentation**: Adding rustdoc comments to public APIs and complex internal functions.

**What you'll learn:**
- Rust's error handling patterns in GUI contexts
- Async patterns in gtk-rs applications
- Performance profiling and optimization
- GTK4's CSS styling system in depth
