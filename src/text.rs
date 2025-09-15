//! Functions to load fonts and draw text.

use crate::{
    color::Color,
    get_context, get_quad_context,
    math::{vec3, Rect},
    texture::{Image, TextureHandle},
    Error,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::color::WHITE;
use glam::vec2;

use std::sync::{Arc, Mutex};
pub(crate) mod atlas;

use atlas::{Atlas, SpriteKey};

#[derive(Debug, Clone)]
pub(crate) struct CharacterInfo {
    pub offset_x: i32,
    pub offset_y: i32,
    pub advance: f32,
    pub sprite: SpriteKey,
}

/// TTF font loaded to GPU
#[derive(Clone)]
pub struct Font {
    font: Rc<fontdue::Font>,
    atlas: Rc<RefCell<Atlas>>,
    characters: Rc<RefCell<Vec<Option<CharacterInfo>>>>,
}

/// World space dimensions of the text, measured by "measure_text" function
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TextDimensions {
    /// Distance from very left to very right of the rasterized text
    pub width: f32,
    /// Distance from the bottom to the top of the text.
    pub height: f32,
    /// Height offset from the baseline of the text.
    /// "draw_text(.., X, Y, ..)" will be rendered in a "Rect::new(X, Y - dimensions.offset_y, dimensions.width, dimensions.height)"
    /// For reference check "text_measures" example.
    pub offset_y: f32,
    /// Width and height of each individual line in the text. Each `Vec2` stores (unscaled_width, unscaled_layout_height).
    pub line_widths: Vec<glam::Vec2>,
}

impl std::fmt::Debug for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Font").field("font", &"fontdue::Font").finish()
    }
}

impl Font {
    const MAX_ASCII: usize = 256;
    const MAX_FONT_SIZE: usize = 64;

