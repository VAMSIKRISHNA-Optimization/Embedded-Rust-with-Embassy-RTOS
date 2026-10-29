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
static TOUCH_CHANNEL: Channel<CriticalSectionRawMutex, (u16, u16), 2> = Channel::new();

// Global channel for LOG events
static SD_CHANNEL: Channel<CriticalSectionRawMutex, heapless::String<10>, 2> = Channel::new();


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
// 2. Display Task (Numeric Keypad Version)
// -------------------------------------------------------------------
#[embassy_executor::task]
pub async fn touchscreen_display_task
(
    mut spi: DisplaySpiDevice, 
    dc: Output<'static, PC7>,
    rst: Output<'static, PA9>,
) 
{
    info!("Initializing TouchScreen Display for Keypad...");

    let mut delay = Delay;
    let spi_interface = SPIInterface::new(spi, dc);

    use mipidsi::options::{Orientation, Rotation};
    let builder = Builder::new(ILI9341Rgb565, spi_interface)
        .reset_pin(rst)
        .display_size(240, 320)
        .orientation(Orientation::new().rotate(Rotation::Deg180)); // Flips the display 180 degrees

    let mut display = builder.init(&mut delay).unwrap();

    // 1. Clear the screen with a black background
    display.clear(Rgb565::BLACK).unwrap();

        // 1.1 Calibrate the touch using squares on the display
        Text::new("TOUCH CALIBRATION - 1 ",Point::new(20, 320/2),
                    U8g2TextStyle::new(fonts::u8g2_font_helvB12_tf, Rgb565::WHITE),)
                .draw(&mut display)
                .unwrap();
        
        // 1.2 First White Square at (0,0)
        Rectangle::new(Point::new(0, 0), Size::new(10, 10))
            .into_styled(PrimitiveStyleBuilder::new().fill_color(Rgb565::new(255, 255, 255)).build())
            .draw(&mut display)
            .unwrap();

        // 1.3 Awaiting user's touch
        let (raw_x1, raw_y1) = TOUCH_CHANNEL.receive().await;

        display.clear(Rgb565::BLACK).unwrap();

        // 1.4 Calibrate the touch using squares on the display
        Text::new("TOUCH CALIBRATION - 2 ",Point::new(40, 320/2),
                    U8g2TextStyle::new(fonts::u8g2_font_helvB12_tf, Rgb565::WHITE),)
                .draw(&mut display)
                .unwrap();
        
        // 1.5 Second White Square at (230, 310)
        Rectangle::new(Point::new(230, 310), Size::new(10, 10))
            .into_styled(PrimitiveStyleBuilder::new().fill_color(Rgb565::new(255, 255, 255)).build())
            .draw(&mut display)
            .unwrap();

        // 1.6 Awaiting user's touch
        let (raw_x2, raw_y2) = TOUCH_CHANNEL.receive().await;

        display.clear(Rgb565::BLACK).unwrap();

        // 1.7 Establishing touch boundaries
        // let min_raw_x = raw_x1.min(raw_x2);
        // let max_raw_x = raw_x1.max(raw_x2);
        // let min_raw_y = raw_y1.min(raw_y2);
        // let max_raw_y = raw_y1.max(raw_y2);


    // 2. Define text and button border styles
    use u8g2_fonts::{fonts, U8g2TextStyle};
    // let text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
    let text_style     = U8g2TextStyle::new(fonts::u8g2_font_helvB24_tf, Rgb565::WHITE);
    let button_style  = PrimitiveStyleBuilder::new()
        .stroke_color(Rgb565::WHITE)
        .stroke_width(2)
        .fill_color(Rgb565::new(8, 16, 8)) // Dark green/gray fill
        .build();

    // 3. Define the keypad grid mapping
    let keys = 
    [
        ["1", "2", "3"],
        ["4", "5", "6"],
        ["7", "8", "9"],
        ["*", "0", "#"],
    ];

    let key_width : i32 = 80;
    let key_height: i32 = 50;
    let start_y   : i32 = 120; // Top 120px left blank for viewing pressed numbers

    // 4. Draw the Keypad
    for (row_idx, row) in keys.iter().enumerate() 
    {
        for (col_idx, key_str) in row.iter().enumerate() 
        {
            let x = (col_idx as i32) * key_width;
            let y = start_y + (row_idx as i32) * key_height;

            // Draw button rectangle
            Rectangle::new(Point::new(x, y), Size::new(key_width as u32, key_height as u32))
                .into_styled(button_style)
                .draw(&mut display)
                .unwrap();

            // Draw text roughly centered in the 80x50 button
            // FONT_10X20 is 10px wide, 20px high. 
            // X offset: (80 - 10) / 2 = 35
            // Y offset: baseline is at the bottom of the text. (50 / 2) + (20 / 2) = 35
            Text::new(
                key_str,
                Point::new(x + 35, y + 35),
                text_style.clone(),
            )
            .draw(&mut display)
            .unwrap();
        }
    }

        // 4.1 Draw the DEL and LOG buttons
        Rectangle::new(Point::new(2*key_width, 0), Size::new(key_width as u32, key_height as u32))
                .into_styled(button_style)
                .draw(&mut display)
                .unwrap();
        
        Text::new(
                "DEL",
                Point::new(2*key_width + 9, 35),
                text_style.clone(),
            )
            .draw(&mut display)
            .unwrap();

        Rectangle::new(Point::new(2*key_width, key_height), Size::new(key_width as u32, key_height as u32))
                .into_styled(button_style)
                .draw(&mut display)
                .unwrap();
        
        Text::new(
                "LOG",
                Point::new(2*key_width + 5, key_height + 35),
                text_style.clone(),
            )
            .draw(&mut display)
            .unwrap();


    info!("Numeric Keypad Rendered!");

        // Clear the top 120px display area
        Rectangle::new(Point::new(0, 0), Size::new(240-80, 120))
            .into_styled(PrimitiveStyleBuilder::new().fill_color(Rgb565::BLACK).build())
            .draw(&mut display)
            .unwrap();

    let mut key_press_count: u32 = 0;
    let mut x = 0;
    let mut y = 0;



    // 5. Idle loop
    let mut phone_number_buf: heapless::String<10> = heapless::String::new();
    
    // Smaller font for the 10-digit display
    let input_text_style = U8g2TextStyle::new(fonts::u8g2_font_helvB12_tf, Rgb565::WHITE);

    loop 
    {
        let (raw_x, raw_y) = TOUCH_CHANNEL.receive().await;

        let rx = raw_x as i32; let rx1 = raw_x1 as i32; let rx2 = raw_x2 as i32;
        let ry = raw_y as i32; let ry1 = raw_y1 as i32; let ry2 = raw_y2 as i32;

        let mut pixel_x = ((ry - ry1) * 230) / (ry2 - ry1);
        let mut pixel_y = ((rx - rx1) * 310) / (rx2 - rx1);

        pixel_x = pixel_x.clamp(0, 239);
        pixel_y = pixel_y.clamp(0, 319);

        let mut update_display = false;

        // 5.3 Hit Detection Logic
        if pixel_y >= 120 
        {
            // Keypad Zone
            let col = pixel_x / 80;
            let row = (pixel_y - 120) / 50;

            if col < 3 && row < 4 && phone_number_buf.len() < 10 
            {
                let pressed_key = keys[row as usize][col as usize];
                if phone_number_buf.push_str(pressed_key).is_ok() 
                {
                    info!("Added: {}", pressed_key);
                    update_display = true;
                }
            }
        } else if pixel_x >= 160 
        {
            // Action Buttons Zone (Right side, top 120px)
            if pixel_y < 50 
            {
                // DEL Button
                if phone_number_buf.pop().is_some() 
                {
                    info!("DEL pressed");
                    update_display = true;
                }
            } else if pixel_y < 100 {
                // LOG Button
                if !phone_number_buf.is_empty() 
                {
                    info!("LOG pressed, sending to SD Card...");
                    // Send to SD card task
                    SD_CHANNEL.send(phone_number_buf.clone()).await;
                    phone_number_buf.clear();
                    update_display = true;
                }
            }
        }

        // 5.4 Redraw the text area only if a valid button was pressed
        if update_display 
        {
            // Clear just the text display area (x: 0, y: 0, width: 160, height: 120)
            Rectangle::new(Point::new(0, 0), Size::new(160, 120))
                .into_styled(PrimitiveStyleBuilder::new().fill_color(Rgb565::BLACK).build())
                .draw(&mut display)
                .unwrap();

            // Render the updated string vertically centered
            Text::new(
                phone_number_buf.as_str(),
                Point::new(10, 65),
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