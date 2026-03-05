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
        pub settings_button: Button,
        pub annotate_button: Button,
        pub definitions_toggle: ToggleButton,
        pub translate_toggle: ToggleButton,

        #[property(get, set, default = false)]
        pub definitions_enabled: Cell<bool>,

        #[property(get, set, default = false)]
        pub translate_enabled: Cell<bool>,
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

        // Style the header bar itself
        imp.header_bar.add_css_class("eyers-headerbar");

        // Configure the header bar
        let title_label = gtk::Label::new(Some("Eyers PDF"));
        title_label.add_css_class("header-title");
        imp.header_bar.set_title_widget(Some(&title_label));
        imp.header_bar.set_show_title_buttons(true);

        // Open PDF button (icon)
        imp.open_button.set_icon_name("document-open-symbolic");
        imp.open_button.set_tooltip_text(Some("Open PDF"));
        imp.open_button.add_css_class("header-open-btn");
        imp.header_bar.pack_start(&imp.open_button);

        // Definitions toggle button (icon)
        imp.definitions_toggle
            .set_icon_name("accessories-dictionary-symbolic");
        imp.definitions_toggle.set_tooltip_text(Some("Definitions"));
        imp.definitions_toggle.set_active(false);
        imp.definitions_toggle
            .add_css_class("header-definitions-toggle");
        imp.header_bar.pack_start(&imp.definitions_toggle);

        // Annotate button (icon)
        imp.annotate_button.set_icon_name("document-edit-symbolic");
        imp.annotate_button
            .set_tooltip_text(Some("Add annotation (a)"));
        imp.annotate_button.add_css_class("header-annotate-btn");
        imp.annotate_button.set_sensitive(false); // Disabled until in visual mode with selection
        imp.header_bar.pack_start(&imp.annotate_button);

        // Settings button (icon)
        imp.settings_button.set_icon_name("emblem-system-symbolic");
        imp.settings_button.set_tooltip_text(Some("Settings"));
        imp.settings_button.add_css_class("header-settings-btn");
        imp.header_bar.pack_start(&imp.settings_button);

        // Translate toggle button (disabled for now - TODO: implement translation feature)
        // imp.translate_toggle.set_icon_name("...");
        // imp.translate_toggle.set_active(false);
        // imp.translate_toggle.set_sensitive(false);
        // imp.translate_toggle.add_css_class("header-translate-toggle");
        // imp.translate_toggle
        //     .set_tooltip_text(Some("Translation feature coming soon"));
        // imp.header_bar.pack_start(&imp.translate_toggle);

        // Bind toggle buttons to properties (bidirectional)
        imp.definitions_toggle
            .bind_property("active", self, "definitions-enabled")
            .bidirectional()
            .sync_create()
            .build();

        imp.translate_toggle
            .bind_property("active", self, "translate-enabled")
            .bidirectional()
            .sync_create()
            .build();

        // Setup mutual exclusion between toggles
        self.setup_mutual_exclusion();
    }

    fn setup_mutual_exclusion(&self) {
        let imp = self.imp();

        // When definitions is toggled ON, turn off translate
        let translate_toggle = imp.translate_toggle.clone();
        imp.definitions_toggle.connect_toggled(move |btn| {
            if btn.is_active() {
                translate_toggle.set_active(false);
            }
        });

        // When translate is toggled ON, turn off definitions
        let definitions_toggle = imp.definitions_toggle.clone();
        imp.translate_toggle.connect_toggled(move |btn| {
            if btn.is_active() {
                definitions_toggle.set_active(false);
            }
        });
    }

    /// Returns the HeaderBar widget to be used with set_titlebar()
    pub fn widget(&self) -> &HeaderBar {
        &self.imp().header_bar
    }

    pub fn open_button(&self) -> &Button {
        &self.imp().open_button
    }

    pub fn settings_button(&self) -> &Button {
        &self.imp().settings_button
    }

    pub fn annotate_button(&self) -> &Button {
        &self.imp().annotate_button
    }

    pub fn definitions_toggle(&self) -> &ToggleButton {
        &self.imp().definitions_toggle
    }

    pub fn translate_toggle(&self) -> &ToggleButton {
        &self.imp().translate_toggle
    }
}

impl Default for EyersHeaderBar {
    fn default() -> Self {
        Self::new()
    }
}
