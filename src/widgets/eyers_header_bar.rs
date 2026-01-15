use glib::Properties;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{Button, HeaderBar, ToggleButton};
use std::cell::Cell;

mod imp {
    use super::*;

    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::EyersHeaderBar)]
    pub struct EyersHeaderBar {
        pub header_bar: HeaderBar,
        pub open_button: Button,
        pub definitions_toggle: ToggleButton,

        #[property(get, set, default = true)]
        pub definitions_enabled: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for EyersHeaderBar {
        const NAME: &'static str = "EyersHeaderBar";
        type Type = super::EyersHeaderBar;
        type ParentType = glib::Object;
    }

    #[glib::derived_properties]
    impl ObjectImpl for EyersHeaderBar {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_widgets();
        }
    }
}

glib::wrapper! {
    pub struct EyersHeaderBar(ObjectSubclass<imp::EyersHeaderBar>);
}

impl EyersHeaderBar {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    fn setup_widgets(&self) {
        let imp = self.imp();

        // Configure the header bar
        imp.header_bar
            .set_title_widget(Some(&gtk::Label::new(Some("Eyers PDF"))));
        imp.header_bar.set_show_title_buttons(true);

        // Open PDF button
        imp.open_button.set_label("Open PDF");
        imp.header_bar.pack_start(&imp.open_button);

        // Definitions toggle button
        imp.definitions_toggle.set_label("Definitions");
        imp.definitions_toggle.set_active(false);
        imp.header_bar.pack_end(&imp.definitions_toggle);

        // Bind toggle button's "active" to our "definitions-enabled" property (bidirectional)
        imp.definitions_toggle
            .bind_property("active", self, "definitions-enabled")
            .bidirectional()
            .sync_create()
            .build();
    }

    /// Returns the HeaderBar widget to be used with set_titlebar()
    pub fn widget(&self) -> &HeaderBar {
        &self.imp().header_bar
    }

    pub fn open_button(&self) -> &Button {
        &self.imp().open_button
    }

    pub fn definitions_toggle(&self) -> &ToggleButton {
        &self.imp().definitions_toggle
    }
}

impl Default for EyersHeaderBar {
    fn default() -> Self {
        Self::new()
    }
}
