use gtk::glib;
use gtk::glib::signal::SignalHandlerId;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{Box, Button, Label, Orientation, ScrolledWindow, Separator, TextView};
use std::cell::{Cell, RefCell};
use std::sync::OnceLock;

const MIN_PANEL_HEIGHT: i32 = 120;
const DEFAULT_PANEL_HEIGHT: i32 = 150;

mod imp {
    use super::*;

    pub struct AnnotationPanel {
        pub selected_text_label: Label,
        pub text_view: TextView,
        pub scrolled_window: ScrolledWindow,
        pub save_button: Button,
        pub cancel_button: Button,
        pub delete_button: Button,
        pub resize_handle: Separator,
        pub panel_height: RefCell<i32>,
        /// The annotation ID if we're editing an existing annotation
        pub annotation_id: Cell<Option<i64>>,
        /// Signal handler for key press on text view
        pub key_handler_id: RefCell<Option<SignalHandlerId>>,
    }

    impl Default for AnnotationPanel {
        fn default() -> Self {
            Self {
                selected_text_label: Label::new(None),
                text_view: TextView::new(),
                scrolled_window: ScrolledWindow::new(),
                save_button: Button::new(),
                cancel_button: Button::new(),
                delete_button: Button::new(),
                resize_handle: Separator::new(Orientation::Horizontal),
                panel_height: RefCell::new(DEFAULT_PANEL_HEIGHT),
                annotation_id: Cell::new(None),
                key_handler_id: RefCell::new(None),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AnnotationPanel {
        const NAME: &'static str = "AnnotationPanel";
        type Type = super::AnnotationPanel;
        type ParentType = Box;
    }

    impl ObjectImpl for AnnotationPanel {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_widgets();
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: OnceLock<Vec<glib::subclass::Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    // Emitted when Save is pressed with (note_text)
                    glib::subclass::Signal::builder("save-requested")
                        .param_types([String::static_type()])
                        .build(),
                    // Emitted when Cancel/Escape is pressed
                    glib::subclass::Signal::builder("cancel-requested").build(),
                    // Emitted when Delete is pressed with (annotation_id)
                    glib::subclass::Signal::builder("delete-requested")
                        .param_types([i64::static_type()])
                        .build(),
                ]
            })
        }
    }

    impl WidgetImpl for AnnotationPanel {}
    impl BoxImpl for AnnotationPanel {}
}

