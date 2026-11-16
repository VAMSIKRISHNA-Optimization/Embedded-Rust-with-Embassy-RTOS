use super::*;


// -------------------------------------------------------------------
// 2. Display Task (QWERTY Keyboard Version)
// -------------------------------------------------------------------
#[embassy_executor::task]
pub async fn touchscreen_display_task(
    mut spi: DisplaySpiDevice, 
    dc: Output<'static, PC7>,
    rst: Output<'static, PA9>,
) 
{
    info!("Initializing TouchScreen Display for QWERTY...");

    let mut delay = Delay;
    let spi_interface = SPIInterface::new(spi, dc);

    use mipidsi::options::{Orientation, Rotation};
    let builder = Builder::new(ILI9341Rgb565, spi_interface)
        .reset_pin(rst)
        .display_size(240, 320)
        .orientation(Orientation::new().rotate(Rotation::Deg90)); // Landscape

    let mut display = builder.init(&mut delay).unwrap();

    // 1. Display Touch Calibration
    use crate::display_integration::display_touch_calibration::display_touch_calibration;
    let (raw_x1, raw_y1, raw_x2, raw_y2) = display_touch_calibration(&mut display).await;

    // 2. Render the QWERTY Keyboard
    use crate::display_integration::qwerty_keyboard::render_qwerty_keyboard;
    render_qwerty_keyboard(&mut display).await;

    // 3. Clear the top 80px text viewport
    Rectangle::new(Point::new(0, 0), Size::new(320, 80))
        .into_styled(PrimitiveStyleBuilder::new().fill_color(Rgb565::BLACK).build())
        .draw(&mut display)
        .unwrap();

    // 4. Expanded buffer for longer messages
    let mut text_buf: heapless::String<256> = heapless::String::new();
    use u8g2_fonts::{fonts, U8g2TextStyle};
    let input_text_style = U8g2TextStyle::new(fonts::u8g2_font_helvB12_tf, Rgb565::WHITE);

    // 5. Idle loop
    loop 
    {
        let (raw_x, raw_y) = TOUCH_CHANNEL.receive().await;

        let rx = raw_x as i32; let rx1 = raw_x1 as i32; let rx2 = raw_x2 as i32;
        let ry = raw_y as i32; let ry1 = raw_y1 as i32; let ry2 = raw_y2 as i32;

        // 1. Map Raw X to Pixel X (using the 310 calibration distance)
        let mut pixel_x = ((rx - rx1) * 310) / (rx2 - rx1);
        
        // 2. Map Raw Y to Pixel Y (using the 230 calibration distance)
        let mut pixel_y = ((ry - ry1) * 230) / (ry2 - ry1);

        pixel_x = pixel_x.clamp(0, 319);
        pixel_y = pixel_y.clamp(0, 239);

        let mut update_display = false;
        let mut char_to_add: Option<&str> = None;

        // 5.3 QWERTY Hit Detection Logic
        if pixel_y >= 80 && pixel_y < 112 
        {
            // Row 1: Numbers
            let col = (pixel_x / 32) as usize;
            let row1 = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
            if col < 10 { char_to_add = Some(row1[col]); }

        } else if pixel_y >= 112 && pixel_y < 144 {
            // Row 2: QWERTY
            let col = (pixel_x / 32) as usize;
            let row2 = ["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"];
            if col < 10 { char_to_add = Some(row2[col]); }

        } else if pixel_y >= 144 && pixel_y < 176 {
            // Row 3: ASDF... (Starts at 16px offset)
            if pixel_x >= 16 && pixel_x < 304 
            {
                let col = ((pixel_x - 16) / 32) as usize;
                let row3 = ["A", "S", "D", "F", "G", "H", "J", "K", "L"];
                if col < 9 { char_to_add = Some(row3[col]); }
            }

        } else if pixel_y >= 176 && pixel_y < 208 {
            // Row 4: UP, ZXCVBNM, DEL
            if pixel_x < 48 
            {
                info!("UP scroll pressed");
            } else if pixel_x < 272 {
                let col = ((pixel_x - 48) / 32) as usize;
                let row4 = ["Z", "X", "C", "V", "B", "N", "M"];
                if col < 7 { char_to_add = Some(row4[col]); }
            } else {
                if text_buf.pop().is_some() 
                {
                    info!("DEL pressed");
                    update_display = true;
                }
            }

        } else if pixel_y >= 208 {
            // Row 5: DOWN, LEFT, RIGHT, SPACE, PERIOD, LOG
            if pixel_x < 48 
            {
                info!("DOWN scroll pressed");
            } else if pixel_x < 80 {
                info!("LEFT cursor pressed");
            } else if pixel_x < 112 {
                info!("RIGHT cursor pressed");
            } else if pixel_x < 240 {
                char_to_add = Some(" "); // SPACE
            } else if pixel_x < 272 {
                char_to_add = Some("."); // PERIOD
            } else {
                if !text_buf.is_empty() 
                {
                    info!("LOG pressed, sending to SD Card...");
                    SD_CHANNEL.send(text_buf.clone()).await;
                    text_buf.clear();
                    update_display = true;
                }
            }
        }

        // Apply character addition if a valid key was hit
        if let Some(c) = char_to_add {
            if text_buf.push_str(c).is_ok() {
                info!("Added: {}", c);
                update_display = true;
            }
        }

        // 5.4 Redraw the text area
        if update_display {
            // Clear the 320x80 text viewport
            Rectangle::new(Point::new(0, 0), Size::new(320, 80))
                .into_styled(PrimitiveStyleBuilder::new().fill_color(Rgb565::BLACK).build())
                .draw(&mut display)
                .unwrap();

            // Render the updated string
            Text::new(
                text_buf.as_str(),
                Point::new(5, 20), // Adjusted top-left for 12pt font wrapping
                input_text_style.clone(),
            )
            .draw(&mut display)
            .unwrap();
        }
    }
}