    pub(crate) fn load_from_bytes(atlas: Rc<RefCell<Atlas>>, bytes: &[u8]) -> Result<Font, Error> {
        assert_eq!(std::mem::needs_drop::<(char, f32, CharacterInfo)>(), false, "(Internal error) (char, f32, CharacterInfo) should not need drop because we avoid .clear and use .set_len(0) which will leak memory if this type requires dropping");

        Ok(Font {
            font: Rc::new(fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())?),
            characters: Rc::new(RefCell::new(vec![None; Self::MAX_ASCII * Self::MAX_FONT_SIZE])),
            atlas,
        })
    }

    pub(crate) fn set_atlas(&mut self, atlas: Rc<RefCell<Atlas>>) {
        self.atlas = atlas;
    }

    pub(crate) fn set_characters(&mut self, characters: Rc<RefCell<Vec<Option<CharacterInfo>>>>) {
        self.characters = characters;
    }

    pub(crate) fn ascent(&self, font_size: f32) -> f32 {
        self.font.horizontal_line_metrics(font_size).unwrap().ascent
    }

    pub(crate) fn descent(&self, font_size: f32) -> f32 {
        self.font.horizontal_line_metrics(font_size).unwrap().descent
    }

    #[inline(always)]
    pub fn char_index(c: char, font_size: u16) -> usize {
        let mut c = c;
        if !c.is_ascii() {
            c = '?'; // Only support ascii for now
        }

        (c as u8) as usize * Self::MAX_FONT_SIZE + font_size as usize
    }

    pub(crate) fn cache_glyph(&self, character: char, size: u16) {
        if self.contains(character, size) {
            return;
        }

        let (metrics, bitmap) = self.font.rasterize(character, size as f32);

        let (width, height) = (metrics.width as u16, metrics.height as u16);

        let sprite = self.atlas.borrow_mut().new_unique_id();
        self.atlas.borrow_mut().cache_sprite(
            sprite,
            Image {
                bytes: bitmap.iter().flat_map(|coverage| vec![255, 255, 255, *coverage]).collect(),
                width,
                height,
            },
        );
        let advance = metrics.advance_width;

        let (offset_x, offset_y) = (metrics.xmin, metrics.ymin);

        let character_info = CharacterInfo {
            advance,
            offset_x,
            offset_y,
            sprite,
        };

        self.characters.borrow_mut()[Self::char_index(character, size)] = Some(character_info);
    }

    pub(crate) fn get(&self, character: char, size: u16) -> Option<CharacterInfo> {
        unsafe { self.characters.borrow().get_unchecked(Self::char_index(character, size)).clone() }
    }

    /// Returns whether the character has been cached
    pub(crate) fn contains(&self, character: char, size: u16) -> bool {
        unsafe { self.characters.borrow().get_unchecked(Self::char_index(character, size)).is_some() }
    }

    pub(crate) fn measure_text(
        &self,
        text: impl AsRef<str>,
        font_size_unscaled: u16,
        font_scale_x: f32,
        font_scale_y: f32,
        max_line_width_unscaled: Option<f32>,
    ) -> TextDimensions {
        unsafe {
            let text = text.as_ref();

            if text.is_empty() {
                return TextDimensions::default();
            }

            let dpi_scaling = miniquad::window::dpi_scale();
            let font_size_for_caching = (font_size_unscaled as f32 * dpi_scaling).ceil() as u16;
            let max_line_width_pixels = max_line_width_unscaled.map(|w| w * dpi_scaling);

            let unique_characters_from_text: std::collections::HashSet<char> = text.chars().collect();
            for character in unique_characters_from_text.iter() {
                if !self.contains(*character, font_size_for_caching) {
                    self.cache_glyph(*character, font_size_for_caching);
                }
            }

            let font_characters = self.characters.borrow();
            let atlas = self.atlas.borrow();

            let new_line_padding = 2.0;
            let mut layout_line_height_scaled: f32 = 0.0;
            if !text.is_empty() {
                for character in unique_characters_from_text.iter() {
                    if let Some(char_data) = font_characters.get_unchecked(Font::char_index(*character, font_size_for_caching)) {
                        if let Some(glyph_data) = atlas.get(char_data.sprite) {
                            layout_line_height_scaled = layout_line_height_scaled.max(glyph_data.rect.h * font_scale_y);
                        }
                    }
                }
                if layout_line_height_scaled == 0.0 {
                    layout_line_height_scaled = font_size_unscaled as f32 * font_scale_y * dpi_scaling;
                }
                if layout_line_height_scaled <= 0.0 {
                    layout_line_height_scaled = font_size_unscaled as f32 * dpi_scaling;
                    if layout_line_height_scaled <= 0.0 {
                        layout_line_height_scaled = 1.0 * dpi_scaling;
                    }
                }
                layout_line_height_scaled += new_line_padding;
            }
            let unscaled_layout_line_h = if layout_line_height_scaled <= 0.0 {
                0.0
            } else {
                layout_line_height_scaled / dpi_scaling
            };

            let mut current_line_scaled_width: f32 = 0.0;
            let mut max_line_width_used_scaled: f32 = 0.0;
            let mut measured_lines_unscaled = Vec::new();
            let mut overall_max_y_offset_scaled: f32 = f32::MIN;

            let mut current_word_width_scaled: f32 = 0.0;
            let mut word_buffer = Vec::<(char, f32, CharacterInfo)>::with_capacity(32);

            // Track characters added to current line for trimming trailing whitespace
            let mut current_line_chars = Vec::<(char, f32)>::with_capacity(64);

            let chars: Vec<char> = text.chars().collect();
            let length = chars.len();
            let mut i = 0;

            // Helper function to trim trailing whitespace from current line
            let mut trim_trailing_whitespace = |line_width: &mut f32, line_chars: &mut Vec<(char, f32)>| -> f32 {
                let mut trimmed_width = 0.0;
                while let Some(&(c, advance)) = line_chars.last() {
                    if c == ' ' || c == '\t' {
                        *line_width -= advance;
                        trimmed_width += advance;
                        line_chars.pop();
                    } else {
                        break;
                    }
                }
                trimmed_width
            };

            while i < length {
                let mut c = chars[i];

                // Handle markup exactly like draw_text_ex (default: enabled)
                if c == '[' {
                    let (action, next_pos) = parse_markup(&chars, i);
                    match action {
                        MarkupResult::Noop => { /* treat as '[' literal below */ }
                        MarkupResult::Literal(char_literal) => {
                            c = char_literal; // continue with this literal char
                        }
                        MarkupResult::Push(_) | MarkupResult::Pop => {
                            // Flush current word buffer to the line, like draw_text_ex does
                            for (_c2, adv, char_info) in word_buffer.drain(..) {
                                if let Some(glyph_data) = atlas.get(char_info.sprite) {
                                    let char_offset_y_s = char_info.offset_y as f32 * font_scale_y;
                                    let char_visual_max_y_s = glyph_data.rect.h * font_scale_y + char_offset_y_s;
                                    overall_max_y_offset_scaled = overall_max_y_offset_scaled.max(char_visual_max_y_s);
                                }
                                current_line_scaled_width += adv;
                                current_line_chars.push((_c2, adv));
                            }
                            current_word_width_scaled = 0.0;

                            // Do not add a line here; just switch styles. Skip tag text.
                            i = next_pos;
                            continue;
                        }
                    }
                }

                if c == '\n' {
                    // Flush buffered word and push a line ALWAYS (even if empty), matching draw_text_ex
                    for (_c2, adv, char_info) in word_buffer.drain(..) {
                        if let Some(glyph_data) = atlas.get(char_info.sprite) {
                            let char_offset_y_s = char_info.offset_y as f32 * font_scale_y;
                            let char_visual_max_y_s = glyph_data.rect.h * font_scale_y + char_offset_y_s;
                            overall_max_y_offset_scaled = overall_max_y_offset_scaled.max(char_visual_max_y_s);
                        }
                        current_line_scaled_width += adv;
                        current_line_chars.push((_c2, adv));
                    }
                    current_word_width_scaled = 0.0;

                    max_line_width_used_scaled = max_line_width_used_scaled.max(current_line_scaled_width);
                    measured_lines_unscaled.push(glam::vec2(current_line_scaled_width / dpi_scaling, unscaled_layout_line_h));

                    current_line_scaled_width = 0.0;
                    current_line_chars.clear();
                    i += 1;
                    continue;
                }

                if let Some(char_data) = font_characters.get_unchecked(Font::char_index(c, font_size_for_caching)).clone() {
                    let advance_scaled = char_data.advance * font_scale_x;
                    if let Some(glyph_data) = atlas.get(char_data.sprite) {
                        let char_offset_y_s = char_data.offset_y as f32 * font_scale_y;
                        let char_visual_max_y_s = glyph_data.rect.h * font_scale_y + char_offset_y_s;
                        overall_max_y_offset_scaled = overall_max_y_offset_scaled.max(char_visual_max_y_s);
                    }

                    if c == ' ' || c == '\t' || c == '-' {
                        // Flush current buffered word into the line width first
                        for (_c2, adv, char_info) in word_buffer.drain(..) {
                            if let Some(glyph_data) = atlas.get(char_info.sprite) {
                                let char_offset_y_s = char_info.offset_y as f32 * font_scale_y;
                                let char_visual_max_y_s = glyph_data.rect.h * font_scale_y + char_offset_y_s;
                                overall_max_y_offset_scaled = overall_max_y_offset_scaled.max(char_visual_max_y_s);
                            }
                            current_line_scaled_width += adv;
                            current_line_chars.push((_c2, adv));
                        }
                        current_word_width_scaled = 0.0;

                        if let Some(max_w_pixels) = max_line_width_pixels {
                            if current_line_scaled_width + advance_scaled > max_w_pixels && current_line_scaled_width > 0.0 {
                                // Current line is full, start new line - trim trailing whitespace first
                                trim_trailing_whitespace(&mut current_line_scaled_width, &mut current_line_chars);

                                max_line_width_used_scaled = max_line_width_used_scaled.max(current_line_scaled_width);
                                measured_lines_unscaled.push(glam::vec2(current_line_scaled_width / dpi_scaling, unscaled_layout_line_h));
                                current_line_scaled_width = 0.0;
                                current_line_chars.clear();

                                // Skip leading space/tab on new line, but NOT '-'
                                if c == ' ' || c == '\t' {
                                    i += 1;
                                    continue;
                                }
                            }
                        }

                        // Add the breaking char to the current line
                        current_line_scaled_width += advance_scaled;
                        current_line_chars.push((c, advance_scaled));
                    } else {
                        // Non-breaking character, check if we need to wrap the whole word
                        if let Some(max_w_pixels) = max_line_width_pixels {
                            if current_line_scaled_width + current_word_width_scaled + advance_scaled > max_w_pixels {
                                if current_line_scaled_width > 0.0 {
                                    // Move entire word to next line - trim trailing whitespace first
                                    trim_trailing_whitespace(&mut current_line_scaled_width, &mut current_line_chars);

                                    max_line_width_used_scaled = max_line_width_used_scaled.max(current_line_scaled_width);
                                    measured_lines_unscaled
                                        .push(glam::vec2(current_line_scaled_width / dpi_scaling, unscaled_layout_line_h));

                                    // Reset line width and flush word buffer to new line
                                    current_line_scaled_width = 0.0;
                                    current_line_chars.clear();
                                    for (_wc, w_adv, w_char_info) in word_buffer.drain(..) {
                                        if let Some(glyph_data) = atlas.get(w_char_info.sprite) {
                                            let char_offset_y_s = w_char_info.offset_y as f32 * font_scale_y;
                                            let char_visual_max_y_s = glyph_data.rect.h * font_scale_y + char_offset_y_s;
                                            overall_max_y_offset_scaled = overall_max_y_offset_scaled.max(char_visual_max_y_s);
                                        }
                                        current_line_scaled_width += w_adv;
                                        current_line_chars.push((_wc, w_adv));
                                    }
                                    current_word_width_scaled = 0.0;
                                } else {
                                    // Word is too long for empty line, break it character by character
                                    for (_wc, w_adv, w_char_info) in word_buffer.drain(..) {
                                        if current_line_scaled_width + w_adv > max_w_pixels && current_line_scaled_width > 0.0 {
                                            // Trim trailing whitespace before wrapping
                                            trim_trailing_whitespace(&mut current_line_scaled_width, &mut current_line_chars);

                                            max_line_width_used_scaled = max_line_width_used_scaled.max(current_line_scaled_width);
                                            measured_lines_unscaled
                                                .push(glam::vec2(current_line_scaled_width / dpi_scaling, unscaled_layout_line_h));
                                            current_line_scaled_width = 0.0;
                                            current_line_chars.clear();
                                        }
                                        if let Some(glyph_data) = atlas.get(w_char_info.sprite) {
                                            let char_offset_y_s = w_char_info.offset_y as f32 * font_scale_y;
                                            let char_visual_max_y_s = glyph_data.rect.h * font_scale_y + char_offset_y_s;
                                            overall_max_y_offset_scaled = overall_max_y_offset_scaled.max(char_visual_max_y_s);
                                        }
                                        current_line_scaled_width += w_adv;
                                        current_line_chars.push((_wc, w_adv));
                                    }
                                    current_word_width_scaled = 0.0;

                                    if current_line_scaled_width + advance_scaled > max_w_pixels && current_line_scaled_width > 0.0 {
                                        // Trim trailing whitespace before wrapping
                                        trim_trailing_whitespace(&mut current_line_scaled_width, &mut current_line_chars);

                                        max_line_width_used_scaled = max_line_width_used_scaled.max(current_line_scaled_width);
                                        measured_lines_unscaled
                                            .push(glam::vec2(current_line_scaled_width / dpi_scaling, unscaled_layout_line_h));
                                        current_line_scaled_width = 0.0;
                                        current_line_chars.clear();
                                    }
                                }
                            }
                        }

                        // Add character to word buffer
                        word_buffer.push((c, advance_scaled, char_data.clone()));
                        current_word_width_scaled += advance_scaled;
                    }
                }

                i += 1;
            }

            // End: flush remaining word and push last line mirroring draw_text_ex
            for (_c2, adv, char_info) in word_buffer.drain(..) {
                if let Some(glyph_data) = atlas.get(char_info.sprite) {
                    let char_offset_y_s = char_info.offset_y as f32 * font_scale_y;
                    let char_visual_max_y_s = glyph_data.rect.h * font_scale_y + char_offset_y_s;
                    overall_max_y_offset_scaled = overall_max_y_offset_scaled.max(char_visual_max_y_s);
                }
                current_line_scaled_width += adv;
                current_line_chars.push((_c2, adv));
            }
            current_word_width_scaled = 0.0;

            // Trim trailing whitespace from the final line before measuring
            trim_trailing_whitespace(&mut current_line_scaled_width, &mut current_line_chars);

            max_line_width_used_scaled = max_line_width_used_scaled.max(current_line_scaled_width);
            if current_line_scaled_width > 0.0 || (measured_lines_unscaled.is_empty() && !text.is_empty()) {
                measured_lines_unscaled.push(glam::vec2(current_line_scaled_width / dpi_scaling, unscaled_layout_line_h));
            }

            let final_width_unscaled = if measured_lines_unscaled.is_empty() {
                0.0
            } else {
                max_line_width_used_scaled / dpi_scaling
            };

            let calculated_total_height_unscaled = if measured_lines_unscaled.is_empty() {
                0.0
            } else {
                (measured_lines_unscaled.len() as f32 * layout_line_height_scaled) / dpi_scaling
            };

            let final_offset_y_unscaled = if overall_max_y_offset_scaled == f32::MIN {
                0.0
            } else {
                overall_max_y_offset_scaled / dpi_scaling
            };

            TextDimensions {
                width: final_width_unscaled,
                height: calculated_total_height_unscaled,
                offset_y: final_offset_y_unscaled,
                line_widths: measured_lines_unscaled,
            }
        }
    }
}

