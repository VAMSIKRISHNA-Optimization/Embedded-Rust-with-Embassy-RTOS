use defmt::info;
use embassy_stm32::gpio::Output;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::spi::Spi;
use embassy_stm32::peripherals::{SPI1, SPI2, PB0, PB6, PB12, PC5, PC7, PA9};
use embassy_stm32::dma::NoDma;

use core::cell::RefCell;
use core::f32::consts::E;

use embassy_embedded_hal::shared_bus::blocking::spi::SpiDeviceWithConfig;
use embassy_sync::blocking_mutex::raw::{NoopRawMutex, CriticalSectionRawMutex};
use embassy_sync::channel::Channel;

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

// Global channel for touch events
pub static TOUCH_CHANNEL: Channel<CriticalSectionRawMutex, (u16, u16), 2> = Channel::new();

// Global channel for LOG events
static SD_CHANNEL: Channel<CriticalSectionRawMutex, heapless::String<256>, 2> = Channel::new();


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

        TOUCH_CHANNEL.send((raw_x, raw_y)).await;

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
// 2. Display Task (QWERTY Keyboard Version)
// -------------------------------------------------------------------
#[embassy_executor::task]
pub async fn touchscreen_display_task(
    mut spi: DisplaySpiDevice, 
    dc: Output<'static, PC7>,
    rst: Output<'static, PA9>,
) 
{
    info!("Initializing TouchScreen Display for QWERTY...");

    let mut delay = Delay;
    let spi_interface = SPIInterface::new(spi, dc);

    use mipidsi::options::{Orientation, Rotation};
    let builder = Builder::new(ILI9341Rgb565, spi_interface)
        .reset_pin(rst)
        .display_size(240, 320)
        .orientation(Orientation::new().rotate(Rotation::Deg90)); // Landscape

    let mut display = builder.init(&mut delay).unwrap();

    // 1. Display Touch Calibration
    use crate::display::display_touch_calibration::display_touch_calibration;
    let (raw_x1, raw_y1, raw_x2, raw_y2) = display_touch_calibration(&mut display).await;

    // 2. Render the QWERTY Keyboard
    use crate::display::qwerty_keyboard::render_qwerty_keyboard;
    render_qwerty_keyboard(&mut display).await;

    // 3. Clear the top 80px text viewport
    Rectangle::new(Point::new(0, 0), Size::new(320, 80))
        .into_styled(PrimitiveStyleBuilder::new().fill_color(Rgb565::BLACK).build())
        .draw(&mut display)
        .unwrap();

    // 4. Expanded buffer for longer messages
    let mut text_buf: heapless::String<256> = heapless::String::new();
    use u8g2_fonts::{fonts, U8g2TextStyle};
    let input_text_style = U8g2TextStyle::new(fonts::u8g2_font_helvB12_tf, Rgb565::WHITE);

    // 5. Idle loop
    loop 
    {
        let (raw_x, raw_y) = TOUCH_CHANNEL.receive().await;

        let rx = raw_x as i32; let rx1 = raw_x1 as i32; let rx2 = raw_x2 as i32;
        let ry = raw_y as i32; let ry1 = raw_y1 as i32; let ry2 = raw_y2 as i32;

        // 1. Map Raw X to Pixel X (using the 310 calibration distance)
        let mut pixel_x = ((rx - rx1) * 310) / (rx2 - rx1);
        
        // 2. Map Raw Y to Pixel Y (using the 230 calibration distance)
        let mut pixel_y = ((ry - ry1) * 230) / (ry2 - ry1);

        pixel_x = pixel_x.clamp(0, 319);
        pixel_y = pixel_y.clamp(0, 239);

        let mut update_display = false;
        let mut char_to_add: Option<&str> = None;

        // 5.3 QWERTY Hit Detection Logic
        if pixel_y >= 80 && pixel_y < 112 
        {
            // Row 1: Numbers
            let col = (pixel_x / 32) as usize;
            let row1 = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
            if col < 10 { char_to_add = Some(row1[col]); }

        } else if pixel_y >= 112 && pixel_y < 144 {
            // Row 2: QWERTY
            let col = (pixel_x / 32) as usize;
            let row2 = ["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"];
            if col < 10 { char_to_add = Some(row2[col]); }

        } else if pixel_y >= 144 && pixel_y < 176 {
            // Row 3: ASDF... (Starts at 16px offset)
            if pixel_x >= 16 && pixel_x < 304 
            {
                let col = ((pixel_x - 16) / 32) as usize;
                let row3 = ["A", "S", "D", "F", "G", "H", "J", "K", "L"];
                if col < 9 { char_to_add = Some(row3[col]); }
            }

        } else if pixel_y >= 176 && pixel_y < 208 {
            // Row 4: UP, ZXCVBNM, DEL
            if pixel_x < 48 
            {
                info!("UP scroll pressed");
            } else if pixel_x < 272 {
                let col = ((pixel_x - 48) / 32) as usize;
                let row4 = ["Z", "X", "C", "V", "B", "N", "M"];
                if col < 7 { char_to_add = Some(row4[col]); }
            } else {
                if text_buf.pop().is_some() 
                {
                    info!("DEL pressed");
                    update_display = true;
                }
            }

        } else if pixel_y >= 208 {
            // Row 5: DOWN, LEFT, RIGHT, SPACE, PERIOD, LOG
            if pixel_x < 48 
            {
                info!("DOWN scroll pressed");
            } else if pixel_x < 80 {
                info!("LEFT cursor pressed");
            } else if pixel_x < 112 {
                info!("RIGHT cursor pressed");
            } else if pixel_x < 240 {
                char_to_add = Some(" "); // SPACE
            } else if pixel_x < 272 {
                char_to_add = Some("."); // PERIOD
            } else {
                if !text_buf.is_empty() 
                {
                    info!("LOG pressed, sending to SD Card...");
                    SD_CHANNEL.send(text_buf.clone()).await;
                    text_buf.clear();
                    update_display = true;
                }
            }
        }

        // Apply character addition if a valid key was hit
        if let Some(c) = char_to_add {
            if text_buf.push_str(c).is_ok() {
                info!("Added: {}", c);
                update_display = true;
            }
        }

        // 5.4 Redraw the text area
        if update_display {
            // Clear the 320x80 text viewport
            Rectangle::new(Point::new(0, 0), Size::new(320, 80))
                .into_styled(PrimitiveStyleBuilder::new().fill_color(Rgb565::BLACK).build())
                .draw(&mut display)
                .unwrap();

            // Render the updated string
            Text::new(
                text_buf.as_str(),
                Point::new(5, 20), // Adjusted top-left for 12pt font wrapping
                input_text_style.clone(),
            )
            .draw(&mut display)
            .unwrap();
        }
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
    let sdcard = SdCard::new(spi_device, Delay);
    let mut volume_mgr = VolumeManager::new(sdcard, DummyTime);
    let mut s_no = 1;

    loop 
    {
        // Block the loop until the display task sends a finalized number
        let phone_number = SD_CHANNEL.receive().await;
        info!("Received number for SD logging: {}", phone_number.as_str());

        // Attempt to mount the volume and open the file
        if let Ok(mut volume) = volume_mgr.open_volume(VolumeIdx(0)) 
        {
            if let Ok(mut root_dir) = volume.open_root_dir() 
            {
                if let Ok(mut logs_dir) = root_dir.open_dir("LOGS") 
                {
                    if let Ok(mut file) = logs_dir.open_file_in_dir(
                        "LOG.CSV", 
                        Mode::ReadWriteCreateOrAppend
                    ) {
                        // Write the CSV header only if the file was just created (length is 0)
                        if file.length() == 0 
                        {
                            info!("New file detected, writing CSV header...");
                            let _ = file.write(b"S.No,Phone Number,Date,Time\n");
                        }

                        // Format and write the data row
                        let mut buffer: String<64> = String::new();
                        if core::write!(&mut buffer, "{},{},2026-10-29,10:14:00\n", s_no, phone_number.as_str()).is_ok() 
                        {
                            info!("Writing data row to LOG.CSV...");
                            let _ = file.write(buffer.as_bytes());
                            let _ = file.flush(); // Required to update FAT directory entry
                            info!("LOG.CSV updated successfully!");
                            s_no += 1;
                        }
                    }
                }
            }
        }
    }
}