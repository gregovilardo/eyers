use std::path::Path;

use gtk::gdk::MemoryTexture;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box, Button, FileDialog, GestureClick, HeaderBar, Label,
    Orientation, Picture, PolicyType, Popover, ScrolledWindow, gdk, gio, glib,
};
use pdfium_render::prelude::*;

const APP_ID: &str = "org.gtk_rs.eyers";
const APP_NAME: &str = "Eyers";

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    let app_box = Box::new(Orientation::Vertical, 0);

    app_box.set_margin_top(12);
    app_box.set_margin_bottom(12);
    app_box.set_margin_start(12);
    app_box.set_margin_end(12);

    let scrolled_window = ScrolledWindow::new();
    scrolled_window.set_vexpand(true);
    // Later you'll append widgets to this box:
    // main_box.append(&some_widget);
    app_box.append(&scrolled_window);
    let pdfium = Pdfium::default();
    let document = pdfium
        .load_pdf_from_file(Path::new("./RobBurbea_SeeingThatFrees.pdf"), None)
        .expect("document");

    let pdf_box = Box::new(Orientation::Vertical, 4);
    for (index, page) in document.pages().iter().enumerate() {
        if let Ok(picture) = create_picture(page) {
            pdf_box.append(&picture);
        } else {
            eprintln!("Error creating picture from page");
        }
    }

    scrolled_window.set_child(Some(&pdf_box));

    let window = ApplicationWindow::builder()
        .application(app)
        .title(APP_NAME)
        .child(&app_box)
        .build();
    window.present();
}

fn load_pdf<'a>(pdfium: &'a Pdfium, path: &Path) -> Option<PdfDocument<'a>> {
    match pdfium.load_pdf_from_file(path, None) {
        Ok(doc) => Some(doc),
        Err(err) => {
            eprintln!("Failed to load PDF: {}", err);
            // Optionally: show a GTK error dialog here
            None
        }
    }
}

fn create_picture(page: PdfPage) -> Result<Picture, PdfiumError> {
    // Configure rendering
    let render_config = PdfRenderConfig::new()
        .set_target_width(2000)
        .set_maximum_height(4000);

    // Render to bitmap
    let bitmap = page.render_with_config(&render_config)?;

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
    Ok(picture)
}
