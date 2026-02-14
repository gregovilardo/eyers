use glib::Properties;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{Box, Button, DropDown, Label, Orientation, StringList, Window};
use std::cell::Cell;

use crate::services::dictionary::Language;

mod imp {
    use super::*;

    #[derive(Properties)]
    #[properties(wrapper_type = super::SettingsWindow)]
    pub struct SettingsWindow {
        pub language_dropdown: DropDown,

        #[property(get, set, default = 0)]
        pub selected_language: Cell<u32>,
    }

    impl Default for SettingsWindow {
        fn default() -> Self {
            let languages = StringList::new(&["English", "Spanish"]);
            let dropdown = DropDown::new(Some(languages), None::<gtk::Expression>);

            Self {
                language_dropdown: dropdown,
                selected_language: Cell::new(0),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SettingsWindow {
        const NAME: &'static str = "SettingsWindow";
        type Type = super::SettingsWindow;
        type ParentType = Window;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SettingsWindow {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_widgets();
        }
    }

    impl WidgetImpl for SettingsWindow {}
    impl WindowImpl for SettingsWindow {}
}

glib::wrapper! {
    pub struct SettingsWindow(ObjectSubclass<imp::SettingsWindow>)
        @extends Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl SettingsWindow {
    pub fn new(parent: &impl IsA<Window>) -> Self {
        glib::Object::builder()
            .property("transient-for", parent)
            .property("modal", true)
            .property("title", "Settings")
            .property("default-width", 400)
            .property("default-height", 200)
            .property("resizable", false)
            .build()
    }

    fn setup_widgets(&self) {
        let imp = self.imp();

        // Main container
        let main_box = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(16)
            .margin_start(24)
            .margin_end(24)
            .margin_top(24)
            .margin_bottom(24)
            .build();

        // Language section
        let lang_box = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(12)
            .build();

        let lang_label = Label::builder()
            .label("Dictionary Language:")
            .halign(gtk::Align::Start)
            .hexpand(true)
            .build();

        lang_box.append(&lang_label);
        lang_box.append(&imp.language_dropdown);

        // Description label
        let desc_label = Label::builder()
            .label("Select the language for dictionary definitions.\nEnglish: Look up English words, get Spanish translations.\nSpanish: Look up Spanish words, get English translations.")
            .halign(gtk::Align::Start)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();

        main_box.append(&lang_box);
        main_box.append(&desc_label);

        // Close button
        let close_button = Button::builder()
            .label("Close")
            .halign(gtk::Align::End)
            .margin_top(8)
            .build();

        let window_weak = self.downgrade();
        close_button.connect_clicked(move |_| {
            if let Some(window) = window_weak.upgrade() {
                window.close();
            }
        });

        main_box.append(&close_button);

        self.set_child(Some(&main_box));

        // Connect dropdown selection changes to property
        let window_weak = self.downgrade();
        imp.language_dropdown
            .connect_selected_notify(move |dropdown| {
                if let Some(window) = window_weak.upgrade() {
                    window.set_selected_language(dropdown.selected());
                }
            });
    }

    /// Returns the currently selected language
    pub fn language(&self) -> Language {
        match self.selected_language() {
            1 => Language::Spanish,
            _ => Language::English,
        }
    }

    /// Sets the language in the dropdown
    pub fn set_language(&self, lang: Language) {
        let idx = match lang {
            Language::English => 0,
            Language::Spanish => 1,
        };
        self.imp().language_dropdown.set_selected(idx);
    }

    /// Returns a reference to the language dropdown for signal connections
    pub fn language_dropdown(&self) -> &DropDown {
        &self.imp().language_dropdown
    }
}

impl Default for SettingsWindow {
    fn default() -> Self {
        glib::Object::builder().build()
    }
}
