mod modes;
mod objects;
mod services;
mod text_map;
mod widgets;

use gtk::prelude::*;
use gtk::{Application, CssProvider, gdk, gio, glib};
use widgets::EyersWindow;

const APP_ID: &str = "org.gtk_rs.eyers";

fn load_css() {
    let provider = CssProvider::new();
    provider.load_from_string(include_str!("resources/style.css"));

    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("Could not get default display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    app.connect_startup(|_| load_css());

    // Handle activation without file (just open window)
    app.connect_activate(|app| {
        let window = EyersWindow::new(app);
        window.present();
    });

    // Handle opening files from command line
    app.connect_open(|app, files, _| {
        let window = EyersWindow::new(app);

        if let Some(file) = files.first() {
            if let Some(path) = file.path() {
                window.open_file(&path);
            }
        }

        window.present();
    });

    app.run()
}
