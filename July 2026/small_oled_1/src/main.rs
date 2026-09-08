#![no_std]
#![no_main]

use cortex_m_rt::entry;
use panic_halt as _;

use stm32f4xx_hal::{pac, prelude::*, i2c::I2c};

// Import the display driver items
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};

// Import the drawing utilities
use embedded_graphics::
{
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::Text,
};

#[entry]
fn main() -> ! 
{
    let dp = pac::Peripherals::take().unwrap();
    let mut rcc = dp.RCC.freeze(stm32f4xx_hal::rcc::Config::default());

    // 1. Setup the GPIO registers we need
    let gpiob = dp.GPIOB.split(&mut rcc);

    // 2. Configure PB8 and PB9 for I2C1 usage (Must be Alternate Open-Drain mode)
    let scl = gpiob.pb8.into_alternate_open_drain();
    let sda = gpiob.pb9.into_alternate_open_drain();

    // 3. Initialize the I2C1 peripheral at Standard Mode (100 kHz)
    let i2c = I2c::new(dp.I2C1, (scl, sda), 100.kHz(), &mut rcc);

    // 4. Wrap the raw I2C instance inside the display interface framework
    let interface = I2CDisplayInterface::new(i2c);

    // 5. Setup the display layout sizing (128x32 pixels)
    let mut display = Ssd1306::new(interface, DisplaySize128x32, DisplayRotation::Rotate0).into_buffered_graphics_mode();
    
    // Initialize the physical screen hardware
    display.init().unwrap();

    // 6. Create a text style layout (using a built-in 6x10 pixel font)
    let text_style = MonoTextStyleBuilder::new()
                                                    .font(&FONT_6X10)
                                                    .text_color(BinaryColor::On)
                                                    .build();

    // 7. Write text onto the virtual frame buffer
    // Point::new(X coordinate, Y coordinate)
    Text::new("Hello, Rust!", Point::new(10, 20), text_style)
                                    .draw(&mut display)
                                    .unwrap();

    // 8. Push the virtual frame buffer pixels over I2C to the physical screen
    display.flush().unwrap();

    loop 
    {
        // Keep the microcontroller running indefinitely
        cortex_m::asm::nop();
    }
}