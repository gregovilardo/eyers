use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box, Button, FileDialog, GestureClick, HeaderBar, Label,
    Orientation, Picture, PolicyType, Popover, ScrolledWindow, gdk, gio, glib,
};
use pdfium_render::prelude::*;
use serde::Deserialize;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

// ============================================================================
// CONSTANTS
// ============================================================================

const APP_ID: &str = "org.gtk_rs.eyers";
const RENDER_WIDTH: i32 = 1000;
const CLICK_TOLERANCE: f64 = 5.0;
const POPOVER_WIDTH: i32 = 500;
const POPOVER_HEIGHT: i32 = 200;
const DEFINITION_POLL_MS: u64 = 500;

// ============================================================================
// DATA STRUCTURES - Core application state
// ============================================================================

type DocumentState = Rc<RefCell<Option<PdfDocument<'static>>>>;

#[derive(Deserialize, Debug)]
struct WordEntry {
    #[serde(default)]
    meanings: Vec<Meaning>,
}

#[derive(Deserialize, Debug)]
struct Meaning {
    #[serde(rename = "partOfSpeech")]
    part_of_speech: String,
    #[serde(default)]
    definitions: Vec<Definition>,
}

#[derive(Deserialize, Debug)]
struct Definition {
    definition: String,
}

/// Data extracted from a click event on a PDF page
struct ClickData {
    pdf_x: f64,
    pdf_y: f64,
    screen_x: f64,
    screen_y: f64,
}

/// Word extracted from PDF text
struct ExtractedWord {
    original: String,
    lowercase: String,
}

/// Configuration for rendering a PDF page
struct PageRenderConfig {
    width: i32,
    height: i32,
    stride: usize,
}

// ============================================================================
// DICTIONARY API - Data fetching and formatting
// ============================================================================

fn fetch_definition(lookup_word: &str, display_word: &str) -> Option<String> {
    let url = format!(
        "https://api.dictionaryapi.dev/api/v2/entries/en/{}",
        lookup_word
    );

    let response = reqwest::blocking::get(&url).ok()?;
    if !response.status().is_success() {
        return None;
    }

    let entries: Vec<WordEntry> = response.json().ok()?;
    let entry = entries.first()?;

    format_definition(entry, display_word)
}

fn format_definition(entry: &WordEntry, display_word: &str) -> Option<String> {
    let mut output = String::new();
    let escaped_display = glib::markup_escape_text(display_word);

    output.push_str(&format!(
        "<span size='large' weight='bold'>{}</span>\n\n",
        escaped_display
    ));

    for meaning in &entry.meanings {
        format_meaning(&mut output, meaning);
    }

    let final_output = output.trim().to_string();
    if final_output.is_empty() {
        None
    } else {
        Some(final_output)
    }
}

fn format_meaning(output: &mut String, meaning: &Meaning) {
    let escaped_pos = glib::markup_escape_text(&meaning.part_of_speech);
    output.push_str(&format!("<b><i>{}</i></b>\n", escaped_pos));

    for (i, def) in meaning.definitions.iter().enumerate() {
        let escaped_def = glib::markup_escape_text(&def.definition);
        output.push_str(&format!(" {}. {}\n", i + 1, escaped_def));
    }
    output.push('\n');
}

// ============================================================================
// PDF TEXT EXTRACTION - Pure data transformations
// ============================================================================

fn calculate_click_coordinates(x: f64, y: f64, page: &PdfPage) -> ClickData {
    let page_width_pts = page.width().value as f64;
    let page_height_pts = page.height().value as f64;
    let scale = RENDER_WIDTH as f64 / page_width_pts;

    ClickData {
        pdf_x: x / scale,
        pdf_y: page_height_pts - (y / scale),
        screen_x: x,
        screen_y: y,
    }
}

fn create_click_rect(click: &ClickData) -> PdfRect {
    PdfRect::new_from_values(
        (click.pdf_y - CLICK_TOLERANCE) as f32,
        (click.pdf_x - CLICK_TOLERANCE) as f32,
        (click.pdf_y + CLICK_TOLERANCE) as f32,
        (click.pdf_x + CLICK_TOLERANCE) as f32,
    )
}

