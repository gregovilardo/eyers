mod modes;
mod services;
mod text_map;
mod widgets;

use gtk::prelude::*;
use gtk::{gdk, glib, Application, CssProvider};
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

fn build_ui(app: &Application) {
    let window = EyersWindow::new(app);
    window.present();
}

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_startup(|_| load_css());
    app.connect_activate(build_ui);

    app.run()
}