impl Font {
    /// List of ascii characters, may be helpful in combination with "populate_font_cache"
    pub fn ascii_character_list() -> Vec<char> {
        (0..255).filter_map(::std::char::from_u32).collect()
    }

    /// List of latin characters
    pub fn latin_character_list() -> Vec<char> {
        "qwertyuiopasdfghjklzxcvbnmQWERTYUIOPASDFGHJKLZXCVBNM1234567890!@#$%^&*(){}[].,:"
            .chars()
            .collect()
    }

    pub fn populate_font_cache(&self, characters: &[char], size: u16) {
        for character in characters {
            self.cache_glyph(*character, size);
        }
    }

    /// Sets the [FilterMode](https://docs.rs/miniquad/latest/miniquad/graphics/enum.FilterMode.html#) of this font's texture atlas.
    ///
    /// Use Nearest if you need integer-ratio scaling for pixel art, for example.
    ///
    /// # Example
    /// ```
    /// # use macroquad::prelude::*;
    /// # #[macroquad::main("test")]
    /// # async fn main() {
    /// let mut font = get_default_font();
    /// font.set_filter(FilterMode::Linear);
    /// # }
    /// ```
    pub fn set_filter(&mut self, filter_mode: miniquad::FilterMode) {
        self.atlas.borrow_mut().set_filter(filter_mode);
    }