fn find_char_index_at_click(text_page: &PdfPageText, click: &ClickData) -> Option<usize> {
    let rect = create_click_rect(click);
    let chars = text_page.chars_inside_rect(rect).ok()?;
    let char_obj = chars.iter().next()?;
    Some(char_obj.index() as usize)
}

fn extract_word_at_index(full_text: &str, idx: usize) -> Option<ExtractedWord> {
    let chars_vec: Vec<char> = full_text.chars().collect();
    if idx >= chars_vec.len() {
        return None;
    }

    let start = find_word_start(&chars_vec, idx);
    let end = find_word_end(&chars_vec, idx);

    if start > end {
        return None;
    }
    let original: String = chars_vec[start..end].iter().collect();
    let lowercase = original.to_lowercase();
    Some(ExtractedWord {
        original,
        lowercase,
    })
}

fn find_word_start(chars: &[char], idx: usize) -> usize {
    let mut start = idx;
    while start > 0 && is_word_char(chars[start]) {
        start -= 1;
    }
    if !is_word_char(chars[start]) {
        start += 1;
    }
    start
}

fn find_word_end(chars: &[char], idx: usize) -> usize {
    let mut end = idx;
    while end < chars.len() && is_word_char(chars[end]) {
        end += 1;
    }
    end
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '\''
}

// ============================================================================
// UI BUILDERS - Widget creation functions
// ============================================================================

fn create_header_bar() -> HeaderBar {
    HeaderBar::builder()
        .title_widget(&gtk::Label::new(Some("Eyers PDF")))
        .show_title_buttons(true)
        .build()
}

fn create_open_button() -> Button {
    Button::builder().label("Open PDF").build()
}

fn create_content_box() -> Box {
    Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .build()
}

fn create_scrolled_window(child: &impl IsA<gtk::Widget>) -> ScrolledWindow {
    ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Automatic)
        .vscrollbar_policy(PolicyType::Automatic)
        .child(child)
        .build()
}

fn create_main_window(
    app: &Application,
    header: &HeaderBar,
    content: &ScrolledWindow,
) -> ApplicationWindow {
    ApplicationWindow::builder()
        .application(app)
        .title("Eyers")
        .default_width(800)
        .default_height(600)
        .titlebar(header)
        .child(content)
        .build()
}

fn create_definition_label() -> Label {
    Label::builder()
        .label("Loading definition...")
        .wrap(true)
        .xalign(0.0)
        .yalign(0.0)
        .selectable(true)
        .build()
}

fn create_definition_scroller(label: &Label) -> ScrolledWindow {
    ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Never)
        .vscrollbar_policy(PolicyType::Automatic)
        .vexpand_set(true)
        .min_content_width(POPOVER_WIDTH)
        .min_content_height(POPOVER_HEIGHT)
        .child(label)
        .build()
}

fn create_popover_container(scroller: &ScrolledWindow, close_button: &Button) -> Box {
    let container = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .margin_start(8)
        .margin_end(8)
        .margin_top(8)
        .margin_bottom(8)
        .build();
    container.append(scroller);
    container.append(close_button);
    container
}

fn create_close_button(popover: &Popover) -> Button {
    let button = Button::builder().label("Close").margin_top(8).build();
    let popover_weak = popover.downgrade();

    button.connect_clicked(move |_| {
        if let Some(p) = popover_weak.upgrade() {
            p.popdown();
        }
    });
    button
}

// ============================================================================
// POPOVER - Definition display
// ============================================================================

fn create_definition_popover(picture: &Picture, x: f64, y: f64) -> (Popover, Label) {
    let popover = Popover::builder().has_arrow(true).build();
    popover.set_parent(picture);
    popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    popover.set_autohide(false);
    popover.set_position(gtk::PositionType::Bottom);

    let label = create_definition_label();
    // label.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let scroller = create_definition_scroller(&label);
    let close_button = create_close_button(&popover);
    let container = create_popover_container(&scroller, &close_button);

    popover.set_child(Some(&container));
    popover.set_size_request(POPOVER_WIDTH, POPOVER_HEIGHT);

    (popover, label)
}

