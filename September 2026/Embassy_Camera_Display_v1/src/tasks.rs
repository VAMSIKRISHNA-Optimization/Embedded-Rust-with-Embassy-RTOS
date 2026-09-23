use embassy_stm32::spi::Spi;
use embassy_stm32::peripherals::{SPI1, PB6, PC7, PA9, SPI2, PB12, PC4, PA3};
use embassy_stm32::dma::NoDma;
use embassy_stm32::gpio::Output;
use embassy_time::{Delay, Timer};

use defmt::{info};

use display_interface_spi::SPIInterface;
use mipidsi::Builder;
use mipidsi::models::ST7789;
use mipidsi::models::ILI9341Rgb565;
use mipidsi::options::ColorInversion; 


use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Rectangle, PrimitiveStyle};
use embedded_graphics::pixelcolor::Rgb565;

use embedded_hal_bus::spi::ExclusiveDevice;

use tinybmp::Bmp;
use embedded_graphics::image::Image;
use embedded_graphics::geometry::Point;


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
    // let builder = Builder::new(ST7789, spi_interface).reset_pin(rst).display_size(240, 320);//.invert_colors(ColorInversion::Inverted);
    
    // Initialize the display with the provided pins and delay
    let mut display = builder.init(&mut delay).unwrap();


    // 1. Embed the raw file bytes directly into your firmware
    let bmp_data = include_bytes!("sample.bmp");

    // 2. Parse the bytes into a BMP object
    let bmp: Bmp<Rgb565> = Bmp::from_slice(bmp_data).unwrap();

    // 3. Wrap it in an Image struct and draw it at coordinates (x: 0, y: 0)
    Image::new(&bmp, Point::new(0, 0)).draw(&mut display).unwrap();

    loop 
    {
        // info!("Refreshing Display...");

        // // Clear the display with a solid color (e.g., black)
        // display.clear(Rgb565::BLACK).unwrap();

        // // Draw a simple rectangle on the display
        // let rect = Rectangle::new(Point::new(10, 10), Size::new(100, 50));
        // rect.into_styled(PrimitiveStyle::with_fill(Rgb565::RED)).draw(&mut display).unwrap();
                                                            
                                                        
        // // Wait for 2 seconds before updating the display again
        // Timer::after_secs(2).await;

        // info!("Clearing RED...");
        // display.clear(Rgb565::RED).unwrap();
        // Timer::after_secs(1).await;

        // info!("Clearing GREEN...");
        // display.clear(Rgb565::GREEN).unwrap();
        // Timer::after_secs(1).await;

        // info!("Clearing BLUE...");
        // display.clear(Rgb565::BLUE).unwrap();
        // Timer::after_secs(1).await;
    }
}