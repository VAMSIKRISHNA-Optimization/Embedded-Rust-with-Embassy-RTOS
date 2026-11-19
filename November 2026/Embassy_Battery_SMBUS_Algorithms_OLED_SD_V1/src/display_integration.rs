pub mod display_touch_calibration;
pub mod qwerty_keyboard;


use embedded_graphics::
{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Rectangle, PrimitiveStyleBuilder},
    text::Text,
};

use u8g2_fonts::{fonts, U8g2TextStyle};


use core::fmt::Debug;
use embedded_graphics::draw_target::DrawTarget;