    // pub fn texture(&self) -> Texture2D {
    //     let font = get_context().fonts_storage.get_font(*self);

    //     font.font_texture
    // }
}

impl Default for Font {
    fn default() -> Self {
        get_default_font()
    }
}

/// Arguments for "draw_text_ex" function such as font, font_size etc
#[derive(Debug, Clone)]
pub struct TextParams<'a> {
    pub font: Option<&'a Font>,
    /// Base size for character height. The size in pixel used during font rasterizing.
    pub font_size: u16,
    /// The glyphs sizes actually drawn on the screen will be font_size * font_scale
    /// However with font_scale too different from 1.0 letters may be blurry
    pub font_scale: f32,
    /// Font X axis would be scaled by font_scale * font_scale_aspect
    /// and Y axis would be scaled by font_scale
    /// Default is 1.0
    pub font_scale_aspect: f32,
    /// Text rotation in radian
    /// Default is 0.0
    pub rotation: f32,
    pub color: Color,
    /// Enable text markup with [#RRGGBB] or [#RRGGBBAA] color tags
    /// Default is true
    pub enable_markup: bool,
    /// Maximum width of a line in pixels, text will wrap if it exceeds this width
    /// None means no wrapping
    /// Default is None
    pub max_line_width: Option<f32>,
}

