use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct PendingKeyBox {
        pub label: gtk::Label,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PendingKeyBox {
        const NAME: &'static str = "EyersPendingKeyBox";
        type Type = super::PendingKeyBox;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for PendingKeyBox {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            // Configure the box
            obj.set_orientation(gtk::Orientation::Horizontal);
            obj.set_halign(gtk::Align::Center);
            obj.set_valign(gtk::Align::End);
            obj.set_margin_bottom(12);

            // Style the label
            self.label.set_halign(gtk::Align::Center);
            self.label.add_css_class("pendingkey-label");

            // Create inner box for styling
            let inner_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            inner_box.add_css_class("pendingkey");
            inner_box.set_margin_start(16);
            inner_box.set_margin_end(16);
            inner_box.set_margin_top(8);
            inner_box.set_margin_bottom(8);
            inner_box.append(&self.label);

            obj.append(&inner_box);

            // Initially hidden
            obj.set_visible(false);
        }
    }

    impl WidgetImpl for PendingKeyBox {}
    impl BoxImpl for PendingKeyBox {}
}

glib::wrapper! {
    /// Shows at the bottom of the window when there's pending input,
    /// like "42g" when the user has typed 42 and then g, waiting for
    /// the second g.
    pub struct PendingKeyBox(ObjectSubclass<imp::PendingKeyBox>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl Default for PendingKeyBox {
    fn default() -> Self {
        Self::new()
    }
}

impl PendingKeyBox {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Set the status text to display
    pub fn set_pendingkey_text(&self, text: &str) {
        self.imp().label.set_text(text);

        // Show/hide based on whether there's text
        let should_show = !text.is_empty();
        if self.is_visible() != should_show {
            self.set_visible(should_show);
        }
    }

    /// Get the current status text
    pub fn pendingkey_text(&self) -> String {
        self.imp().label.text().to_string()
    }
}
