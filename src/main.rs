mod modes;
mod services;
mod text_map;
mod widgets;

use gtk::prelude::*;
use gtk::{glib, Application};
use widgets::EyersWindow;

const APP_ID: &str = "org.gtk_rs.eyers";

fn build_ui(app: &Application) {
    let window = EyersWindow::new(app);
    window.present();
}

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}
