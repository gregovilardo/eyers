/// Represents the current input state of the key handler.
/// This is the internal state machine for multi-key sequences.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum InputState {
    /// Ready to receive new input
    #[default]
    Ready,
    /// Waiting for second 'g' (gg to go to start, or number+g to go to page)
    PendingG,
    /// Waiting for a character to find forward (f + char)
    PendingFForward,
    /// Waiting for a character to find backward (F + char)
    PendingFBackward,
    /// Waiting for an element to search it forward (]+a for annotations for example)
    PendingElementForward,
    /// Waiting for an element to search it backward ([+a for annotations for example)
    PendingElementBackward,
}

impl InputState {
    /// Returns true if this state is waiting for additional input
    pub fn is_pending(&self) -> bool {
        !matches!(self, InputState::Ready)
    }

    /// Get a display string for the current state (for status bar)
    pub fn display_suffix(&self) -> &'static str {
        match self {
            InputState::Ready => "",
            InputState::PendingG => "g",
            InputState::PendingFForward => "f",
            InputState::PendingFBackward => "F",
            InputState::PendingElementForward => "]",
            InputState::PendingElementBackward => "[",
        }
    }
}