impl<'a> Default for TextParams<'a> {
    fn default() -> TextParams<'a> {
        TextParams {
            font: None,
            font_size: 20,
            font_scale: 1.0,
            font_scale_aspect: 1.0,
            color: WHITE,
            rotation: 0.0,
            enable_markup: true,
            max_line_width: None,
        }
    }
}

/// Load font from file with "path"
pub async fn load_ttf_font(path: &str) -> Result<Font, Error> {
    let bytes = crate::file::load_file(path)
        .await
        .map_err(|_| Error::FontError("The Font file couldn't be loaded"))?;

    load_ttf_font_from_bytes(&bytes[..])
}

/// Load font from bytes array, may be use in combination with include_bytes!
/// ```ignore
/// let font = load_ttf_font_from_bytes(include_bytes!("font.ttf"));
/// ```
pub fn load_ttf_font_from_bytes(bytes: &[u8]) -> Result<Font, Error> {
    let atlas = Rc::new(RefCell::new(Atlas::new(get_quad_context(), miniquad::FilterMode::Linear)));

    let mut font = Font::load_from_bytes(atlas.clone(), bytes)?;

    font.populate_font_cache(&Font::ascii_character_list(), 15);

    let ctx = get_context();

    font.set_filter(ctx.default_filter_mode);

    Ok(font)
}

/// Draw text with given font_size
/// Returns text size
pub fn draw_text(text: impl AsRef<str>, x: f32, y: f32, font_size: f32, color: Color) {
    draw_text_ex(
        text,
        x,
        y,
        TextParams {
            font_size: font_size as u16,
            font_scale: 1.0,
            color,
            ..Default::default()
        },
    )
}

