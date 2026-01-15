use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{ApplicationWindow, Button, FileDialog, HeaderBar, PolicyType, ScrolledWindow};
use pdfium_render::prelude::*;
use std::cell::RefCell;
use std::path::Path;

use crate::widgets::PdfView;

mod imp {
    use super::*;

    pub struct EyersWindow {
        pub pdf_view: PdfView,
        pub pdfium: RefCell<Option<&'static Pdfium>>,
    }

    impl Default for EyersWindow {
        fn default() -> Self {
            Self {
                pdf_view: PdfView::new(),
                pdfium: RefCell::new(None),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for EyersWindow {
        const NAME: &'static str = "EyersWindow";
        type Type = super::EyersWindow;
        type ParentType = ApplicationWindow;
    }

    impl ObjectImpl for EyersWindow {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_widgets();
        }
    }

    impl WidgetImpl for EyersWindow {}
    impl WindowImpl for EyersWindow {}
    impl ApplicationWindowImpl for EyersWindow {}
}

glib::wrapper! {
    pub struct EyersWindow(ObjectSubclass<imp::EyersWindow>)
        @extends ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl EyersWindow {
    pub fn new(app: &gtk::Application) -> Self {
        let window: Self = glib::Object::builder()
            .property("application", app)
            .property("title", "Eyers")
            .property("default-width", 800)
            .property("default-height", 600)
            .build();

        window.init_pdfium();
        window
    }

    fn init_pdfium(&self) {
        let bindings =
            Pdfium::bind_to_library(Path::new("./libpdfium.so")).expect("Failed to bind to PDFium");
        let pdfium: &'static Pdfium =
            std::boxed::Box::leak(std::boxed::Box::new(Pdfium::new(bindings)));

        self.imp().pdfium.replace(Some(pdfium));
        self.imp().pdf_view.set_pdfium(pdfium);
    }

    fn setup_widgets(&self) {
        let header_bar = self.create_header_bar();
        self.set_titlebar(Some(&header_bar));

        let scrolled_window = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Automatic)
            .vscrollbar_policy(PolicyType::Automatic)
            .child(&self.imp().pdf_view)
            .build();

        self.set_child(Some(&scrolled_window));
    }

    fn create_header_bar(&self) -> HeaderBar {
        let header_bar = HeaderBar::builder()
            .title_widget(&gtk::Label::new(Some("Eyers PDF")))
            .show_title_buttons(true)
            .build();

        let open_button = Button::builder().label("Open PDF").build();
        self.setup_open_button(&open_button);
        header_bar.pack_start(&open_button);

        header_bar
    }

    fn setup_open_button(&self, button: &Button) {
        let window_weak = self.downgrade();

        button.connect_clicked(move |_| {
            if let Some(window) = window_weak.upgrade() {
                window.show_open_dialog();
            }
        });
    }

    fn show_open_dialog(&self) {
        let dialog = FileDialog::builder().title("Select a PDF").build();
        let window_weak = self.downgrade();

        dialog.open(Some(self), None::<&gio::Cancellable>, move |result| {
            if let Some(window) = window_weak.upgrade() {
                window.handle_file_dialog_result(result);
            }
        });
    }

    fn handle_file_dialog_result(&self, result: Result<gio::File, glib::Error>) {
        let file = match result {
            Ok(f) => f,
            Err(_) => return,
        };

        let path = match file.path() {
            Some(p) => p,
            None => return,
        };

        if let Err(e) = self.imp().pdf_view.load_pdf(path) {
            eprintln!("{}", e);
        }
    }

    pub fn pdf_view(&self) -> &PdfView {
        &self.imp().pdf_view
    }
}
