//! Cross-platform mouse, keyboard (and gamepads soon) module.

use std::collections::HashSet;

use crate::get_context;
use crate::prelude::screen_height;
use crate::prelude::screen_width;
use crate::Vec2;
pub use miniquad::{KeyCode, MouseButton};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TouchPhase {
    Started,
    Stationary,
    Moved,
    Ended,
    Cancelled,
}

impl From<miniquad::TouchPhase> for TouchPhase {
    fn from(miniquad_phase: miniquad::TouchPhase) -> TouchPhase {
        match miniquad_phase {
            miniquad::TouchPhase::Started => TouchPhase::Started,
            miniquad::TouchPhase::Moved => TouchPhase::Moved,
            miniquad::TouchPhase::Ended => TouchPhase::Ended,
            miniquad::TouchPhase::Cancelled => TouchPhase::Cancelled,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Touch {
    pub id: u64,
    pub phase: TouchPhase,
    pub position: Vec2,
}

/// This is set to false by default, meaning mouse events will raise touch events in addition to raising mouse events.
/// If set to false, mouse events won't affect touches.
pub fn is_simulating_touch_with_mouse() -> bool {
    get_context().simulate_touch_with_mouse
}

/// This is set to false by default, meaning mouse events will raise touch events in addition to raising mouse events.
/// If set to false, mouse events won't affect touches.
pub fn simulate_touch_with_mouse(option: bool) {
    get_context().simulate_touch_with_mouse = option;
}

/// Return touches with positions in pixels.
pub fn touches() -> Vec<Touch> {
    get_context().touches.values().cloned().collect()
}

/// Return touches with positions in range [-1; 1].
pub fn touches_local() -> Vec<Touch> {
    get_context()
        .touches
        .values()
        .map(|touch| {
            let mut touch = touch.clone();
            touch.position = convert_to_local(touch.position);
            touch
        })
        .collect()
}

pub fn mouse_wheel() -> (f32, f32) {
    let context = get_context();

    (context.mouse_wheel.x, context.mouse_wheel.y)
}

/// Detect if the key has been pressed once
pub fn is_key_pressed(key_code: KeyCode) -> bool {
    let context = get_context();

    context.keys_pressed.contains(&key_code)
}

/// Detect if the key is being pressed
pub fn is_key_down(key_code: KeyCode) -> bool {
    let context = get_context();

    context.keys_down.contains(&key_code)
}

/// Detect if the key has been released this frame
pub fn is_key_released(key_code: KeyCode) -> bool {
    let context = get_context();

    context.keys_released.contains(&key_code)
}

/// Return the last pressed char.
/// Each "get_char_pressed" call will consume a character from the input queue.
pub fn get_char_pressed() -> Option<char> {
    let context = get_context();

    context.chars_pressed_queue.pop()
}

pub(crate) fn get_char_pressed_ui() -> Option<char> {
    let context = get_context();

    context.chars_pressed_ui_queue.pop()
}

/// Return the last pressed key.
pub fn get_last_key_pressed() -> Option<KeyCode> {
    let context = get_context();
    // TODO: this will return a random key from keys_pressed HashMap instead of the last one, fix me later
    context.keys_pressed.iter().next().cloned()
}

pub fn get_keys_pressed() -> HashSet<KeyCode> {
    let context = get_context();
    context.keys_pressed.clone()
}

pub fn get_keys_down() -> HashSet<KeyCode> {
    let context = get_context();
    context.keys_down.clone()
}

pub fn get_keys_released() -> HashSet<KeyCode> {
    let context = get_context();
    context.keys_released.clone()
}

/// Clears input queue
pub fn clear_input_queue() {
    let context = get_context();
    context.chars_pressed_queue.clear();
    context.chars_pressed_ui_queue.clear();
}

/// Convert a position in pixels to a position in the range [-1; 1].
fn convert_to_local(pixel_pos: Vec2) -> Vec2 {
    Vec2::new(pixel_pos.x / screen_width(), pixel_pos.y / screen_height()) * 2.0 - Vec2::new(1.0, 1.0)
}

/// Prevents quit
pub fn prevent_quit() {
    get_context().prevent_quit_event = true;
}

/// Detect if quit has been requested
pub fn is_quit_requested() -> bool {
    get_context().quit_requested
}