glib::wrapper! {
    pub struct AnnotationPanel(ObjectSubclass<imp::AnnotationPanel>)
        @extends Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl AnnotationPanel {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    fn setup_widgets(&self) {
        let imp = self.imp();

        self.set_orientation(Orientation::Vertical);
        self.set_spacing(0);

        // Resize handle at top
        imp.resize_handle.set_margin_bottom(8);
        imp.resize_handle.add_css_class("spacer");
        self.append(&imp.resize_handle);

        // Main content area
        let content_box = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .margin_start(12)
            .margin_end(12)
            .margin_bottom(12)
            .vexpand(true)
            .build();

        // Header row with selected text preview
        let header_box = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .build();

        let annotation_label = Label::new(Some("Annotation for:"));
        annotation_label.add_css_class("dim-label");
        header_box.append(&annotation_label);

        imp.selected_text_label
            .set_ellipsize(gtk::pango::EllipsizeMode::End);
        imp.selected_text_label.set_max_width_chars(60);
        imp.selected_text_label.set_hexpand(true);
        imp.selected_text_label.set_xalign(0.0);
        imp.selected_text_label
            .add_css_class("annotation-selected-text");
        header_box.append(&imp.selected_text_label);

        content_box.append(&header_box);

        // Text input area
        imp.text_view.set_wrap_mode(gtk::WrapMode::Word);
        imp.text_view.set_accepts_tab(false);
        imp.text_view.add_css_class("annotation-input");

        imp.scrolled_window.set_child(Some(&imp.text_view));
        imp.scrolled_window.set_min_content_height(60);
        imp.scrolled_window.set_vexpand(true);
        imp.scrolled_window.add_css_class("annotation-scroll");
        content_box.append(&imp.scrolled_window);

        // Button row
        let button_box = Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .halign(gtk::Align::End)
            .build();

        // Delete button (left-aligned, only visible when editing)
        imp.delete_button.set_label("Delete");
        imp.delete_button.add_css_class("destructive-action");
        imp.delete_button.set_visible(false);
        imp.delete_button.set_halign(gtk::Align::Start);
        imp.delete_button.set_hexpand(true);

        // Cancel button
        imp.cancel_button.set_label("Cancel");

        // Save button
        imp.save_button.set_label("Save");
        imp.save_button.add_css_class("suggested-action");

        button_box.append(&imp.delete_button);
        button_box.append(&imp.cancel_button);
        button_box.append(&imp.save_button);
        content_box.append(&button_box);

        self.append(&content_box);

        // Set initial size
        self.set_size_request(-1, DEFAULT_PANEL_HEIGHT);

        // Apply styling
        self.add_css_class("annotation-panel");

        // Connect button signals
        self.setup_button_signals();
        self.setup_keyboard_handling();
    }

    fn setup_button_signals(&self) {
        let imp = self.imp();

        // Save button
        let panel_weak = self.downgrade();
        imp.save_button.connect_clicked(move |_| {
            if let Some(panel) = panel_weak.upgrade() {
                panel.emit_save();
            }
        });

        // Cancel button
        let panel_weak = self.downgrade();
        imp.cancel_button.connect_clicked(move |_| {
            if let Some(panel) = panel_weak.upgrade() {
                panel.emit_by_name::<()>("cancel-requested", &[]);
            }
        });

        // Delete button
        let panel_weak = self.downgrade();
        imp.delete_button.connect_clicked(move |_| {
            if let Some(panel) = panel_weak.upgrade() {
                if let Some(id) = panel.imp().annotation_id.get() {
                    panel.emit_by_name::<()>("delete-requested", &[&id]);
                }
            }
        });
    }

    fn setup_keyboard_handling(&self) {
        let imp = self.imp();

        let controller = gtk::EventControllerKey::new();
        let panel_weak = self.downgrade();

        controller.connect_key_pressed(move |_, key, _, modifiers| {
            if let Some(panel) = panel_weak.upgrade() {
                // Escape to cancel
                if key == gtk::gdk::Key::Escape {
                    panel.emit_by_name::<()>("cancel-requested", &[]);
                    return glib::Propagation::Stop;
                }

                // Ctrl+Enter to save (Enter alone creates newlines)
                if key == gtk::gdk::Key::Return
                    && modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK)
                {
                    panel.emit_save();
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });

        imp.text_view.add_controller(controller);
    }

    fn emit_save(&self) {
        let buffer = self.imp().text_view.buffer();
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        self.emit_by_name::<()>("save-requested", &[&text]);
    }

    /// Set the selected text preview
    pub fn set_selected_text(&self, text: &str) {
        // Truncate and clean up for display
        let display_text = text
            .chars()
            .take(100)
            .collect::<String>()
            .replace('\n', " ");
        let display_text = if text.len() > 100 {
            format!("\"{}...\"", display_text)
        } else {
            format!("\"{}\"", display_text)
        };
        self.imp().selected_text_label.set_text(&display_text);
    }

    /// Set the note text in the editor
    pub fn set_note(&self, text: &str) {
        self.imp().text_view.buffer().set_text(text);
    }

    /// Get the current note text
    pub fn note(&self) -> String {
        let buffer = self.imp().text_view.buffer();
        buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string()
    }

    /// Set the annotation ID (for editing mode)
    pub fn set_annotation_id(&self, id: Option<i64>) {
        let imp = self.imp();
        imp.annotation_id.set(id);
        imp.delete_button.set_visible(id.is_some());
    }

    /// Get the annotation ID
    pub fn annotation_id(&self) -> Option<i64> {
        self.imp().annotation_id.get()
    }

    /// Clear the panel and reset to initial state
    pub fn clear(&self) {
        let imp = self.imp();
        imp.selected_text_label.set_text("");
        imp.text_view.buffer().set_text("");
        imp.annotation_id.set(None);
        imp.delete_button.set_visible(false);
    }

    /// Focus the text input
    pub fn focus_input(&self) {
        self.imp().text_view.grab_focus();
    }

    pub fn save_button(&self) -> &Button {
        &self.imp().save_button
    }

    pub fn cancel_button(&self) -> &Button {
        &self.imp().cancel_button
    }

    pub fn delete_button(&self) -> &Button {
        &self.imp().delete_button
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

impl Default for AnnotationPanel {
    fn default() -> Self {
        Self::new()
    }
}
