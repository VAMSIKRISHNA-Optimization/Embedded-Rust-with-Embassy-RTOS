use super::*;
use embedded_graphics::{
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle},
    pixelcolor::Rgb565,
};
use u8g2_fonts::{fonts, U8g2TextStyle};
use core::fmt::Debug;

pub async fn render_qwerty_keyboard<D>(display: &mut D)
where
    D: DrawTarget<Color = Rgb565>,
    D::Error: Debug,
{
    let text_style = U8g2TextStyle::new(fonts::u8g2_font_helvB12_tf, Rgb565::WHITE);
    let button_style = PrimitiveStyleBuilder::new()
        .stroke_color(Rgb565::WHITE)
        .stroke_width(1)
        .fill_color(Rgb565::new(8, 16, 8))
        .build();

    let key_h  : i32 = 32;
    let start_y: i32 = 80; // Leaves top 80px for the text viewport

    // Helper closure to draw buttons
    let mut draw_btn = |label: &str, x: i32, y: i32, w: i32| 
    {
        Rectangle::new(Point::new(x, y), Size::new(w as u32, key_h as u32))
            .into_styled(button_style)
            .draw(display)
            .unwrap();

        // Approximate centering for text
        let text_x = x + (w / 2) - (label.len() as i32 * 4);
        Text::new(label, Point::new(text_x, y + 22), text_style.clone())
            .draw(display)
            .unwrap();
    };

    // Row 1: Numbers (10 keys, 32px each)
    let row1 = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
    for (i, key) in row1.iter().enumerate() 
    {
        draw_btn(key, (i as i32) * 32, start_y, 32);
    }

    // Row 2: QWERTY (10 keys, 32px each)
    let row2 = ["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"];
    for (i, key) in row2.iter().enumerate() 
    {
        draw_btn(key, (i as i32) * 32, start_y + key_h, 32);
    }

    // Row 3: ASDFGHJKL (9 keys, 32px each, offset by 16px to center)
    let row3 = ["A", "S", "D", "F", "G", "H", "J", "K", "L"];
    for (i, key) in row3.iter().enumerate() 
    {
        draw_btn(key, 16 + (i as i32) * 32, start_y + 2 * key_h, 32);
    }

    // Row 4: Shift/Up, ZXCVBNM, Backspace/DEL
    // ^ (Up Scroll) is 48px wide, letters are 32px, DEL is 48px wide. Total = 320px.
    let row4_y = start_y + 3 * key_h;
    draw_btn("^", 0, row4_y, 48);
    let row4_letters = ["Z", "X", "C", "V", "B", "N", "M"];
    for (i, key) in row4_letters.iter().enumerate() 
    {
        draw_btn(key, 48 + (i as i32) * 32, row4_y, 32);
    }
    draw_btn("DEL", 48 + (7 * 32), row4_y, 48);

    // Row 5: Down, Left, Right, Space, Period, LOG
    // v(48px), <(32px), >(32px), SPACE(128px), .(32px), LOG(48px)
    let row5_y = start_y + 4 * key_h;
    draw_btn("v", 0, row5_y, 48);
    draw_btn("<", 48, row5_y, 32);
    draw_btn(">", 80, row5_y, 32);
    draw_btn("SPACE", 112, row5_y, 128);
    draw_btn(".", 240, row5_y, 32);
    draw_btn("LOG", 272, row5_y, 48);
}