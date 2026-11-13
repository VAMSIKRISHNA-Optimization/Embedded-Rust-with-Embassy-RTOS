use super::*;
use crate::tasks::TOUCH_CHANNEL;

pub async fn display_touch_calibration<D>(display: &mut D) -> (u16, u16, u16, u16)
where
    D: DrawTarget<Color = Rgb565>,
    D::Error: Debug,
{
    let text_style = U8g2TextStyle::new(fonts::u8g2_font_helvB12_tf, Rgb565::WHITE);

    // 1. Clear the screen with a black background
    display.clear(Rgb565::BLACK).unwrap();

    // 1.1 Calibrate the touch using squares on the display
    Text::new("TOUCH CALIBRATION - 1", Point::new(80, 120), text_style.clone())
        .draw(display)
        .unwrap();

    // 1.2 First White Square at (0,0) (Top-Left)
    Rectangle::new(Point::new(0, 0), Size::new(10, 10))
        .into_styled(PrimitiveStyleBuilder::new().fill_color(Rgb565::WHITE).build())
        .draw(display)
        .unwrap();

    // 1.3 Await user's touch
    let (raw_x1, raw_y1) = TOUCH_CHANNEL.receive().await;

    display.clear(Rgb565::BLACK).unwrap();

    // 1.4 Second calibration text
    Text::new("TOUCH CALIBRATION - 2", Point::new(80, 120), text_style.clone())
        .draw(display)
        .unwrap();

    // 1.5 Second White Square at (310, 230) (Bottom-Right for Landscape)
    Rectangle::new(Point::new(310, 230), Size::new(10, 10))
        .into_styled(PrimitiveStyleBuilder::new().fill_color(Rgb565::WHITE).build())
        .draw(display)
        .unwrap();

    // 1.6 Await user's touch
    let (raw_x2, raw_y2) = TOUCH_CHANNEL.receive().await;

    display.clear(Rgb565::BLACK).unwrap();

    (raw_x1, raw_y1, raw_x2, raw_y2)
}