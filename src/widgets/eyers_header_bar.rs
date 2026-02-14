use glib::Properties;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{Button, HeaderBar, Label, ToggleButton};
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
        pub mode_label: Label,
        pub pages_indicator_label: Label,

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

        // Mode label (left side, before open button)
        imp.mode_label.set_label("NORMAL");
        imp.mode_label.add_css_class("mode-label");
        imp.header_bar.pack_start(&imp.mode_label);

        imp.pages_indicator_label
            .add_css_class("pages-indicator-label");
        imp.header_bar.pack_start(&imp.pages_indicator_label);

        // Configure the header bar
        imp.header_bar
            .set_title_widget(Some(&gtk::Label::new(Some("Eyers PDF"))));
        imp.header_bar.set_show_title_buttons(true);

        // Open PDF button
        imp.open_button.set_label("Open PDF");
        imp.header_bar.pack_start(&imp.open_button);

        // Translate toggle button (disabled for now - TODO: implement translation feature)
        // imp.translate_toggle.set_label("Translate");
        // imp.translate_toggle.set_active(false);
        // imp.translate_toggle.set_sensitive(false);
        // imp.translate_toggle
        //     .set_tooltip_text(Some("Translation feature coming soon"));
        // imp.header_bar.pack_end(&imp.translate_toggle);

        // Settings button (gear icon)
        imp.settings_button.set_icon_name("emblem-system-symbolic");
        imp.settings_button.set_tooltip_text(Some("Settings"));
        imp.header_bar.pack_end(&imp.settings_button);

        // Definitions toggle button
        imp.definitions_toggle.set_label("Definitions");
        imp.definitions_toggle.set_active(false);
        imp.header_bar.pack_end(&imp.definitions_toggle);

        // Annotate button (note-taking icon)
        imp.annotate_button.set_icon_name("document-edit-symbolic");
        imp.annotate_button
            .set_tooltip_text(Some("Add annotation (a)"));
        imp.annotate_button.set_sensitive(false); // Disabled until in visual mode with selection
        imp.header_bar.pack_end(&imp.annotate_button);

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

    pub fn mode_label(&self) -> &Label {
        &self.imp().mode_label
    }

    pub fn set_mode_text(&self, mode: &str) {
        self.imp().mode_label.set_label(mode);
    }

    pub fn set_pages_indicator_text(&self, text: &str) {
        self.imp().pages_indicator_label.set_label(text);
    }
}

impl Default for EyersHeaderBar {
    fn default() -> Self {
        Self::new()
    }
}
