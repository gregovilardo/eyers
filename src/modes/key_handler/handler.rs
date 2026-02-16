use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::input_state::InputState;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct KeyHandler {
        /// Current input state (pending sequences)
        pub(super) input_state: RefCell<InputState>,
        /// Accumulated count for commands (e.g., 42G)
        pub(super) pending_count: Cell<Option<u32>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for KeyHandler {
        const NAME: &'static str = "EyersKeyHandler";
        type Type = super::KeyHandler;
        type ParentType = glib::Object;
    }

    impl ObjectImpl for KeyHandler {
        fn properties() -> &'static [glib::ParamSpec] {
            use std::sync::OnceLock;
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                vec![glib::ParamSpecString::builder("status-text")
                    .nick("Status Text")
                    .blurb("Text to display in the status bar showing pending input")
                    .read_only()
                    .build()]
            })
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "status-text" => self.obj().status_text().to_value(),
                _ => unimplemented!(),
            }
        }
    }
}

glib::wrapper! {
    /// KeyHandler manages the input state machine for keyboard shortcuts.
    ///
    /// It tracks:
    /// - Multi-key sequences (g, f, F, n prefixes)
    /// - Accumulated counts (42G, 5j, etc.)
    ///
    /// The `status-text` property can be bound to a UI element to show
    /// the current pending input state to the user.
    pub struct KeyHandler(ObjectSubclass<imp::KeyHandler>);
}

impl Default for KeyHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyHandler {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    /// Get the current input state
    pub fn input_state(&self) -> InputState {
        self.imp().input_state.borrow().clone()
    }

    /// Set the input state and notify property change
    pub fn set_input_state(&self, state: InputState) {
        let changed = {
            let mut current = self.imp().input_state.borrow_mut();
            if *current != state {
                *current = state;
                true
            } else {
                false
            }
        };
        if changed {
            self.notify("status-text");
        }
    }

    /// Get the pending count (defaults to 1 if not set)
    pub fn count(&self) -> u32 {
        self.imp().pending_count.get().unwrap_or(1)
    }

    /// Get the raw pending count (None if not accumulating)
    pub fn pending_count(&self) -> Option<u32> {
        self.imp().pending_count.get()
    }

    /// Set the pending count
    pub fn set_pending_count(&self, count: Option<u32>) {
        let old = self.imp().pending_count.get();
        if old != count {
            self.imp().pending_count.set(count);
            self.notify("status-text");
        }
    }

    /// Accumulate a digit into the pending count
    pub fn accumulate_digit(&self, digit: u32) {
        let new_count = match self.imp().pending_count.get() {
            Some(current) => current
                .checked_mul(10)
                .and_then(|v| v.checked_add(digit))
                .unwrap_or(current), // Keep original on overflow
            None => digit,
        };
        self.set_pending_count(Some(new_count));
    }

    /// Reset all state (input state and count)
    pub fn reset(&self) {
        self.set_input_state(InputState::Ready);
        self.set_pending_count(None);
    }

    /// Reset only the input state, keeping the count for certain operations
    pub fn reset_input_state(&self) {
        self.set_input_state(InputState::Ready);
    }

    /// Reset the count but keep the input state
    pub fn reset_count(&self) {
        self.set_pending_count(None);
    }

    /// Check if there's any pending state
    pub fn has_pending_state(&self) -> bool {
        self.imp().input_state.borrow().is_pending() || self.imp().pending_count.get().is_some()
    }

    /// Get the status text for display in the UI
    /// Returns something like "42g" when count is 42 and waiting for second g
    pub fn status_text(&self) -> String {
        let count_str = self
            .imp()
            .pending_count
            .get()
            .map(|c| c.to_string())
            .unwrap_or_default();

        let state_str = self.imp().input_state.borrow().display_suffix();

        format!("{}{}", count_str, state_str)
    }
}
