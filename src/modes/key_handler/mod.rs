mod handler;
mod input_state;
mod key_action;
mod processing;

pub use handler::KeyHandler;
pub use key_action::{KeyAction, ScrollDir};
pub use processing::{
    KeyResult, handle_normal_mode_key, handle_post_global_key, handle_pre_global_key,
    handle_toc_key, handle_visual_mode_key,
};
