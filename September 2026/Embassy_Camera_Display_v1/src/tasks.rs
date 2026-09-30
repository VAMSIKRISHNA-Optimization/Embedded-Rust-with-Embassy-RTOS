use embassy_stm32::spi::Spi;
use embassy_stm32::dcmi::Dcmi;
use embassy_stm32::peripherals::{SPI1, DCMI, DMA2_CH3, DMA2_CH1, PC4, PC5, PB1};
use embassy_stm32::dma::NoDma;
use embassy_stm32::gpio::Output;
use embassy_time::{Delay, Timer};

use defmt::{info};

use display_interface_spi::SPIInterface;
use mipidsi::Builder;
use mipidsi::models::ILI9341Rgb565;


use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::pixelcolor::raw::RawU16;
use embedded_graphics::geometry::Point;
use embedded_graphics::image::Image;

use embedded_hal_bus::spi::ExclusiveDevice;

use tinybmp::Bmp;

use static_cell::StaticCell;
use core::cell::UnsafeCell;

use embassy_time::{Duration, WithTimeout};

// 1. Declare the massive buffer globally outside the task function
// Expanded to 9602 to safely absorb a 1-byte DMA shift without going out of bounds
static mut PADDED_BUF_WORDS: [u32; 9602] = [0; 9602];
const FRAME_WIDTH   : u32 = 160;
const FRAME_HEIGHT  : u32 = 120;
const FRAME_SIZE_BYTES: usize = (FRAME_WIDTH * FRAME_HEIGHT * 2) as usize; // 38,400 bytes
const FRAME_SIZE_WORDS: usize = 9602; // Added 2 extra words for safety padding

// static FRAME_BUFFER: StaticCell<[u32; FRAME_SIZE_WORDS]> = StaticCell::new();

// // Static RAM allocation (.bss) - 0 stack memory overhead
// static mut FRAME_BUFFER: [u32; FRAME_SIZE_WORDS] = [0u32; FRAME_SIZE_WORDS];

// Thread-safe wrapper for interior mutability.
// Places the 38.4 KB buffer directly into RAM (.bss) at compile time without stack overhead.
struct SyncBuffer<T>(UnsafeCell<T>);
unsafe impl<T> Sync for SyncBuffer<T> {}

static FRAME_BUFFER: SyncBuffer<[u32; FRAME_SIZE_WORDS]> = SyncBuffer(UnsafeCell::new([0u32; FRAME_SIZE_WORDS]));



#[embassy_executor::task]
pub async fn touchscreen_display_task(spi: Spi<'static, SPI1, DMA2_CH3, NoDma>, cs: Output<'static, PC4>, dc: Output<'static, PC5>, rst: Output<'static, PB1>) 
{
    // 1. Create a delay object for timing operations
    let mut delay = Delay; // Blocking delay

    info!("Hello from the TouchScreen Display Task!");

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


#[embassy_executor::task]
pub async fn camera_stream_task(
    mut dcmi: Dcmi<'static, DCMI, DMA2_CH1>,
    spi: Spi<'static, SPI1, DMA2_CH3, NoDma>,
    cs: Output<'static, PC4>,
    dc: Output<'static, PC5>,
    rst: Output<'static, PB1>,
) 
{
    let mut delay = Delay;

    info!("Initializing Display for Camera Streaming...");

    let spi_device = ExclusiveDevice::new(spi, cs, Delay).unwrap();
    let spi_interface = SPIInterface::new(spi_device, dc);

    let builder = Builder::new(ILI9341Rgb565, spi_interface)
        .reset_pin(rst)
        .display_size(240, 320);
    
    let mut display = builder.init(&mut delay).unwrap();

    // Safely acquire reference to static buffer without triggering `static_mut_refs`
    let frame_buf_words: &'static mut [u32; FRAME_SIZE_WORDS] = unsafe { &mut *FRAME_BUFFER.0.get() };

    // Bounding box centered on the 320x240 screen
    let area = Rectangle::new(Point::new(80, 60), Size::new(FRAME_WIDTH, FRAME_HEIGHT));

    info!("Starting Camera Streaming Loop...");

    loop 
    {
        info!("0. CAPTURE BEGINNING...");
        
        // Grab a mutable reference to the global static buffer
        let padded_buf_words = unsafe { &mut *core::ptr::addr_of_mut!(PADDED_BUF_WORDS) };

        // 1. Capture the frame (it's okay if this still warns about overrun)
        match dcmi.capture(padded_buf_words).await {
            Ok(_) => defmt::info!("Capture success!"),
            Err(e) => defmt::warn!("Capture error (likely overrun): {:?}", defmt::Debug2Format(&e)),
        }

        // 2. Safely cast the ENTIRE padded buffer to bytes
        let raw_bytes: &[u8] = unsafe {
            core::slice::from_raw_parts(
                padded_buf_words.as_ptr() as *const u8,
                38408 // 9602 words * 4 bytes = 38408 bytes
            )
        };

        // 3. THE MAGIC SHIFT: Skip the first garbage byte left over from the overrun
        let offset = 2; // Change to 0 or 2 if geometry is still broken
        
        // 4. Slice out exactly 38,400 bytes for the screen, starting after the offset
        let frame_buf_bytes = &raw_bytes[offset .. offset + 38400];

        // 5. FIX THE PURPLE: Use Little Endian (from_le_bytes) to decode real colors
        let colors = frame_buf_bytes.chunks_exact(2).map(|chunk| {
            let raw_pixel = u16::from_le_bytes([chunk[0], chunk[1]]); 
            Rgb565::from(RawU16::new(raw_pixel))
        });

        // 6. Render to display
        let _ = display.fill_contiguous(&area, colors);

        defmt::info!("3. DONE! Frame rendered to display. Looping back for next capture...");
        // embassy_time::block_for(embassy_time::Duration::from_millis(1000));
    }
}