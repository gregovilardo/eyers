use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct StatusBar {
        pub center_box: gtk::CenterBox,
        pub mode_label: gtk::Label,
        pub pages_indicator_label: gtk::Label,
        pub pdf_name: gtk::Label,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for StatusBar {
        const NAME: &'static str = "EyersStatusBar";
        type Type = super::StatusBar;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for StatusBar {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_widgets();
        }
    }
    impl WidgetImpl for StatusBar {}
}

glib::wrapper! {
    pub struct StatusBar(ObjectSubclass<imp::StatusBar>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusBar {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    fn setup_widgets(&self) {
        let imp = self.imp();

        // Style the header bar itself
        self.add_css_class("statusbar");
        let center_box = &imp.center_box;
        center_box.set_can_focus(false);
        center_box.set_can_target(false);
        center_box.set_hexpand(false);
        center_box.set_vexpand(false);

        // Mode label (left side, before open button)
        imp.mode_label.set_label("NORMAL");
        imp.mode_label.add_css_class("mode-label");
        center_box.set_start_widget(Some(&imp.mode_label));

        imp.pages_indicator_label
            .add_css_class("pages-indicator-label");
        center_box.set_end_widget(Some(&imp.pages_indicator_label));
    }

    pub fn widget(&self) -> &gtk::CenterBox {
        &self.imp().center_box
    }

    pub fn mode_label(&self) -> &gtk::Label {
        &self.imp().mode_label
    }

    pub fn set_mode_text(&self, mode: &str) {
        self.imp().mode_label.set_label(mode);
    }

    pub fn set_pdf_name(&self, name: &str) {
        self.imp().pdf_name.set_label(name);
    }

    pub fn set_pages_indicator_text(&self, text: &str) {
        self.imp().pages_indicator_label.set_label(text);
    }
}
