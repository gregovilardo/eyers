use gtk::pango::ffi::PANGO_ELLIPSIZE_END;
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

const APP_ID: &str = "org.gtk_rs.eyers";

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

fn fetch_definition(lookup_word: &str, display_word: &str) -> Option<String> {
    let url = format!(
        "https://api.dictionaryapi.dev/api/v2/entries/en/{}",
        lookup_word
    );
    match reqwest::blocking::get(&url) {
        Ok(response) => {
            if response.status().is_success() {
                if let Ok(entries) = response.json::<Vec<WordEntry>>() {
                    if let Some(entry) = entries.first() {
                        let mut formatted_output = String::new();
                        let escaped_display = glib::markup_escape_text(display_word);
                        formatted_output.push_str(&format!(
                            "<span size='large' weight='bold'>{}</span>\n\n",
                            escaped_display
                        ));

                        for meaning in &entry.meanings {
                            let escaped_pos = glib::markup_escape_text(&meaning.part_of_speech);
                            formatted_output.push_str(&format!("<b><i>{}</i></b>\n", escaped_pos));
                            for (i, def) in meaning.definitions.iter().enumerate() {
                                let escaped_def = glib::markup_escape_text(&def.definition);
                                formatted_output.push_str(&format!(
                                    " {}. {}\n",
                                    i + 1,
                                    escaped_def
                                ));
                            }
                            formatted_output.push_str("\n");
                        }

                        // Trim trailing newlines
                        let final_output = formatted_output.trim().to_string();
                        if !final_output.is_empty() {
                            return Some(final_output);
                        }
                    }
                }
            }
        }
        Err(e) => eprintln!("Network error: {}", e),
    }
    None
}

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    let bindings =
        Pdfium::bind_to_library(&Path::new("./libpdfium.so")).expect("Failed to bind to PDFium");
    let pdfium: &'static Pdfium =
        std::boxed::Box::leak(std::boxed::Box::new(Pdfium::new(bindings)));

    let header_bar = HeaderBar::builder()
        .title_widget(&gtk::Label::new(Some("Eyers PDF")))
        .show_title_buttons(true)
        .build();

    let open_button = Button::builder().label("Open PDF").build();

    header_bar.pack_start(&open_button);

    let content_box = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .build();

    let scrolled_window = ScrolledWindow::builder()
        .hscrollbar_policy(PolicyType::Automatic)
        .vscrollbar_policy(PolicyType::Automatic)
        .child(&content_box)
        .build();

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Eyers")
        .default_width(800)
        .default_height(600)
        .titlebar(&header_bar)
        .child(&scrolled_window)
        .build();

    let window_weak = window.downgrade();
    let content_box_weak = content_box.downgrade();

    // Shared state for the document
    let document_state: Rc<RefCell<Option<PdfDocument<'static>>>> = Rc::new(RefCell::new(None));

    open_button.connect_clicked(move |_| {
        let dialog = FileDialog::builder().title("Select a PDF").build();

        let window = window_weak.clone();
        let content_box = content_box_weak.clone();
        let document_state = document_state.clone();

        if let Some(window) = window.upgrade() {
            dialog.open(Some(&window), None::<&gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        if let Some(content_box) = content_box.upgrade() {
                            load_and_render_pdf(path, &content_box, pdfium, document_state.clone());
                        }
                    }
                }
            });
        }
    });

    window.present();
}