fn spawn_definition_fetch(word: ExtractedWord, label: Label) {
    let (sender, receiver) = std::sync::mpsc::channel::<String>();
    let lookup = word.lowercase.clone();
    let display = word.original.clone();

    std::thread::spawn(move || {
        let definition = fetch_definition(&lookup, &display)
            .unwrap_or_else(|| "Definition not found.".to_string());
        let _ = sender.send(definition);
    });

    let label_weak = label.downgrade();
    glib::timeout_add_local(
        std::time::Duration::from_millis(DEFINITION_POLL_MS),
        move || poll_definition_result(&receiver, &label_weak),
    );
}

fn poll_definition_result(
    receiver: &std::sync::mpsc::Receiver<String>,
    label_weak: &glib::WeakRef<Label>,
) -> glib::ControlFlow {
    if let Ok(definition) = receiver.try_recv() {
        if let Some(label) = label_weak.upgrade() {
            label.set_markup(&definition);
        }
        return glib::ControlFlow::Break;
    }
    glib::ControlFlow::Continue
}

// ============================================================================
// PDF RENDERING
// ============================================================================

fn create_render_config() -> PdfRenderConfig {
    PdfRenderConfig::new()
        .set_target_width(RENDER_WIDTH)
        .set_format(PdfBitmapFormat::BGRA)
}

fn calculate_page_dimensions(bitmap: &PdfBitmap) -> PageRenderConfig {
    let width = bitmap.width() as i32;
    let height = bitmap.height() as i32;
    PageRenderConfig {
        width,
        height,
        stride: (width * 4) as usize,
    }
}

fn create_texture_from_bitmap(bitmap: &PdfBitmap, config: &PageRenderConfig) -> gdk::MemoryTexture {
    let bytes = bitmap.as_raw_bytes();
    let bytes_glib = glib::Bytes::from(&bytes);

    gdk::MemoryTexture::new(
        config.width,
        config.height,
        gdk::MemoryFormat::B8g8r8a8,
        &bytes_glib,
        config.stride,
    )
}

fn create_page_picture(texture: &gdk::MemoryTexture) -> Picture {
    Picture::builder()
        .can_shrink(false)
        .paintable(texture)
        .build()
}

fn clear_content_box(content_box: &Box) {
    while let Some(child) = content_box.first_child() {
        content_box.remove(&child);
    }
}

// ============================================================================
// CLICK HANDLER - Event processing
// ============================================================================

fn handle_page_click(
    x: f64,
    y: f64,
    page_index: usize,
    document_state: &DocumentState,
    picture: &Picture,
) {
    let state_borrow = document_state.borrow();
    let doc = match state_borrow.as_ref() {
        Some(d) => d,
        None => return,
    };

    let page = match doc.pages().get(page_index as u16) {
        Ok(p) => p,
        Err(_) => return,
    };

    let click = calculate_click_coordinates(x, y, &page);
    process_click_on_page(&page, &click, picture);
}

fn process_click_on_page(page: &PdfPage, click: &ClickData, picture: &Picture) {
    let text_page = match page.text() {
        Ok(tp) => tp,
        Err(_) => return,
    };

    let char_idx = match find_char_index_at_click(&text_page, click) {
        Some(idx) => idx,
        None => {
            println!("No character found near click.");
            return;
        }
    };

    let full_text = text_page.all();
    if let Some(word) = extract_word_at_index(&full_text, char_idx) {
        let (popover, label) = create_definition_popover(picture, click.screen_x, click.screen_y);
        popover.popup();
        spawn_definition_fetch(word, label);
    }
}

// ============================================================================
// PAGE SETUP - Gesture and rendering
// ============================================================================

fn setup_page_gesture(picture: &Picture, page_index: usize, document_state: DocumentState) {
    let gesture = GestureClick::new();
    let picture_clone = picture.clone();

    gesture.connect_pressed(move |_, _, x, y| {
        handle_page_click(x, y, page_index, &document_state, &picture_clone);
    });

    picture.add_controller(gesture);
}

