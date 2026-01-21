use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{ApplicationWindow, Box, Orientation, Paned, PolicyType, ScrolledWindow};
use pdfium_render::prelude::*;
use std::cell::RefCell;
use std::path::Path;

use crate::widgets::{EyersHeaderBar, PdfView, TocPanel, TranslationPanel};

mod imp {
    use super::*;

    pub struct EyersWindow {
        pub header_bar: EyersHeaderBar,
        pub pdf_view: PdfView,
        pub toc_panel: TocPanel,
        pub translation_panel: TranslationPanel,
        pub pdfium: RefCell<Option<&'static Pdfium>>,
        pub paned: RefCell<Option<Paned>>,
    }

    impl Default for EyersWindow {
        fn default() -> Self {
            Self {
                header_bar: EyersHeaderBar::new(),
                pdf_view: PdfView::new(),
                toc_panel: TocPanel::new(),
                translation_panel: TranslationPanel::new(),
                pdfium: RefCell::new(None),
                paned: RefCell::new(None),
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
            .property("default-width", 1000)
            .property("default-height", 700)
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

        self.set_titlebar(Some(imp.header_bar.widget()));
        self.setup_open_button();

        imp.header_bar
            .bind_property("definitions-enabled", &imp.pdf_view, "definitions-enabled")
            .sync_create()
            .build();

        imp.header_bar
            .bind_property("translate-enabled", &imp.pdf_view, "translate-enabled")
            .sync_create()
            .build();

        let paned = Paned::builder()
            .orientation(Orientation::Horizontal)
            .build();
        paned.set_wide_handle(true);

        let scrolled_window = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Automatic)
            .vscrollbar_policy(PolicyType::Automatic)
            .vexpand(true)
            .hexpand(true)
            .child(&imp.pdf_view)
            .build();

        paned.set_start_child(Some(&scrolled_window));
        paned.set_end_child(Some(&imp.toc_panel));
        paned.set_resize_start_child(true);
        paned.set_shrink_start_child(false);
        paned.set_resize_end_child(false);
        paned.set_shrink_end_child(false);
        paned.set_position(800);

        imp.paned.replace(Some(paned.clone()));

        let main_box = Box::builder().orientation(Orientation::Vertical).build();

        main_box.append(&paned);

        imp.translation_panel.set_visible(false);
        main_box.append(&imp.translation_panel);

        self.set_child(Some(&main_box));

        self.setup_translation_panel();
        self.setup_toc_panel();
        self.setup_keyboard_controller();
        self.setup_scroll_tracking();
    }

    fn setup_scroll_tracking(&self) {
        let pdf_view = self.imp().pdf_view.clone();

        if let Some(scrolled_window) = self.scrolled_window() {
            let adjustment = scrolled_window.vadjustment();

            adjustment.connect_value_changed(move |_| {
                pdf_view.schedule_page_update();
            });
        }
    }

    fn scrolled_window(&self) -> Option<gtk::ScrolledWindow> {
        self.imp()
            .paned
            .borrow()
            .as_ref()?
            .start_child()? //Get the first child of the paned (the "start" pane)
            .downcast()
            .ok()
    }

    fn setup_translation_panel(&self) {
        let imp = self.imp();

        let panel = imp.translation_panel.clone();
        imp.translation_panel
            .close_button()
            .connect_clicked(move |_| {
                panel.set_visible(false);
                panel.clear();
            });

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

    fn setup_toc_panel(&self) {
        let imp = self.imp();

        let panel = imp.toc_panel.clone();
        imp.toc_panel.close_button().connect_clicked(move |_| {
            panel.set_visible(false);
        });

        let pdf_view = imp.pdf_view.clone();
        imp.toc_panel.connect_closure(
            "chapter-selected",
            false,
            glib::closure_local!(move |_panel: &TocPanel, page_index: u32| {
                pdf_view.scroll_to_page(page_index as u16);
            }),
        );
    }

    fn setup_keyboard_controller(&self) {
        let controller = gtk::EventControllerKey::new();
        let window_weak = self.downgrade();

        controller.connect_key_pressed(move |_, key, _, _| {
            if let Some(window) = window_weak.upgrade() {
                let imp = window.imp();
                let toc_visible = imp.toc_panel.is_visible();

                if key == gtk::gdk::Key::Tab {
                    window.toggle_toc_panel();
                    return glib::Propagation::Stop;
                }

                if toc_visible {
                    match key {
                        gtk::gdk::Key::j | gtk::gdk::Key::Down => {
                            imp.toc_panel.select_next();
                            return glib::Propagation::Stop;
                        }
                        gtk::gdk::Key::k | gtk::gdk::Key::Up => {
                            imp.toc_panel.select_prev();
                            return glib::Propagation::Stop;
                        }
                        gtk::gdk::Key::Return => {
                            imp.toc_panel.navigate_and_close();
                            return glib::Propagation::Stop;
                        }
                        gtk::gdk::Key::Escape => {
                            imp.toc_panel.set_visible(false);
                            return glib::Propagation::Stop;
                        }
                        _ => {}
                    }
                }
            }
            glib::Propagation::Proceed
        });

        self.add_controller(controller);
    }

    fn toggle_toc_panel(&self) {
        let imp = self.imp();
        let is_visible = imp.toc_panel.is_visible();
        imp.toc_panel.set_visible(!is_visible);

        if !is_visible {
            imp.toc_panel.grab_focus();
            let current_page = imp.pdf_view.current_page();
            imp.toc_panel.select_current_chapter(current_page);
        }
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
            return;
        }

        self.extract_and_populate_bookmarks();
    }

    fn extract_and_populate_bookmarks(&self) {
        let bookmarks = self.imp().pdf_view.bookmarks();
        self.imp().toc_panel.populate(&bookmarks);
    }

    pub fn header_bar(&self) -> &EyersHeaderBar {
        &self.imp().header_bar
    }

    pub fn pdf_view(&self) -> &PdfView {
        &self.imp().pdf_view
    }

    pub fn toc_panel(&self) -> &TocPanel {
        &self.imp().toc_panel
    }

    pub fn translation_panel(&self) -> &TranslationPanel {
        &self.imp().translation_panel
    }
}