fn load_and_render_pdf(
    path: PathBuf,
    content_box: &Box,
    pdfium: &'static Pdfium,
    document_state: Rc<RefCell<Option<PdfDocument<'static>>>>,
) {
    while let Some(child) = content_box.first_child() {
        content_box.remove(&child);
    }

    let document = match pdfium.load_pdf_from_file(&path, None) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("Failed to open PDF: {}", e);
            return;
        }
    };

    *document_state.borrow_mut() = Some(document);

    let state_borrow = document_state.borrow();
    let doc_ref = state_borrow.as_ref().unwrap();

    for (index, page) in doc_ref.pages().iter().enumerate() {
        let render_config = PdfRenderConfig::new()
            .set_target_width(1000)
            .set_format(PdfBitmapFormat::BGRA);

        if let Ok(bitmap) = page.render_with_config(&render_config) {
            let width = bitmap.width() as i32;
            let height = bitmap.height() as i32;
            let stride = (width * 4) as usize;

            let bytes = bitmap.as_raw_bytes();
            let bytes_glib = glib::Bytes::from(&bytes);

            let texture = gdk::MemoryTexture::new(
                width,
                height,
                gdk::MemoryFormat::B8g8r8a8,
                &bytes_glib,
                stride,
            );

            let picture = Picture::builder()
                .can_shrink(false)
                .paintable(&texture)
                .build();

            let gesture = GestureClick::new();
            let document_state_clone = document_state.clone();
            let picture_clone = picture.clone();

            gesture.connect_pressed(move |_, _, x, y| {
                if let Some(doc) = document_state_clone.borrow().as_ref() {
                    if let Ok(page) = doc.pages().get(index as u16) {
                        let page_width_pts = page.width().value as f64;
                        let page_height_pts = page.height().value as f64;

                        let scale = 1000.0 / page_width_pts;

                        let pdf_x = x / scale;
                        let pdf_y = page_height_pts - (y / scale);

                        if let Ok(text_page) = page.text() {
                            let tolerance = 5.0;
                            let rect = PdfRect::new_from_values(
                                (pdf_y - tolerance) as f32,
                                (pdf_x - tolerance) as f32,
                                (pdf_y + tolerance) as f32,
                                (pdf_x + tolerance) as f32,
                            );

                            let chars_inside_rect = text_page.chars_inside_rect(rect);

                            if let Ok(chars) = chars_inside_rect {
                                if let Some(char_obj) = chars.iter().next() {
                                    let char_index = char_obj.index();

                                    let full_text = text_page.all();
                                    let idx = char_index as usize;
                                    let chars_vec: Vec<char> = full_text.chars().collect();

                                    if idx < chars_vec.len() {
                                        // Scan backwards
                                        let mut start = idx;
                                        while start > 0
                                            && (chars_vec[start].is_alphanumeric()
                                                || chars_vec[start] == '\'')
                                        {
                                            start -= 1;
                                        }
                                        if !chars_vec[start].is_alphanumeric() {
                                            start += 1;
                                        }

                                        // Scan forwards
                                        let mut end = idx;
                                        while end < chars_vec.len()
                                            && (chars_vec[end].is_alphanumeric()
                                                || chars_vec[end] == '\'')
                                        {
                                            end += 1;
                                        }

                                        let word: String = chars_vec[start..end].iter().collect();
                                        let word_lower = word.to_lowercase();
                                        println!(
                                            "Fetching definition for: {} (original: {})",
                                            word_lower, word
                                        );

                                        let popover = Popover::builder().has_arrow(true).build();
                                        popover.set_parent(&picture_clone);

                                        let rect = gdk::Rectangle::new(x as i32, y as i32, 1, 1);
                                        popover.set_pointing_to(Some(&rect));

                                        // Disable autohide so popover stays open until explicitly closed
                                        popover.set_autohide(false);

                                        let label = Label::new(Some("Loading definition..."));
                                        label.set_wrap(true);
                                        // label.set_max_width_chars(80);
                                        label.set_xalign(0.0);
                                        label.set_yalign(0.0);
                                        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                                        label.set_selectable(true);

                                        let text_scroller = ScrolledWindow::builder()
                                            .hscrollbar_policy(PolicyType::Never)
                                            .vscrollbar_policy(PolicyType::Automatic)
                                            .vexpand_set(true)
                                            .min_content_width(500)
                                            .min_content_height(250)
                                            // .max_content_height(500)
                                            .child(&label)
                                            .build();

                                        // Add close button since autohide is disabled
                                        let close_button =
                                            Button::builder().label("Close").margin_top(8).build();

                                        let popover_weak = popover.downgrade();
                                        close_button.connect_clicked(move |_| {
                                            if let Some(p) = popover_weak.upgrade() {
                                                p.popdown();
                                            }
                                        });

                                        let container = Box::builder()
                                            .orientation(Orientation::Vertical)
                                            .spacing(4)
                                            .margin_start(8)
                                            .margin_end(8)
                                            .margin_top(8)
                                            .margin_bottom(8)
                                            .build();
                                        container.append(&text_scroller);
                                        container.append(&close_button);

                                        popover.set_child(Some(&container));
                                        popover.set_size_request(500, 300);
                                        popover.popup();

                                        let word_clone = word_lower.clone();
                                        let label_weak = label.downgrade();

                                        let (sender, receiver) =
                                            std::sync::mpsc::channel::<String>();

                                        let display_word = word.clone();

                                        std::thread::spawn(move || {
                                            let definition =
                                                fetch_definition(&word_clone, &display_word)
                                                    .unwrap_or_else(|| {
                                                        "Definition not found.".to_string()
                                                    });
                                            println!("{}", definition);
                                            let _ = sender.send(definition);
                                        });

                                        glib::timeout_add_local(
                                            std::time::Duration::from_millis(500),
                                            move || {
                                                if let Ok(definition) = receiver.try_recv() {
                                                    if let Some(label) = label_weak.upgrade() {
                                                        label.set_markup(&definition);
                                                    }
                                                    return glib::ControlFlow::Break;
                                                }
                                                glib::ControlFlow::Continue
                                            },
                                        );
                                    }
                                } else {
                                    println!("No character found near click.");
                                }
                            }
                        }
                    }
                }
            });

            picture.add_controller(gesture);
            content_box.append(&picture);
        }
    }
}