fn render_single_page(page: &PdfPage) -> Option<Picture> {
    let config = create_render_config();
    let bitmap = page.render_with_config(&config).ok()?;

    let dimensions = calculate_page_dimensions(&bitmap);
    let texture = create_texture_from_bitmap(&bitmap, &dimensions);

    Some(create_page_picture(&texture))
}

fn render_pdf_pages(content_box: &Box, document_state: &DocumentState) {
    let state_borrow = document_state.borrow();
    let doc = state_borrow.as_ref().unwrap();

    for (index, page) in doc.pages().iter().enumerate() {
        if let Some(picture) = render_single_page(&page) {
            setup_page_gesture(&picture, index, document_state.clone());
            content_box.append(&picture);
        }
    }
}

// ============================================================================
// PDF LOADING
// ============================================================================

fn load_pdf_document(
    path: &PathBuf,
    pdfium: &'static Pdfium,
) -> Result<PdfDocument<'static>, PdfiumError> {
    pdfium.load_pdf_from_file(path, None)
}

fn load_and_render_pdf(
    path: PathBuf,
    content_box: &Box,
    pdfium: &'static Pdfium,
    document_state: DocumentState,
) {
    clear_content_box(content_box);

    let document = match load_pdf_document(&path, pdfium) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("Failed to open PDF: {}", e);
            return;
        }
    };

    *document_state.borrow_mut() = Some(document);
    render_pdf_pages(content_box, &document_state);
}

// ============================================================================
// APPLICATION SETUP
// ============================================================================

fn init_pdfium() -> &'static Pdfium {
    let bindings =
        Pdfium::bind_to_library(Path::new("./libpdfium.so")).expect("Failed to bind to PDFium");
    std::boxed::Box::leak(std::boxed::Box::new(Pdfium::new(bindings)))
}

fn setup_open_button(
    button: &Button,
    window: &ApplicationWindow,
    content_box: &Box,
    pdfium: &'static Pdfium,
) {
    let window_weak = window.downgrade();
    let content_box_weak = content_box.downgrade();
    let document_state: DocumentState = Rc::new(RefCell::new(None));

    button.connect_clicked(move |_| {
        handle_open_button_click(&window_weak, &content_box_weak, pdfium, &document_state);
    });
}

fn handle_open_button_click(
    window_weak: &glib::WeakRef<ApplicationWindow>,
    content_box_weak: &glib::WeakRef<Box>,
    pdfium: &'static Pdfium,
    document_state: &DocumentState,
) {
    let dialog = FileDialog::builder().title("Select a PDF").build();
    let window = match window_weak.upgrade() {
        Some(w) => w,
        None => return,
    };

    let content_box_weak = content_box_weak.clone();
    let document_state = document_state.clone();

    dialog.open(Some(&window), None::<&gio::Cancellable>, move |result| {
        handle_file_dialog_result(result, &content_box_weak, pdfium, &document_state);
    });
}

fn handle_file_dialog_result(
    result: Result<gio::File, glib::Error>,
    content_box_weak: &glib::WeakRef<Box>,
    pdfium: &'static Pdfium,
    document_state: &DocumentState,
) {
    let file = match result {
        Ok(f) => f,
        Err(_) => return,
    };

    let path = match file.path() {
        Some(p) => p,
        None => return,
    };

    if let Some(content_box) = content_box_weak.upgrade() {
        load_and_render_pdf(path, &content_box, pdfium, document_state.clone());
    }
}

// ============================================================================
// MAIN UI BUILDER
// ============================================================================

fn build_ui(app: &Application) {
    let pdfium = init_pdfium();

    let header_bar = create_header_bar();
    let open_button = create_open_button();
    header_bar.pack_start(&open_button);

    let content_box = create_content_box();
    let scrolled_window = create_scrolled_window(&content_box);
    let window = create_main_window(app, &header_bar, &scrolled_window);

    setup_open_button(&open_button, &window, &content_box, pdfium);
    window.present();
}

// ============================================================================
// ENTRY POINT
// ============================================================================

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}