pub fn draw_text_ex(text: impl AsRef<str>, x: f32, y: f32, params: TextParams) {
    unsafe {
        let text = text.as_ref();

        if text.is_empty() {
            return;
        }

        let font = params.font.unwrap_or_else(|| &get_context().fonts_storage.default_font);

        let dpi_scaling = miniquad::window::dpi_scale();

        let rot = params.rotation;
        let font_scale_x = params.font_scale * params.font_scale_aspect;
        let font_scale_y = params.font_scale;
        let font_size = (params.font_size as f32 * dpi_scaling).ceil() as u16; // Scaled font size for glyph cache
        let max_line_width_pixels = params.max_line_width.map(|w| w * dpi_scaling).unwrap_or(-1.);

        let mut current_line_scaled_width: f32 = 0.0; // Tracks width of the current line being built (scaled)

        let mut max_offset_y_scaled: f32 = f32::MIN; // Scaled max offset from baseline
        let mut min_offset_y_scaled: f32 = f32::MAX; // Not directly used in TextDimensions, but calculated by render_character

        let mut current_word_width_scaled: f32 = 0.0;
        let mut word_buffer = Vec::<(char, f32, CharacterInfo)>::with_capacity(32);

        let rot_cos = rot.cos();
        let rot_sin = rot.sin();

        let chars: Vec<char> = text.chars().collect();
        for character in chars.iter() {
            if !font.contains(*character, font_size) {
                font.cache_glyph(*character, font_size);
            }
        }

        let font_characters = font.characters.borrow();
        let mut atlas = font.atlas.borrow_mut();

        let enable_markup = params.enable_markup;
        let original_color = params.color;
        let mut color = original_color;
        let mut color_stack = Vec::<Color>::with_capacity(4);

        let mut current_x = x; // Screen-space X for drawing current char
        let mut current_y = y; // Screen-space Y for drawing current char (baseline)
        let start_x = x;
        let start_y = y;

        let new_line_padding = 2.0; // Added to scaled line height components

        let mut layout_line_height_scaled: f32 = 0.0; // The uniform scaled height for advancing lines
        if !text.is_empty() {
            for character in chars.iter() {
                if let Some(char_data) = font_characters.get_unchecked(Font::char_index(*character, font_size)) {
                    if let Some(glyph_data) = atlas.get(char_data.sprite) {
                        // glyph_data.rect.h is physical pixels from rasterization at `font_size` (dpi-scaled size)
                        // font_scale_y is params.font_scale
                        layout_line_height_scaled = layout_line_height_scaled.max(glyph_data.rect.h * font_scale_y);
                    }
                }
            }
            if layout_line_height_scaled == 0.0 {
                // Fallback if no glyphs or zero height glyphs
                layout_line_height_scaled = params.font_size as f32 * font_scale_y * dpi_scaling;
            }
            if layout_line_height_scaled <= 0.0 {
                // Ensure positive
                layout_line_height_scaled = params.font_size as f32 * dpi_scaling;
                if layout_line_height_scaled <= 0.0 {
                    layout_line_height_scaled = 1.0 * dpi_scaling;
                }
            }
            layout_line_height_scaled += new_line_padding;
        }
        // If text was empty, layout_line_height_scaled remains 0.0, which is fine.

        let length = chars.len();
        let mut i = 0;

        while i < length {
            let mut c = chars[i];

            if c == '\n' {
                render_word(
                    &mut word_buffer,
                    &mut atlas,
                    &mut current_x,
                    &mut current_y,
                    &mut max_offset_y_scaled,
                    &mut min_offset_y_scaled,
                    rot,
                    rot_cos,
                    rot_sin,
                    font_scale_x,
                    font_scale_y,
                    dpi_scaling,
                    color,
                );
                current_line_scaled_width += current_word_width_scaled;
                word_buffer.set_len(0); // avoid .clear() to not drop contents since our types dont need dropping
                current_word_width_scaled = 0.0;

                current_x = start_x;
                current_y += layout_line_height_scaled;
                current_line_scaled_width = 0.0;
                i += 1;
                continue;
            }

            if c == '[' {
                let (action, next_pos) = parse_markup(&chars, i); // parse_markup needs to be in scope
                match action {
                    MarkupResult::Noop => {}
                    MarkupResult::Literal(char_literal) => {
                        c = char_literal;
                    }
                    MarkupResult::Push(new_color) => {
                        render_word(
                            &mut word_buffer,
                            &mut atlas,
                            &mut current_x,
                            &mut current_y,
                            &mut max_offset_y_scaled,
                            &mut min_offset_y_scaled,
                            rot,
                            rot_cos,
                            rot_sin,
                            font_scale_x,
                            font_scale_y,
                            dpi_scaling,
                            color,
                        );
                        current_line_scaled_width += current_word_width_scaled;
                        word_buffer.set_len(0); // avoid .clear() to not drop contents since our types dont need dropping
                        current_word_width_scaled = 0.0;

                        color_stack.push(color);

                        if enable_markup {
                            color = new_color;
                        }

                        i = next_pos;
                        continue;
                    }
                    MarkupResult::Pop => {
                        render_word(
                            &mut word_buffer,
                            &mut atlas,
                            &mut current_x,
                            &mut current_y,
                            &mut max_offset_y_scaled,
                            &mut min_offset_y_scaled,
                            rot,
                            rot_cos,
                            rot_sin,
                            font_scale_x,
                            font_scale_y,
                            dpi_scaling,
                            color,
                        );
                        current_line_scaled_width += current_word_width_scaled;
                        word_buffer.set_len(0); // avoid .clear() to not drop contents since our types dont need dropping
                        current_word_width_scaled = 0.0;

                        if enable_markup {
                            color = color_stack.pop().unwrap_or(original_color);
                        }

                        i = next_pos;
                        continue;
                    }
                }
                i = next_pos;
            }

            if let Some(char_data) = font_characters.get_unchecked(Font::char_index(c, font_size)) {
                // char_data.advance is from fontdue for rasterized size (font_size, which is dpi-scaled)
                // So, char_data.advance is in physical pixels for that rasterization.
                // advance_scaled is thus physical_pixels * font_scale_x.
                let advance_scaled = char_data.advance * font_scale_x;

                if c == ' ' || c == '\t' || c == '-' {
                    // Word-breaking characters
                    render_word(
                        &mut word_buffer,
                        &mut atlas,
                        &mut current_x,
                        &mut current_y,
                        &mut max_offset_y_scaled,
                        &mut min_offset_y_scaled,
                        rot,
                        rot_cos,
                        rot_sin,
                        font_scale_x,
                        font_scale_y,
                        dpi_scaling,
                        color,
                    );
                    current_line_scaled_width += current_word_width_scaled;
                    word_buffer.set_len(0); // avoid .clear() to not drop contents since our types dont need dropping
                    current_word_width_scaled = 0.0;

                    if max_line_width_pixels != -1.0 {
                        if current_line_scaled_width + advance_scaled > max_line_width_pixels && current_line_scaled_width > 0.0 {
                            current_x = start_x;
                            current_y += layout_line_height_scaled;
                            current_line_scaled_width = 0.0;

                            if c == ' ' || c == '\t' {
                                // Skip leading space/tab on new line
                                i += 1;
                                continue;
                            }
                        }
                    }

                    render_character(
                        char_data,
                        &mut atlas,
                        current_x,
                        current_y,
                        rot_cos,
                        rot_sin,
                        font_scale_x,
                        font_scale_y,
                        dpi_scaling,
                        color,
                        &mut max_offset_y_scaled,
                        &mut min_offset_y_scaled,
                        rot,
                    );
                    current_x += advance_scaled;
                    current_line_scaled_width += advance_scaled;
                } else {
                    // Regular character
                    if max_line_width_pixels != -1.0 {
                        if current_line_scaled_width + current_word_width_scaled + advance_scaled > max_line_width_pixels {
                            if current_line_scaled_width > 0.0 {
                                current_x = start_x;
                                current_y += layout_line_height_scaled;
                                current_line_scaled_width = 0.0;
                            } else {
                                // current_line_scaled_width == 0.0. Word (word_buffer + c) is too long for an empty line.
                                for (_buffered_char, buffered_advance, buffered_char_data) in word_buffer.drain(..) {
                                    if current_line_scaled_width + buffered_advance > max_line_width_pixels
                                        && current_line_scaled_width > 0.0
                                    {
                                        current_x = start_x;
                                        current_y += layout_line_height_scaled;
                                        current_line_scaled_width = 0.0;
                                    }
                                    render_character(
                                        &buffered_char_data,
                                        &mut atlas,
                                        current_x,
                                        current_y,
                                        rot_cos,
                                        rot_sin,
                                        font_scale_x,
                                        font_scale_y,
                                        dpi_scaling,
                                        color,
                                        &mut max_offset_y_scaled,
                                        &mut min_offset_y_scaled,
                                        rot,
                                    );
                                    current_x += buffered_advance;
                                    current_line_scaled_width += buffered_advance;
                                }
                                current_word_width_scaled = 0.0; // word_buffer is now empty.

                                if current_line_scaled_width + advance_scaled > max_line_width_pixels && current_line_scaled_width > 0.0 {
                                    current_x = start_x;
                                    current_y += layout_line_height_scaled;
                                    current_line_scaled_width = 0.0;
                                }
                            }
                        }
                    }
                    word_buffer.push((c, advance_scaled, char_data.clone()));
                    current_word_width_scaled += advance_scaled;
                }
            }
            i += 1;
        }

        render_word(
            &mut word_buffer,
            &mut atlas,
            &mut current_x,
            &mut current_y,
            &mut max_offset_y_scaled,
            &mut min_offset_y_scaled,
            rot,
            rot_cos,
            rot_sin,
            font_scale_x,
            font_scale_y,
            dpi_scaling,
            color,
        );
    }
}

