# Eyers - Installation and Usage Guide

## What is Eyers?

Eyers is a PDF reader with vim-style keyboard navigation and built-in dictionary lookup for language learning. You can navigate documents using keyboard commands, look up word definitions, and save annotations that persist across sessions.
Probably i'm not following true vim-motions in any right way, please don't be mad.

## Installation

### Requirements

- Rust 1.90.0 or higher
- GTK4 development libraries

On Linux:
```bash
# Debian/Ubuntu
sudo apt install libgtk-4-dev build-essential

# Fedora
sudo dnf install gtk4-devel gcc

# Arch Linux
sudo pacman -S gtk4 base-devel
```

### Build

```bash
git clone <repository-url>
cd eyers
cargo build --release
```

The binary will be at `target/release/eyers`

## Dictionary Setup

The dictionary is a SQLite database with word definitions and translations between English and Spanish. The data comes from Wiktionary, processed through [kaikki.org](https://kaikki.org/) JSONL dumps.

### Option A: Use Existing Dictionary

If the repository includes `dictionary.db`:

```bash
mkdir -p ~/.local/share/eyers
cp dictionary.db ~/.local/share/eyers/
```

### Option B: Build Dictionary from Scratch

Download Wiktionary JSONL files:

```bash
# English dictionary
wget https://kaikki.org/dictionary/raw-wiktextract-data.jsonl.gz -O en-wiktionary.jsonl.gz
gunzip en-wiktionary.jsonl.gz

# Spanish dictionary
wget https://kaikki.org/dictionary/Spanish/raw-wiktextract-data.jsonl.gz -O es-wiktionary.jsonl.gz
gunzip es-wiktionary.jsonl.gz
```

Run the builder script:

```bash
pip install --user tqdm

python3 scripts/jsonl_to_sqlite.py \
    --en en-wiktionary.jsonl \
    --es es-wiktionary.jsonl \
    --output dictionary.db

mv dictionary.db ~/.local/share/eyers/
```

This takes 10-20 minutes. The resulting database is around 435MB with 2.3M word entries.

## Usage

### Opening a PDF

```bash
eyers document.pdf
```

Or start without arguments and press `o` to open a file picker.

### Modes

The application has two modes:

**Normal Mode** (default): Scroll through the document with `j/k` keys. No cursor visible.

**Visual Mode**: Navigate word by word with a blue cursor. Activate by pressing `v` from Normal mode.

## Keyboard Shortcuts

### Global (any mode)

| Key | Action |
|-----|--------|
| `o` | Open file picker |
| `p` | Open settings |
| `e` | Export annotations to markdown |
| `Tab` | Toggle table of contents / annotations list |
| `b` | Show/hide header bar |
| `+` / `-` | Zoom in/out |
| `Ctrl+d` / `Ctrl+u` | Half page down/up |
| `G` | Go to end |
| `42gg` or `42G` | Go to page 42 |
| `gg` | Go to start |
| `Esc` | Cancel / exit mode |

### Normal Mode 

| Key | Action |
|-----|--------|
| `j` / `k` | Scroll down/up |
| `h` / `l` | Scroll left/right |
| `v` | Enter Visual mode |

### Visual Mode

| Key | Action |
|-----|--------|
| `h/j/k/l` | Navigate words (left/down/up/right) |
| `0` | Start of line |
| `$` | End of line |
| `s` | Toggle selection anchor |
| `y` | Copy selected text |
| `d` | Show definition |
| `a` | Create/edit annotation |
| `fa` | Find next word starting with 'a' |
| `Fa` | Find previous word starting with 'a' |
| `]a` | Next annotation |
| `[a` | Previous annotation |
| `Esc - v` | Exit to Normal mode |

### Table of Contents Panel

| Key | Action |
|-----|--------|
| `j` / `k` | Select next/previous |
| `gg` / `G` | First/last entry |
| `Enter` | Jump to selected |
| `a` | Edit annotation (in annotations mode) |
| `d` | Delete annotation (in annotations mode) |

## Data Storage

### Dictionary

Location: `~/.local/share/eyers/dictionary.db`

SQLite database with three tables:

- `words`: 2.3M entries (word, language code)
- `senses`: definitions and etymologies
- `translations`: English ↔ Spanish translations

### Annotations

Location: `~/.local/share/eyers/annotations.db`

Stores your highlights and notes. Each annotation contains:
- PDF path
- Text selection range (page and word indices)
- Selected text
- Your note
- Timestamps

## TODO

- [ ] Translations capabilities
- [ ] Different highlight colors for diferent categories 
- [ ] LaTEX rendering while taking annotations
- [ ] Copy LaTEX from pdf
- [ ] Add more vim navigation (like Ctrl-o/Ctrl-i, marks, etc)
- [ ] Identify words when they are separate by hyphens
- [ ] Search text in the pdf (!important)
- [ ] Grammar correction on annotations
- [ ] Search definitions inside the popover of definitions

## Known Issues

- The cursor can occasionally get stuck when scrolling down in Visual mode. Go to Normal mode, go down and re-enter Visual mode if this happens.
- Text order may be incorrect in PDFs with complex layouts.
- Zoom is to slow. 
- In visual mode when zoomed the display dosn't update to the sides.
- If annotation panel gets unfocused then Esc dosn't work to leave the panel.
- In certain pdfs, the text selected gets weird spacing.
- In certain pdfs, selecting crosspage text removes all spacing (!important).
