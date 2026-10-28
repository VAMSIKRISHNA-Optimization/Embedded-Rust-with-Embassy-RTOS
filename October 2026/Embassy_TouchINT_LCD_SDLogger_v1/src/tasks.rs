use defmt::info;
use embassy_stm32::gpio::Output;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::spi::Spi;
use embassy_stm32::peripherals::{SPI1, SPI2, PB0, PB6, PB12, PC5, PC7, PA9};
use embassy_stm32::dma::NoDma;

use core::cell::RefCell;

use embassy_embedded_hal::shared_bus::blocking::spi::SpiDeviceWithConfig;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;

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


// Type alias to keep the function signature clean
type DisplaySpiDevice = SpiDeviceWithConfig<'static, NoopRawMutex, Spi<'static, SPI1, NoDma, NoDma>, Output<'static, PB6>>;
type SdSpiDevice      = SpiDeviceWithConfig<'static, NoopRawMutex, Spi<'static, SPI1, NoDma, NoDma>, Output<'static, PB0>>;


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
        // if irq.is_high() 
        // {
        //     irq.wait_for_falling_edge().await;
        // }
        irq.wait_for_falling_edge().await;
    
        // 2. Glitch Filter: Wait 5ms for physical contact to stabilize
        embassy_time::Timer::after_millis(5).await;

        // 3. If the pin bounced back HIGH, it was SPI noise! Skip the read.
        if irq.is_high() {
            continue;
        }

        info!("Real Screen Touch Confirmed! Extracting coordinates...");
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

        // 4. Wait until the user completely LIFTS their finger off the screen
        while irq.is_low() 
        {
            embassy_time::Timer::after_millis(10).await;
        }
        info!("Touch Released!");

        // if raw_x < 4095 && raw_y < 4095 && raw_x > 100 && raw_y > 100 
        // {
        //     TOUCH_SIGNAL.signal(true);
        // }
        
        // embassy_time::Timer::after_millis(20).await; // Debounce delay to avoid multiple triggers from a single touch event
    }
}


// -------------------------------------------------------------------
// 2. Display Task
// -------------------------------------------------------------------
#[embassy_executor::task]
pub async fn touchscreen_display_task
(
    mut spi: DisplaySpiDevice,
    dc: Output<'static, PC7>,
    rst: Output<'static, PA9>,
) {
    info!("Initializing TouchScreen Display...");
    let mut delay = Delay;
    let spi_interface = SPIInterface::new(spi, dc);
    let builder = Builder::new(ILI9341Rgb565, spi_interface).reset_pin(rst).display_size(240, 320);

    let mut display = builder.init(&mut delay).unwrap();
    let mut ticker = Ticker::every(Duration::from_secs(1));
    
    // Define text styles for light and dark backgrounds
    let white_text = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
    let black_text = MonoTextStyle::new(&FONT_10X20, Rgb565::BLACK);

    // Color list to cycle through
    let colors = 
    [
        (Rgb565::RED, &white_text, "COLOR: RED"),
        (Rgb565::GREEN, &black_text, "COLOR: GREEN"),
        (Rgb565::BLUE, &white_text, "COLOR: BLUE"),
        (Rgb565::YELLOW, &black_text, "COLOR: YELLOW"),
        (Rgb565::MAGENTA, &white_text, "COLOR: MAGENTA"),
        (Rgb565::CYAN, &black_text, "COLOR: CYAN"),
    ];

    let mut color_idx = 0;

    loop 
    {
        let (bg_color, text_style, color_name) = colors[color_idx];

        // 1. Clear full screen with the current background color
        display.clear(bg_color).unwrap();

        // 2. Render status text to verify graphics rendering
        Text::new("DISPLAY TEST OK!", Point::new(30, 140), *text_style)
            .draw(&mut display)
            .unwrap();

        Text::new(color_name, Point::new(50, 170), *text_style)
            .draw(&mut display)
            .unwrap();

        info!("Display updated -> {}", color_name);

        // Advance to next color
        color_idx = (color_idx + 1) % colors.len();

        // Wait for 1 second before switching
        // ticker.next().await;
        embassy_time::Timer::after_millis(1000).await;
    }
}



// -------------------------------------------------------------------
// 3. SD Card Task
// -------------------------------------------------------------------
use core::fmt::Write;
use embedded_sdmmc::{Mode, SdCard, TimeSource, Timestamp, VolumeIdx, VolumeManager};
use heapless::String;

// 3.1. Create a Dummy TimeSource for FAT file timestamps
struct DummyTime;

impl TimeSource for DummyTime 
{
    fn get_timestamp(&self) -> Timestamp 
    {
        Timestamp {
            year_since_1970: 56,   // 1970 + 56 = 2026
            zero_indexed_month: 9, // October (0-indexed)
            zero_indexed_day: 23,  // 24th (0-indexed)
            hours: 14,
            minutes: 49,
            seconds: 0,
        }
    }
}


// 3.2. The SD Card Task
#[embassy_executor::task]
pub async fn sd_task(spi_device: SdSpiDevice) 
{
    info!("SD Task started");
    // Initialize the SD card block device
    let sdcard = SdCard::new(spi_device, Delay);
    let mut volume_mgr = VolumeManager::new(sdcard, DummyTime);
    let mut s_no = 1;

    loop 
    {
        // Attempt to mount the volume
        if let Ok(mut volume) = volume_mgr.open_volume(VolumeIdx(0)) 
        {
            info!("Volume mounted successfully!");
            // Call open_root_dir DIRECTLY on the volume object
            if let Ok(mut root_dir) = volume.open_root_dir() 
            {
                info!("Root directory opened successfully!");
                // Call open_dir DIRECTLY on the root directory object
                if let Ok(mut logs_dir) = root_dir.open_dir("LOGS") 
                {
                    info!("LOGS directory opened successfully!");
                    // Call open_file_in_dir DIRECTLY on the logs directory
                    if let Ok(mut file) = logs_dir.open_file_in_dir(
                        "LOG.CSV", 
                        Mode::ReadWriteCreateOrAppend
                    ) {
                        // Format and write the CSV row
                        let mut buffer: String<64> = String::new();
                        if core::write!(&mut buffer, "{},2026-10-24,14:49:00\n", s_no).is_ok() 
                        {
                            info!("Writing to LOG.CSV");
                            // Call write and flush DIRECTLY on the file object
                            let _ = file.write(buffer.as_bytes());
                            let _ = file.flush(); // Required to update FAT directory entry
                            info!("LOG.CSV updated successfully!");
                            s_no += 1;
                        }
                    }
                }
            }
        }
        
        // Wait 1 second before the next write
        Timer::after_secs(1).await;
    }
}