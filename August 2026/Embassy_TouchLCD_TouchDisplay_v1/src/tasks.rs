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
pub async fn touchscreen_touch_task(_spi: Spi<'static, SPI2, NoDma, NoDma>, _cs: Output<'static, PB12>, mut irq: ExtiInput<'static, PC5>) 
{
    info!("Touch Task Started!");

    loop 
    {
        // 1. Go to sleep until the screen is physically pressed!
        irq.wait_for_low().await;
        
        info!("Screen Touched! Waking up display task...");

        // 2. We will read the X/Y SPI coordinates here later.

        // 3. Signal the display task to wake up and change color
        TOUCH_SIGNAL.signal(true);

        // 4. Wait for the user to lift their finger before looping
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