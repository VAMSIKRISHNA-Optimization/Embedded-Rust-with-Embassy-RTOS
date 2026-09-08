use embassy_stm32::spi::Spi;
use embassy_stm32::peripherals::{SPI1, PB6, PC7, PA9, SPI2, PB12, PC5};
use embassy_stm32::dma::NoDma;
use embassy_stm32::gpio::Output;
use embassy_time::{Delay};

use defmt::{info};

use display_interface_spi::SPIInterface;
use mipidsi::Builder;
use mipidsi::models::ILI9341Rgb565;

use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::Rgb565;

use embedded_hal_bus::spi::ExclusiveDevice;

use crate::TOUCH_SIGNAL;
use embassy_stm32::exti::ExtiInput;

#[embassy_executor::task]
pub async fn touchscreen_touch_task(mut spi: Spi<'static, SPI2, NoDma, NoDma>, mut cs: Output<'static, PB12>, mut irq: ExtiInput<'static, PC5>) 
{
    info!("Touch Task Started!");

    loop 
    {
        // 1. Go to sleep until the screen is physically pressed!
        irq.wait_for_low().await;
        
        info!("Screen Touched! Extracting coordinates...");

        // Buffer to hold the incoming data from the SPI bus
        let mut rx_buf = [0u8; 3];

        // -----------------------------------
        // 2. Read X Coordinate
        // Command 0xD0 (11010000): Start=1, Channel=X, 12-bit, Differential
        // -----------------------------------
        let tx_x = [0xD0, 0x00, 0x00];
        
        cs.set_low();
        // Since we are using NoDma, we use the blocking transfer method
        let _ = spi.blocking_transfer(&mut rx_buf, &tx_x);
        cs.set_high();
        
        // The 12-bit payload spans across rx_buf[1] and rx_buf[2].
        // Shift right by 3 to account for the XPT2046 busy clock cycle.
        let mut raw_x = (((rx_buf[1] as u16) << 8) | (rx_buf[2] as u16)) >> 3;
        raw_x &= 0x0FFF; // Mask out any garbage bits to ensure 12-bit max (4095)

        // -----------------------------------
        // 3. Read Y Coordinate
        // Command 0x90 (10010000): Start=1, Channel=Y, 12-bit, Differential
        // -----------------------------------
        let tx_y = [0x90, 0x00, 0x00];
        
        cs.set_low();
        let _ = spi.blocking_transfer(&mut rx_buf, &tx_y);
        cs.set_high();
        
        let mut raw_y = (((rx_buf[1] as u16) << 8) | (rx_buf[2] as u16)) >> 3;
        raw_y &= 0x0FFF;

        // Print the extracted 12-bit values to the defmt terminal
        info!("Raw Touch -> X: {}, Y: {}", raw_x, raw_y);

        // 4. Signal the display task to wake up and change color
        TOUCH_SIGNAL.signal(true);

        // 5. Wait for the user to lift their finger before looping
        // This prevents terminal spam while the finger is held down
        irq.wait_for_high().await;
    }
}


#[embassy_executor::task]
pub async fn touchscreen_display_task(spi: Spi<'static, SPI1, NoDma, NoDma>, cs: Output<'static, PB6>, dc: Output<'static, PC7>, rst: Output<'static, PA9>) 
{
    // 1. Create a delay object for timing operations
    let mut delay = Delay; // Blocking delay

    info!("Initializing TouchScreen Display...");

    // 2. Combine the SPI bus and CS pin into a single SpiDevice
    let spi_device = ExclusiveDevice::new(spi, cs, Delay).unwrap();

    // 3. Create an SPI interface for the display
    let spi_interface = SPIInterface::new(spi_device, dc);

    // Configuring the builder for the display driver
    let builder = Builder::new(ILI9341Rgb565, spi_interface).reset_pin(rst).display_size(240, 320);//.invert_colors(ColorInversion::Inverted);

    // Initialize the display with the provided pins and delay
    let mut display = builder.init(&mut delay).unwrap();

    // Color toggle variable to switch between red and blue
    let mut color_toggle = false;

    loop 
    {
        // 1. Go to sleep and consume zero CPU until signaled!
        let _ = TOUCH_SIGNAL.wait().await;

        info!("Display Task Woke Up! Changing color...");

        // 2. Toggle color based on touch
        if color_toggle 
        {
            display.clear(Rgb565::RED).unwrap();
        } 
        else 
        {
            display.clear(Rgb565::BLUE).unwrap();
        }
        
        color_toggle = !color_toggle;
    }
}