pub mod display_touch_calibration;
pub mod qwerty_keyboard;


use embedded_graphics::
{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Rectangle, PrimitiveStyleBuilder},
    text::Text,
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
};

use u8g2_fonts::{fonts, U8g2TextStyle};

use embedded_graphics::prelude::*;
use core::fmt::Debug;
use embedded_graphics::draw_target::DrawTarget;