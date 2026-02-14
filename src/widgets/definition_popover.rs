use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{Box, Button, Label, Orientation, PolicyType, Popover, ScrolledWindow};
use std::cell::RefCell;

use crate::services::dictionary;
use crate::services::dictionary::Language;

const POPOVER_WIDTH: i32 = 500;
const POPOVER_HEIGHT: i32 = 200;
const DEFINITION_POLL_MS: u64 = 500;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct DefinitionPopover {
        pub label: RefCell<Option<Label>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DefinitionPopover {
        const NAME: &'static str = "DefinitionPopover";
        type Type = super::DefinitionPopover;
        type ParentType = Popover;
    }

    impl ObjectImpl for DefinitionPopover {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_widgets();
        }
    }

    impl WidgetImpl for DefinitionPopover {}
    impl PopoverImpl for DefinitionPopover {}
}

glib::wrapper! {
    pub struct DefinitionPopover(ObjectSubclass<imp::DefinitionPopover>)
        @extends Popover, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::ShortcutManager;
}

impl DefinitionPopover {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    fn setup_widgets(&self) {
        self.set_has_arrow(true);
        self.set_autohide(false);
        self.set_position(gtk::PositionType::Bottom);

        let label = Label::builder()
            .label("Loading definition...")
            .wrap(true)
            .xalign(0.0)
            .yalign(0.0)
            .selectable(true)
            .build();

        let scroller = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .vexpand_set(true)
            .min_content_width(POPOVER_WIDTH)
            .min_content_height(POPOVER_HEIGHT)
            .child(&label)
            .build();

        let close_button = self.create_close_button();

        let container = Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(4)
            .margin_start(8)
            .margin_end(8)
            .margin_top(8)
            .margin_bottom(8)
            .build();

        container.append(&scroller);
        container.append(&close_button);

        self.set_child(Some(&container));
        self.set_size_request(POPOVER_WIDTH, POPOVER_HEIGHT);

        self.imp().label.replace(Some(label));
    }

    fn create_close_button(&self) -> Button {
        let button = Button::builder().label("Close").margin_top(8).build();
        let popover_weak = self.downgrade();

        button.connect_clicked(move |_| {
            if let Some(popover) = popover_weak.upgrade() {
                popover.popdown();
            }
        });

        button
    }

    pub fn show_at(&self, parent: &impl IsA<gtk::Widget>, x: f64, y: f64) {
        self.set_parent(parent.as_ref());
        self.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        self.popup();
    }

    pub fn fetch_and_display(&self, original_word: String, lookup_word: String, lang: Language) {
        let (sender, receiver) = std::sync::mpsc::channel::<String>();

        std::thread::spawn(move || {
            let definition = dictionary::fetch_definition(&lookup_word, &original_word, lang)
                .unwrap_or_else(|| {
                    format!("Definition for <b>{lookup_word}</b> not found.").to_string()
                });
            let _ = sender.send(definition);
        });

        let label_weak = self.imp().label.borrow().as_ref().map(|l| l.downgrade());

        if let Some(label_weak) = label_weak {
            glib::timeout_add_local(
                std::time::Duration::from_millis(DEFINITION_POLL_MS),
                move || {
                    if let Ok(definition) = receiver.try_recv() {
                        if let Some(label) = label_weak.upgrade() {
                            label.set_markup(&definition);
                        }
                        return glib::ControlFlow::Break;
                    }
                    glib::ControlFlow::Continue
                },
            );
        }
    }
}

impl Default for DefinitionPopover {
    fn default() -> Self {
        Self::new()
    }
}