// Make sure `parse_markup`, `render_word`, `render_character`, `MarkupResult`, `get_context`, `CharacterInfo`, `Color`
// and `smallvec::SmallVec` are correctly defined and in scope.
// The `render_word` and `render_character` helpers would use `max_offset_y_scaled` and `min_offset_y_scaled`.

// Make sure `parse_markup`, `render_word`, `render_character`, `MarkupResult`, `get_context`, `CharacterInfo`, `Color`
// and `smallvec::SmallVec` are correctly defined and in scope.
// The `render_word` and `render_character` helpers would use `max_offset_y_scaled` and `min_offset_y_scaled`.
// Helper function to render a buffered word
fn render_word(
    word_buffer: &mut Vec<(char, f32, CharacterInfo)>,
    atlas: &mut std::cell::RefMut<Atlas>,
    current_x: &mut f32,
    current_y: &mut f32,
    max_offset_y: &mut f32,
    min_offset_y: &mut f32,
    rot: f32,
    rot_cos: f32,
    rot_sin: f32,
    font_scale_x: f32,
    font_scale_y: f32,
    dpi_scaling: f32,
    color: Color,
) {
    for (_, advance, char_data) in word_buffer.iter() {
        render_character(
            char_data,
            atlas,
            *current_x,
            *current_y,
            rot_cos,
            rot_sin,
            font_scale_x,
            font_scale_y,
            dpi_scaling,
            color,
            max_offset_y,
            min_offset_y,
            rot,
        );

        *current_x += *advance;
    }
}

// Helper function to render a single character
fn render_character(
    char_data: &CharacterInfo,
    atlas: &mut std::cell::RefMut<Atlas>,
    current_x: f32,
    current_y: f32,
    rot_cos: f32,
    rot_sin: f32,
    font_scale_x: f32,
    font_scale_y: f32,
    dpi_scaling: f32,
    color: Color,
    max_offset_y: &mut f32,
    min_offset_y: &mut f32,
    rot: f32, // Use the original rotation value directly
) {
    let offset_x = char_data.offset_x as f32 * font_scale_x;
    let offset_y = char_data.offset_y as f32 * font_scale_y;

    let glyph = atlas.get(char_data.sprite).as_ref().unwrap().rect;
    let glyph_scaled_h = glyph.h * font_scale_y;

    *min_offset_y = (*min_offset_y).min(offset_y);
    *max_offset_y = (*max_offset_y).max(glyph_scaled_h + offset_y);

    let dest_x = (offset_x) * rot_cos + (glyph_scaled_h + offset_y) * rot_sin;
    let dest_y = (offset_x) * rot_sin + (-glyph_scaled_h - offset_y) * rot_cos;

    let dest = Rect::new(
        dest_x / dpi_scaling + current_x,
        dest_y / dpi_scaling + current_y,
        glyph.w / dpi_scaling * font_scale_x,
        glyph.h / dpi_scaling * font_scale_y,
    );

    crate::texture::draw_texture_ex(
        &crate::texture::Texture2D::create_and_cache_size(TextureHandle::Unmanaged(atlas.texture())),
        dest.x,
        dest.y,
        color,
        crate::texture::DrawTextureParams {
            dest_size: Some(vec2(dest.w, dest.h)),
            source: Some(glyph),
            rotation: rot, // Use the original rotation directly
            pivot: Some(vec2(dest.x, dest.y)),
            ..Default::default()
        },
    );
}

/// Get the text center.
pub fn get_text_center(
    text: impl AsRef<str>,
    font: Option<&Font>,
    font_size: u16,
    font_scale: f32,
    rotation: f32,
    max_line_width_unscaled: Option<f32>,
) -> crate::Vec2 {
    let measure = measure_text(text, font, font_size, font_scale, max_line_width_unscaled);

    let x_center = measure.width / 2.0 * rotation.cos() + measure.height / 2.0 * rotation.sin();
    let y_center = measure.width / 2.0 * rotation.sin() - measure.height / 2.0 * rotation.cos();

    crate::Vec2::new(x_center, y_center)
}

