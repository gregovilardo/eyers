use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{Box, Button, Label, Orientation, Separator, Spinner};
use std::cell::RefCell;

use crate::services::translation;

const MIN_PANEL_HEIGHT: i32 = 80;
const DEFAULT_PANEL_HEIGHT: i32 = 100;

mod imp {
    use super::*;

    pub struct TranslationPanel {
        pub label: Label,
        pub spinner: Spinner,
        pub close_button: Button,
        pub resize_handle: Separator,
        pub panel_height: RefCell<i32>,
    }

    impl Default for TranslationPanel {
        fn default() -> Self {
            Self {
                label: Label::new(None),
                spinner: Spinner::new(),
                close_button: Button::new(),
                resize_handle: Separator::new(Orientation::Horizontal),
                panel_height: RefCell::new(DEFAULT_PANEL_HEIGHT),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TranslationPanel {
        const NAME: &'static str = "TranslationPanel";
        type Type = super::TranslationPanel;
        type ParentType = Box;
    }

    impl ObjectImpl for TranslationPanel {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_widgets();
        }
    }

    impl WidgetImpl for TranslationPanel {}
    impl BoxImpl for TranslationPanel {}
}

glib::wrapper! {
    pub struct TranslationPanel(ObjectSubclass<imp::TranslationPanel>)
        @extends Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl TranslationPanel {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    fn setup_widgets(&self) {
        let imp = self.imp();

        self.set_orientation(Orientation::Vertical);
        self.set_spacing(0);

        // Resize handle at top (visual separator, draggable in future)
        imp.resize_handle.set_margin_bottom(8);
        imp.resize_handle.add_css_class("spacer");
        self.append(&imp.resize_handle);

        // Content area
        let content_box = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(12)
            .margin_start(12)
            .margin_end(12)
            .margin_bottom(12)
            .vexpand(true)
            .build();

        // Translation label
        imp.label.set_wrap(true);
        imp.label.set_xalign(0.0);
        imp.label.set_yalign(0.0);
        imp.label.set_hexpand(true);
        imp.label.set_vexpand(true);
        imp.label.set_selectable(true);
        imp.label.add_css_class("translation-text");
        content_box.append(&imp.label);

        // Spinner (hidden by default)
        imp.spinner.set_visible(false);
        content_box.append(&imp.spinner);

        // Close button
        imp.close_button.set_icon_name("window-close-symbolic");
        imp.close_button.set_valign(gtk::Align::Start);
        imp.close_button.add_css_class("flat");
        content_box.append(&imp.close_button);

        self.append(&content_box);

        // Set initial size
        self.set_size_request(-1, DEFAULT_PANEL_HEIGHT);

        // Apply styling
        self.add_css_class("translation-panel");
    }

    pub fn close_button(&self) -> &Button {
        &self.imp().close_button
    }

    pub fn set_loading(&self, loading: bool) {
        let imp = self.imp();
        imp.spinner.set_visible(loading);
        if loading {
            imp.spinner.start();
            imp.label.set_text("Translating...");
        } else {
            imp.spinner.stop();
        }
    }

    pub fn set_translation(&self, text: &str) {
        self.imp().label.set_text(text);
        self.set_loading(false);
    }

    pub fn set_error(&self, error: &str) {
        self.imp().label.set_markup(&format!(
            "<span color='red'>{}</span>",
            glib::markup_escape_text(error)
        ));
        self.set_loading(false);
    }

    pub fn translate(&self, text: String) {
        self.set_loading(true);

        let (sender, receiver) = std::sync::mpsc::channel::<Result<String, String>>();

        std::thread::spawn(move || {
            let result = translation::translate(&text).map_err(|e| e.to_string());
            let _ = sender.send(result);
        });

        let panel_weak = self.downgrade();
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            if let Ok(result) = receiver.try_recv() {
                if let Some(panel) = panel_weak.upgrade() {
                    match result {
                        Ok(translated) => panel.set_translation(&translated),
                        Err(error) => panel.set_error(&error),
                    }
                }
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    }

    pub fn clear(&self) {
        self.imp().label.set_text("");
        self.set_loading(false);
    }

    pub fn set_panel_height(&self, height: i32) {
        let height = height.max(MIN_PANEL_HEIGHT);
        self.imp().panel_height.replace(height);
        self.set_size_request(-1, height);
    }

    pub fn panel_height(&self) -> i32 {
        *self.imp().panel_height.borrow()
    }
}

impl Default for TranslationPanel {
    fn default() -> Self {
        Self::new()
    }
}
