//! Window and associated to window rendering context related functions.

use crate::{get_context, get_quad_context};

use crate::color::Color;

// miniquad is re-exported for the use in combination with `get_internal_gl`
pub use miniquad;

pub use miniquad::conf::Conf;

/// Block execution until the next frame.
#[must_use = "use `next_frame().await` to advance to the next frame"]
pub fn next_frame() -> crate::exec::FrameFuture {
    crate::thread_assert::same_thread();
    crate::exec::FrameFuture::default()
}

/// Fill window background with solid color.
/// Note: even when "clear_background" is not called explicitly,
/// the screen will be cleared at the beginning of the frame.
pub fn clear_background(color: Color) {
    let context = get_context();

    context.gl.clear(get_quad_context(), color);
}

#[doc(hidden)]
pub fn gl_set_drawcall_buffer_capacity(max_vertices: usize, max_indices: usize) {
    let context = get_context();
    context.gl.update_drawcall_capacity(get_quad_context(), max_vertices, max_indices);
}

pub struct InternalGlContext<'a> {
    pub quad_context: &'a mut dyn miniquad::RenderingBackend,
    pub quad_gl: &'a mut crate::quad_gl::QuadGl,
}

impl<'a> InternalGlContext<'a> {
    /// Draw all the batched stuff and reset the internal state cache.
    /// May be helpful for combining macroquad's drawing with raw miniquad/opengl calls.
    pub fn flush(&mut self) {
        get_context().perform_render_passes();
    }
}

pub unsafe fn get_internal_gl<'a>() -> InternalGlContext<'a> {
    let context = get_context();

    InternalGlContext {
        quad_context: get_quad_context(),
        quad_gl: &mut context.gl,
    }
}

pub fn screen_width() -> f32 {
    let context = get_context();
    context.screen_width / miniquad::window::dpi_scale()
}

pub fn screen_height() -> f32 {
    let context = get_context();

    context.screen_height / miniquad::window::dpi_scale()
}

pub fn screen_dpi_scale() -> f32 {
    miniquad::window::dpi_scale()
}

/// Request the window size to be the given value. This takes DPI into account.
///
/// Note that the OS might decide to give a different size. Additionally, the size in macroquad won't be updated until the next `next_frame().await`.
pub fn request_new_screen_size(width: f32, height: f32) {
    miniquad::window::set_window_size(
        (width * miniquad::window::dpi_scale()) as u32,
        (height * miniquad::window::dpi_scale()) as u32,
    );
    // We do not set the context.screen_width and context.screen_height here.
    // After `set_window_size` is called, EventHandlerFree::resize will be invoked, setting the size correctly.
    // Because the OS might decide to give a different screen dimension, setting the context.screen_* here would be confusing.
}

/// Toggle whether the window is fullscreen.
pub fn set_fullscreen(fullscreen: bool) {
    miniquad::window::set_fullscreen(fullscreen);
}