pub fn measure_text(
    text: impl AsRef<str>,
    font: Option<&Font>,
    font_size: u16,
    font_scale: f32,
    max_line_width_unscaled: Option<f32>,
) -> TextDimensions {
    let font = font.unwrap_or_else(|| &get_context().fonts_storage.default_font);

    font.measure_text(text, font_size, font_scale, font_scale, max_line_width_unscaled)
}

pub(crate) struct FontsStorage {
    default_font: Font,
}

impl FontsStorage {
    pub(crate) fn new(ctx: &mut dyn miniquad::RenderingBackend) -> FontsStorage {
        let atlas = Rc::new(RefCell::new(Atlas::new(ctx, miniquad::FilterMode::Linear)));

        let default_font = Font::load_from_bytes(atlas, include_bytes!("ProggyClean.ttf")).unwrap();
        FontsStorage { default_font }
    }
}

/// Returns macroquads default font.
pub fn get_default_font() -> Font {
    let context = get_context();
    context.fonts_storage.default_font.clone()
}

/// Replaces macroquads default font with `font`.
pub fn set_default_font(font: Font) {
    let context = get_context();
    context.fonts_storage.default_font = font;
}

/// From given font size in world space gives
/// (font_size, font_scale and font_aspect) params to make rasterized font
/// looks good in currently active camera
pub fn camera_font_scale(world_font_size: f32) -> (u16, f32, f32) {
    let context = get_context();
    let (scr_w, scr_h) = miniquad::window::screen_size();
    let cam_space = context.projection_matrix().inverse().transform_vector3(vec3(2., 2., 0.));
    let (cam_w, cam_h) = (cam_space.x.abs(), cam_space.y.abs());

    let screen_font_size = world_font_size * scr_h / cam_h;

    let font_size = screen_font_size as u16;

    (font_size, cam_h / scr_h, scr_h / scr_w * cam_w / cam_h)
}

enum MarkupResult {
    Literal(char),
    Pop,
    Push(Color),
    Noop,
}

fn parse_markup(chars: &[char], pos: usize) -> (MarkupResult, usize) {
    let length = chars.len();

    let c = chars[pos];

    if c == '[' {
        // Ensure we can read a next character
        if pos + 1 < length {
            // Check if the next char is just a bracket
            if chars[pos + 1] == '[' {
                return (MarkupResult::Literal('['), pos + 1);
            } else if chars[pos + 1] == ']' {
                // This would be a pop
                return (MarkupResult::Pop, pos + 2);
            } else if chars[pos + 1] == '#' {
                // We're now going to parse a color tag (either RRGGBBAA (8) or RRGGBB (6) where AA is set to FF)
                let mut rgba = 0;
                let mut shift = 32; // 32 bits (we work with nibbles, not bytes)

                // Parse the RGBA color tag.
                for x in (pos + 2)..(pos + 2 + 8 + 1) {
                    if x < length {
                        let char = chars[x];

                        // If we end the batch, ensure we have either 6 or 8 chars or it won't be valid.
                        if char == ']' {
                            // Shift is 4 in RRGGBB and -4 in RRGGBBAA.
                            if shift == 8 {
                                // 6 chars because RRGGBB
                                rgba |= 0xFF;

                                return (
                                    MarkupResult::Push(Color {
                                        r: ((rgba >> 24) & 0xFF) as f32 / 255.0,
                                        g: ((rgba >> 16) & 0xFF) as f32 / 255.0,
                                        b: ((rgba >> 8) & 0xFF) as f32 / 255.0,
                                        a: (rgba & 0xFF) as f32 / 255.0,
                                    }),
                                    pos + 2 + 6 + 1,
                                );
                            } else if shift == 0 {
                                // 8 chars because RRGGBBAA
                                return (
                                    MarkupResult::Push(Color {
                                        r: ((rgba >> 24) & 0xFF) as f32 / 255.0,
                                        g: ((rgba >> 16) & 0xFF) as f32 / 255.0,
                                        b: ((rgba >> 8) & 0xFF) as f32 / 255.0,
                                        a: (rgba & 0xFF) as f32 / 255.0,
                                    }),
                                    pos + 2 + 8 + 1,
                                );
                            } else {
                                // Wrong shift. Literal.
                                return (MarkupResult::Literal('['), pos + 1);
                            }
                        }

                        // Parse hex nibble from the char given
                        let nibble = if ('a'..='f').contains(&char) {
                            10 + char as u8 - b'a'
                        } else if char.is_ascii_digit() {
                            char as u8 - b'0'
                        } else if ('A'..='F').contains(&char) {
                            10 + char as u8 - b'A'
                        } else {
                            // Don't parse this block anymore because it's corrupt.
                            return (MarkupResult::Literal('['), pos + 1);
                        };

                        shift -= 4;
                        rgba |= (nibble as u32) << shift;
                    } else {
                        // It was out of bounds, so it's taken literally.
                        return (MarkupResult::Literal('['), pos + 1);
                    }
                }

                // Assume the color tag parsing went wrong and take the bracket literally.
                return (MarkupResult::Literal('['), pos + 1);
            }
        }
    }

    (MarkupResult::Noop, pos + 1)
}
