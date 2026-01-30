# Eyers

## A PDF reader with vim-inspired navigation and language learning features, built with Rust and GTK4.

### Features
- Vim-like modes: Normal mode for scrolling, Visual mode for word-level cursor navigation
- Text selection: Word-by-word selection with clipboard copying
- Translation: English-to-Spanish translation via LibreTranslate
- Dictionary: Word definitions in popup windows
- TOC navigation: Jump to chapters via table of contents
- Keyboard-driven: All major operations accessible via keybindings
Key Bindings
- j/k or arrows - Scroll down/up
- h/l or arrows - Scroll left/right
- v - Toggle visual mode
- s - Toggle text selection
- y - Copy selected text  
- d - Show word definition
- t - Translate selection
- gg - Jump to start
- {number}gg - Jump to {number} 
- G - Jump to end

### Tech
- Rust + GTK4
- pdfium-render for PDF viewing
- LibreTranslate API for translations
---
