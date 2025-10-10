use macroquad::prelude::*;

const EXAMPLE_TEXT_1: &str = "Hello [#FF0000]red text[] normal text";
const EXAMPLE_TEXT_2: &str = "This is normal text [#FF0000]then red text that will wrap across multiple lines[] and back to normal";
const EXAMPLE_TEXT_3: &str =
    "Multiple [#FF0000]red[] [#00FF00]green[] [#0000FF]blue[] colors in one line hen red text that will wrap across multiple";
const EXAMPLE_TEXT_4: &str = "Line with [#FF0000]red text\nand newline[] normal text";

#[macroquad::main("Markup Line Width Test")]
async fn main() {
    let font_size = 24.0;
    let wrap_width = 300.0;
    let x = 50.0;
    let mut y = 50.0;

    loop {
        clear_background(WHITE);

        let params = TextParams {
            font: None,
            font_size: font_size as u16,
            font_scale: 1.0,
            font_scale_aspect: 1.0,
            rotation: 0.0,
            color: BLACK,
            enable_markup: true,
            max_line_width: Some(wrap_width),
        };

        // Test 1: Simple markup without wrapping
        draw_text("Test 1: Simple markup (no wrapping)", x, y - 20.0, 16.0, DARKGRAY);

        let dims1 = measure_text(EXAMPLE_TEXT_1, None, font_size as u16, 1.0, Some(wrap_width));
        draw_text(
            &format!("Measured width: {:.1} (should ignore markup)", dims1.width),
            x,
            y - 5.0,
            14.0,
            BLUE,
        );

        // Draw background rectangles
        for (i, line) in dims1.line_widths.iter().enumerate() {
            let line_y = y + (i as f32 * line.y) - dims1.offset_y;
            draw_rectangle(x, line_y, line.x, line.y, Color::new(0.0, 0.0, 1.0, 0.2));
        }

        // Draw text with markup
        draw_text_ex(EXAMPLE_TEXT_1, x, y, params.clone());

        // Show what it's measured as (without markup)
        let clean_text1 = "Hello red text normal text";
        let clean_dims1 = measure_text(clean_text1, None, font_size as u16, 1.0, Some(wrap_width));
        draw_text(
            &format!("Clean text width: {:.1}", clean_dims1.width),
            x + 350.0,
            y - 5.0,
            14.0,
            GREEN,
        );
        draw_text_ex(
            clean_text1,
            x + 350.0,
            y,
            TextParams {
                color: DARKGREEN,
                ..params
            },
        );

        y += dims1.height + 60.0;

        // Test 2: Markup with line wrapping
        draw_text("Test 2: Markup with line wrapping", x, y - 20.0, 16.0, DARKGRAY);

        let dims2 = measure_text(EXAMPLE_TEXT_2, None, font_size as u16, 1.0, Some(wrap_width));
        draw_text(
            &format!("Lines: {}, Width: {:.1}", dims2.line_widths.len(), dims2.width),
            x,
            y - 5.0,
            14.0,
            BLUE,
        );

        // Draw background rectangles for each line
        for (i, line) in dims2.line_widths.iter().enumerate() {
            let line_y = y + (i as f32 * line.y) - dims2.offset_y;
            draw_rectangle(x, line_y, line.x, line.y, Color::new(1.0, 0.0, 0.0, 0.2));
            // Draw line number
            draw_text(&format!("{}", i + 1), x - 20.0, line_y + line.y / 2.0, 12.0, DARKGRAY);
        }

        // Draw text with markup (colors persist across lines)
        draw_text_ex(EXAMPLE_TEXT_2, x, y, params.clone());

        y += dims2.height + 60.0;

        // Test 3: Multiple colors in one line
        draw_text("Test 3: Multiple colors", x, y - 20.0, 16.0, DARKGRAY);

        let font_scale = 0.56;
        let dims3 = measure_text(EXAMPLE_TEXT_3, None, font_size as u16, font_scale, Some(wrap_width));
        draw_text(&format!("Width: {:.1} (markup tags ignored)", dims3.width), x, y - 5.0, 14.0, BLUE);

        // Draw background rectangles for each line
        for (i, line) in dims3.line_widths.iter().enumerate() {
            let line_y = y + (i as f32 * line.y) - dims3.offset_y;
            draw_rectangle(x, line_y, line.x, line.y, Color::new(0.5, 0.5, 0.5, 0.2));
            // Draw line number
            draw_text(&format!("{}", i + 1), x - 20.0, line_y + line.y / 2.0, 12.0, DARKGRAY);
        }

        // Draw text

        let mut new_params = params.clone();
        new_params.font_scale = font_scale;

        draw_text_ex(EXAMPLE_TEXT_3, x, y, new_params);

        y += dims3.height + 60.0;

        // Test 4: Markup with explicit newlines
        draw_text("Test 4: Markup with newlines", x, y - 20.0, 16.0, DARKGRAY);

        let dims4 = measure_text(EXAMPLE_TEXT_4, None, font_size as u16, 1.0, Some(wrap_width));
        draw_text(&format!("Lines: {} (one per \\n)", dims4.line_widths.len()), x, y - 5.0, 14.0, BLUE);

        // Draw background rectangles for each line
        for (i, line) in dims4.line_widths.iter().enumerate() {
            let line_y = y + (i as f32 * line.y) - dims4.offset_y;
            draw_rectangle(x, line_y, line.x, line.y, Color::new(0.0, 1.0, 0.0, 0.2));
        }

        // Draw text (red color continues after newline)
        draw_text_ex(EXAMPLE_TEXT_4, x, y, params);

        y += dims4.height + 80.0;

        // Demonstration section
        draw_text("KEY POINTS:", x, y, 18.0, RED);
        y += 25.0;

        draw_text(
            "• Markup tags [#FF0000] and [] are INVISIBLE to width calculations",
            x,
            y,
            14.0,
            BLACK,
        );
        y += 20.0;

        draw_text(
            "• Colors persist across line wraps and newlines until [] or new color",
            x,
            y,
            14.0,
            BLACK,
        );
        y += 20.0;

        draw_text("• Line breaking decisions are based ONLY on visible text", x, y, 14.0, BLACK);
        y += 20.0;

        draw_text(
            "• Background rectangles show measured dimensions match visible text",
            x,
            y,
            14.0,
            BLACK,
        );

        // Reset for next frame
        if y > screen_height() - 100.0 {
            y = 50.0;
        }

        next_frame().await;
    }
}
