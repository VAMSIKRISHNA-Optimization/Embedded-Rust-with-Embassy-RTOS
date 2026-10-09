use defmt::info;
use embassy_stm32::gpio::Output;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::spi::Spi;
use embassy_stm32::peripherals::{SPI1, SPI2, PB6, PB12, PC5, PC7, PA9};
use embassy_stm32::dma::NoDma;


// Alias core formatting trait for display text formatting
use core::fmt::Write as FmtWrite;

use embassy_time::{block_for, Duration, Timer, Delay, Ticker};

use embedded_graphics::
{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Rectangle, PrimitiveStyleBuilder},
    text::Text,
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
};
use display_interface_spi::SPIInterface;
use mipidsi::Builder;
use mipidsi::models::ILI9341Rgb565;
use embassy_time::{};

use crate::{TOUCH_SIGNAL};


// -------------------------------------------------------------------
// 1. Touch Task (Handles edge transitions + pre-existing low state)
// -------------------------------------------------------------------
#[embassy_executor::task]
pub async fn touchscreen_touch_task
(
    mut spi: Spi<'static, SPI2, NoDma, NoDma>,
    mut cs: Output<'static, PB12>,
    mut irq: ExtiInput<'static, PC5>, 
) 
{
    info!("Touch Task Started!");

    // CRITICAL: Ensure SPI frequency is reconfigured to <= 2 MHz here 
    // if this bus is shared with the display!

    cs.set_low();
    let _ = spi.blocking_transfer(&mut [0u8; 1], &[0x90]); // Send wake up
    cs.set_high();

    info!("Touch Task Initialized. Listening for touch events...");

    loop 
    {
        if irq.is_high() 
        {
            irq.wait_for_falling_edge().await;
        }

        info!("Screen Touched! Extracting coordinates...");
        block_for(Duration::from_millis(1));

        cs.set_low();
        
        // 1. Send X command (0x90)
        let _ = spi.blocking_transfer(&mut [0u8; 1], &[0x90]);
        // Block the CPU for 100 microseconds to let the ADC convert
        block_for(Duration::from_micros(100));
        
        // 2. Read 2 bytes for X
        let mut x_buf = [0u8; 2];
        let _ = spi.blocking_transfer(&mut x_buf, &[0x00, 0x00]);
        // Block the CPU for 100 microseconds to let the ADC convert
        block_for(Duration::from_micros(100));

        // 3. Send Y command (0xD0)
        let _ = spi.blocking_transfer(&mut [0u8; 1], &[0xD0]);
        // Block the CPU for 100 microseconds to let the ADC convert
        block_for(Duration::from_micros(100));
        
        // 4. Read 2 bytes for Y
        let mut y_buf = [0u8; 2];
        let _ = spi.blocking_transfer(&mut y_buf, &[0x00, 0x00]);
        // Block the CPU for 100 microseconds to let the ADC convert
        block_for(Duration::from_micros(100));


        // 5. Send command to power down ADC and enable PENIRQ again
        let _ = spi.blocking_transfer(&mut [0u8; 1], &[0x90]); 
        // Block the CPU for 100 microseconds to let the ADC convert
        block_for(Duration::from_micros(100));
        
        cs.set_high();

        // The XPT2046 returns 1 dummy bit, 12 data bits, and 3 trailing zeros.
        // Shifting by 3 correctly aligns the 12-bit value.
        let raw_x = (((x_buf[0] as u16) << 8) | (x_buf[1] as u16)) >> 3;
        let raw_y = (((y_buf[0] as u16) << 8) | (y_buf[1] as u16)) >> 3;

        info!("Raw Touch -> X: {}, Y: {}", raw_x, raw_y);

        // if raw_x < 4095 && raw_y < 4095 && raw_x > 100 && raw_y > 100 
        // {
        //     TOUCH_SIGNAL.signal(true);
        // }
        
        embassy_time::Timer::after_millis(150).await;
    }
}


// -------------------------------------------------------------------
// 2. Display Task
// -------------------------------------------------------------------
#[embassy_executor::task]
pub async fn touchscreen_display_task
(
    spi: Spi<'static, SPI1, NoDma, NoDma>,
    cs: Output<'static, PB6>,
    dc: Output<'static, PC7>,
    rst: Output<'static, PA9>,
) {
    info!("Initializing TouchScreen Display...");
    let mut delay = Delay;
    let spi_device = embedded_hal_bus::spi::ExclusiveDevice::new(spi, cs, Delay).unwrap();
    let spi_interface = SPIInterface::new(spi_device, dc);
    let builder = Builder::new(ILI9341Rgb565, spi_interface).reset_pin(rst).display_size(240, 320);

    let mut display = builder.init(&mut delay).unwrap();
    let mut bg_color = Rgb565::BLACK;
    let mut ticker = Ticker::every(Duration::from_secs(1));
    let text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);

    display.clear(bg_color).unwrap();

    loop 
    {

    }
}

