use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{ApplicationWindow, Box, Orientation, PolicyType, ScrolledWindow};
use pdfium_render::prelude::*;
use std::cell::RefCell;
use std::path::Path;

use crate::widgets::{EyersHeaderBar, PdfView, TranslationPanel};

mod imp {
    use super::*;

    pub struct EyersWindow {
        pub header_bar: EyersHeaderBar,
        pub pdf_view: PdfView,
        pub translation_panel: TranslationPanel,
        pub pdfium: RefCell<Option<&'static Pdfium>>,
    }

    impl Default for EyersWindow {
        fn default() -> Self {
            Self {
                header_bar: EyersHeaderBar::new(),
                pdf_view: PdfView::new(),
                translation_panel: TranslationPanel::new(),
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
        let imp = self.imp();

        // Setup header bar
        self.set_titlebar(Some(imp.header_bar.widget()));
        self.setup_open_button();

        // Bind header bar properties to pdf_view properties
        imp.header_bar
            .bind_property("definitions-enabled", &imp.pdf_view, "definitions-enabled")
            .sync_create()
            .build();

        imp.header_bar
            .bind_property("translate-enabled", &imp.pdf_view, "translate-enabled")
            .sync_create()
            .build();

        // Main content layout: vertical box with scrolled PDF view and translation panel
        let main_box = Box::builder().orientation(Orientation::Vertical).build();

        // Scrolled window with PDF view (expands to fill available space)
        let scrolled_window = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Automatic)
            .vscrollbar_policy(PolicyType::Automatic)
            .vexpand(true)
            .child(&imp.pdf_view)
            .build();

        main_box.append(&scrolled_window);

        // Translation panel (hidden by default)
        imp.translation_panel.set_visible(false);
        main_box.append(&imp.translation_panel);

        self.set_child(Some(&main_box));

        // Setup translation panel connections
        self.setup_translation_panel();
    }

    fn setup_translation_panel(&self) {
        let imp = self.imp();

        // Close button hides panel
        let panel = imp.translation_panel.clone();
        imp.translation_panel
            .close_button()
            .connect_clicked(move |_| {
                panel.set_visible(false);
                panel.clear();
            });

        // Connect pdf_view's translate-requested signal to translation panel
        let panel = imp.translation_panel.clone();
        imp.pdf_view.connect_closure(
            "translate-requested",
            false,
            glib::closure_local!(move |_view: &PdfView, text: &str| {
                panel.set_visible(true);
                panel.translate(text.to_string());
            }),
        );
    }

    fn setup_open_button(&self) {
        let window_weak = self.downgrade();

        self.imp()
            .header_bar
            .open_button()
            .connect_clicked(move |_| {
                if let Some(window) = window_weak.upgrade() {
                    window.show_open_dialog();
                }
            });
    }

    fn show_open_dialog(&self) {
        let dialog = gtk::FileDialog::builder().title("Select a PDF").build();
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

    pub fn header_bar(&self) -> &EyersHeaderBar {
        &self.imp().header_bar
    }

    pub fn pdf_view(&self) -> &PdfView {
        &self.imp().pdf_view
    }

    pub fn translation_panel(&self) -> &TranslationPanel {
        &self.imp().translation_panel
    }
}
