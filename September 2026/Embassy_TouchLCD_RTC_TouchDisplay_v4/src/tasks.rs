use defmt::info;
use embassy_stm32::gpio::Output;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::spi::Spi;
use embassy_stm32::peripherals::{SPI1, SPI2, PB6, PB12, PC5, PC7, PA9, USART2};
use embassy_stm32::dma::NoDma;
use embassy_stm32::usart::BufferedUart;

// Alias core formatting trait for display text formatting
use core::fmt::Write as FmtWrite;

// Standard async I/O traits for UART operations
use embedded_io_async::{Read, Write};

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Rectangle, PrimitiveStyleBuilder},
    text::Text,
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
};
use display_interface_spi::SPIInterface;
use mipidsi::Builder;
use mipidsi::models::ILI9341Rgb565;
use embassy_time::{Delay, Duration, Ticker};

use crate::{TOUCH_SIGNAL, I2C1_BUS};

pub const SD3078_ADDRESS: u8 = 0x32;

#[derive(Default)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub min: u8,
    pub sec: u8,
}

pub fn bcd_to_dec(val: u8) -> u8 {
    ((val / 16) * 10) + (val % 16)
}

// -------------------------------------------------------------------
// 1. Touch Task (Handles edge transitions + pre-existing low state)
// -------------------------------------------------------------------
#[embassy_executor::task]
pub async fn touchscreen_touch_task(
    mut spi: Spi<'static, SPI2, NoDma, NoDma>,
    mut cs: Output<'static, PB12>,
    mut irq: ExtiInput<'static, PC5>, 
) {
    // embassy_time::Timer::after_millis(500).await;
    info!("Touch Task Started!");

    // Initial SPI command to wake up XPT2046 and set PENIRQ active (PD1=0, PD0=0)
    cs.set_low();
    let mut dummy = [0u8; 3];
    let _ = spi.blocking_transfer(&mut dummy, &[0x90, 0x00, 0x00]);
    cs.set_high();

    info!("Touch Task Initialized. Listening for touch events...");

    loop 
    {
        // If pin is HIGH, wait for falling edge.
        // If pin is ALREADY LOW (held touch or booted low), proceed immediately.
        if irq.is_high() 
        {
            irq.wait_for_falling_edge().await;
        }

        info!("Screen Touched! Extracting coordinates...");

        cs.set_low();
        let mut x_buf = [0u8; 3];
        let mut y_buf = [0u8; 3];

        let _ = spi.blocking_transfer(&mut x_buf, &[0x90, 0x00, 0x00]);
        let _ = spi.blocking_transfer(&mut y_buf, &[0xD0, 0x00, 0x00]);
        cs.set_high();

        let raw_x = (((x_buf[1] as u16) << 8) | (x_buf[2] as u16)) >> 3;
        let raw_y = (((y_buf[1] as u16) << 8) | (y_buf[2] as u16)) >> 3;

        info!("Raw Touch -> X: {}, Y: {}", raw_x, raw_y);

        if raw_x > 100 && raw_y > 100 
        {
            TOUCH_SIGNAL.signal(true);
        }
        
        // Debounce delay to prevent task thrashing
        embassy_time::Timer::after_millis(1500).await;
    }
}


// -------------------------------------------------------------------
// 2. Display Task
// -------------------------------------------------------------------
#[embassy_executor::task]
pub async fn touchscreen_display_task(
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
        info!("Starting Display Refresh!");
        let mut current_time = DateTime::default();
        let mut is_valid = false;

        {
            let mut bus_guard = I2C1_BUS.lock().await;
            if let Some(ref mut i2c) = *bus_guard {
                let mut read_buf = [0u8; 7];
                if i2c.blocking_write_read(SD3078_ADDRESS, &[0x00], &mut read_buf).is_ok() 
                {
                    current_time.sec    = bcd_to_dec(read_buf[0] & 0x7F);
                    current_time.min    = bcd_to_dec(read_buf[1] & 0x7F);
                    current_time.hour   = bcd_to_dec(read_buf[2] & 0x3F);
                    current_time.day    = bcd_to_dec(read_buf[4] & 0x3F);
                    current_time.month  = bcd_to_dec(read_buf[5] & 0x1F);
                    current_time.year   = 2000 + bcd_to_dec(read_buf[6]) as u16;

                    if current_time.year >= 2026 
                    {
                        is_valid = true;
                    }
                }
            }
        }

        if TOUCH_SIGNAL.signaled() 
        {
            TOUCH_SIGNAL.reset();
            info!("Display Task Woke Up! Changing color...");
            bg_color = if bg_color == Rgb565::BLACK { Rgb565::BLUE } else { Rgb565::BLACK };
            display.clear(bg_color).unwrap();
        }

        let mut time_str: heapless::String<32> = heapless::String::new();
        if is_valid 
        {
            let _ = FmtWrite::write_fmt
            (
                &mut time_str,
                format_args!
                (
                    "{:02}-{:02}-{:04} {:02}:{:02}:{:02}",
                    current_time.day, current_time.month, current_time.year,
                    current_time.hour, current_time.min, current_time.sec
                ),
            );
        } else {
            let _ = FmtWrite::write_str(&mut time_str, "RTC DATE & TIME NOT SET");
        }

        let text_bg = PrimitiveStyleBuilder::new().fill_color(bg_color).build();
        Rectangle::new(Point::new(0, 140), Size::new(240, 40))
            .into_styled(text_bg)
            .draw(&mut display)
            .unwrap();

        let x_pos = if is_valid { (240 - 190) / 2 } else { 5 };
        Text::new(&time_str, Point::new(x_pos as i32, 160), text_style)
            .draw(&mut display)
            .unwrap();

        info!("Display Refresh: DONE!");
        embassy_time::Timer::after_millis(1000).await;
        // ticker.next().await;
    }
}

// -------------------------------------------------------------------
// 3. UART Time Config Task
// -------------------------------------------------------------------
#[embassy_executor::task]
pub async fn uart_time_config_task(mut usart: BufferedUart<'static, USART2>) 
{
    info!("UART Task Started!");
    use crate::RTC_SET_SIGNAL;
    
    let boot_msg = b"\r\n Please set date and time! \r\n Please follow the format \"SET DD-MM-YYYY HH:MM:SS\" \r\n  Example: \"SET 10-09-2026 15:45:02\" \r\n ";
    if let Err(e) = usart.write_all(boot_msg).await 
    {
        defmt::error!("UART TX failed: {:?}", e);
    } else {
        info!("UART Boot Banner Sent!");
    }

    let mut uart_buf     = [0u8; 1];
    let mut message_buf = [0_u8; 64];
    let mut length_tracker: usize = 0;
    loop 
    {
        match usart.read(&mut uart_buf).await 
        {
            Ok(1) => 
            {
                info!("UART RX Interrupt fired! Received byte: {:#04x}", uart_buf[0]);
                let received_byte = uart_buf[0];

                match received_byte
                {
                    b'\r' | b'\n' => 
                    {
                        if length_tracker > 0
                        {
                            info!("Command received! Length: {}", length_tracker);
                            
                            let command = &message_buf[..length_tracker];

                            if let Ok(command_str) = core::str::from_utf8(command)
                            {
                                let mut parts = command_str.split_whitespace();
                                // Check if the first token is "SET"
                                if parts.next() == Some("SET") 
                                {
                                    let date_part = parts.next(); // e.g. Some("10-09-2026")
                                    let time_part = parts.next(); // e.g. Some("15:45:02")
                                    
                                    match (date_part, time_part) 
                                    {
                                        (Some(date), Some(time)) => 
                                        {
                                            info!("Valid SET command structure: Date={}, Time={}", date, time);
                                            let mut DateTime_RTC_Payload: [u8; 8] = [0; 8];

                                            let mut date_str_iter = date.split('-');
                                            let extracted_date_parts: [&str; 3] = [date_str_iter.next().unwrap_or(""), date_str_iter.next().unwrap_or(""), date_str_iter.next().unwrap_or("")];
                                            
                                            match parse_date(extracted_date_parts)
                                            {
                                                Ok((d, m, y)) => 
                                                {
                                                    info!("Parsed Date: Day={}, Month={}, Year={}", d, m, y);

                                                    DateTime_RTC_Payload[5] = dec_to_bcd(d);
                                                    DateTime_RTC_Payload[6] = dec_to_bcd(m);
                                                    DateTime_RTC_Payload[7] = dec_to_bcd((y % 100) as u8);

                                                }
                                                Err(e)       =>  defmt::error!("DATE PARSING ERROR: {}", e),
                                            }

                                            
                                            let mut time_str_iter = time.split(':');
                                            let extracted_time_parts: [&str; 3] = [time_str_iter.next().unwrap_or(""), time_str_iter.next().unwrap_or(""), time_str_iter.next().unwrap_or("")];
                                            
                                            match parse_time(extracted_time_parts)
                                            {
                                                Ok((h, m, s))  => 
                                                {
                                                    info!("Parsed Time: Hour={}, Minute={}, Second={}", h, m, s);

                                                    DateTime_RTC_Payload[1] = dec_to_bcd(s);
                                                    DateTime_RTC_Payload[2] = dec_to_bcd(m);
                                                    DateTime_RTC_Payload[3] = dec_to_bcd(h);
                                                },
                                                Err(e)       =>  defmt::error!("TIME PARSING ERROR: {}", e),
                                            }

                                            // Write the parsed date and time to the RTC
                                            {
                                                let mut bus_guard = I2C1_BUS.lock().await;
                                                if let Some(ref mut i2c) = *bus_guard 
                                                {
                                                    // 1. Chain the fallible operations. If any step fails, the rest are safely skipped.
                                                        let transaction_result = i2c.blocking_write(SD3078_ADDRESS, &[0x10, 0x80])
                                                            .and_then(|_| i2c.blocking_write(SD3078_ADDRESS, &[0x0F, 0x84]))
                                                            .and_then(|_| i2c.blocking_write(SD3078_ADDRESS, &DateTime_RTC_Payload));

                                                        // 2. Evaluate the final result of the entire chain
                                                        if transaction_result.is_ok() 
                                                        {
                                                            defmt::info!("RTC Date and Time successfully updated!");
                                                            // Notify main that the RTC is ready
                                                            RTC_SET_SIGNAL.signal(());
                                                        } 
                                                        else 
                                                        {
                                                            defmt::error!("Failed to complete RTC write transaction!");
                                                        }

                                                        // 3. ALWAYS re-lock the RTC, regardless of transaction success or failure
                                                        // We use `let _ =` to explicitly ignore the Result here, as we are already cleaning up
                                                        let _ = i2c.blocking_write(SD3078_ADDRESS, &[0x10, 0x00]);
                                                        let _ = i2c.blocking_write(SD3078_ADDRESS, &[0x0F, 0x00]);
                                                        defmt::info!("RTC Write Protection Re-Enabled.");
                                                }
                                            }


                                        }
                                        _ => defmt::warn!("Missing date or time arguments! Format: SET DD-MM-YYYY HH:MM:SS"),
                                    }
                                } else 
                                {
                                    defmt::warn!("Unknown command prefix! Expected 'SET'");
                                }

                            }
                            else 
                            {
                                defmt::error!("Failed to convert command to string!");
                            }
                            
                            // Reset tracker for the next command
                            length_tracker = 0;

                        }
                    }
                    other_char => 
                    {
                        if length_tracker < message_buf.len() 
                        {
                            message_buf[length_tracker] = other_char;
                            length_tracker += 1;
                        } else 
                        {
                            defmt::warn!("Buffer full! Clearing unhandled command...");
                            length_tracker = 0;
                        }
                    }
                }
                
            }
            Ok(_) => {}
            Err(e) => defmt::error!("UART error: {:?}", e),
        }

        if length_tracker == 25
        {
            
        }

        if length_tracker >= message_buf.len()
        {
            length_tracker = 0;
            message_buf    = [0; 64];  
        }
        
    }


}


fn parse_date(date_arr:[&str; 3]) -> Result<(u8, u8, u16), & 'static str>
{
    if let Ok(year_val) = date_arr[2].parse::<u16>()
    {
        if year_val < 1 || year_val > 5000 
        {
            return Err("Invalid Year, Year must be between 1 to 5000");
        } 

        if let Ok(month_val) = date_arr[1].parse::<u8>()
        {

            if month_val < 1 || month_val > 12 
            {
                return Err("Invalid Month, Month must be between 1 to 12");
            }

            if let Ok(day_val) = date_arr[0].parse::<u8>()
            {
                if !is_leap_year(year_val) && month_val == 2 && day_val > 28
                {
                    return Err("Invalid Day, Day must be between 1 to 28 for February as the given year is not a leap year");
                }

                if is_leap_year(year_val) && month_val == 2 && day_val > 29 
                {
                    return Err("Invalid Day, Day must be between 1 to 29 for February as the given year is a leap year");
                }

                if month_val == 4 || month_val == 6 || month_val == 9 || month_val == 11 
                {
                    if day_val > 30 
                    {
                        return Err("Invalid Day, Day must be between 1 to 30 for the given month");
                    }
                }

                if day_val < 1 || day_val > 31 
                {
                    return Err("Invalid Day, Day must be between 1 to 31");
                }

                return Ok((day_val, month_val, year_val));

            }
            else 
            {
                return Err("Invalid Day, Day must be numeric (between 1 to 31)");
            }

        }
        else 
        {
            return Err("Invalid Month, Month must be numeric (between 1 to 12)");
        }

    }
    else 
    {
        return Err("Invalid Year, Year must be numeric (between 1 to 5000)");
    }
}


fn is_leap_year(year: u16) -> bool 
{
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}


fn parse_time(time_arr:[&str; 3]) -> Result<(u8, u8, u8), & 'static str>
{
    if let Ok(hour_val) = time_arr[0].parse::<u8>()
    {
        if hour_val > 23 
        {
            return Err("Invalid Hour, Hour must be between 0 to 23");
        }

        if let Ok(minute_val) = time_arr[1].parse::<u8>()
        {
            if minute_val > 59 
            {
                return Err("Invalid Minute, Minute must be between 0 to 59");
            }

            if let Ok(second_val) = time_arr[2].parse::<u8>()
            {
                if second_val > 59 
                {
                    return Err("Invalid Second, Second must be between 0 to 59");
                }

                return Ok((hour_val, minute_val, second_val));

            }
            else 
            {
                return Err("Invalid Second, Second must be numeric (between 0 to 59)");
            }

        }
        else 
        {
            return Err("Invalid Minute, Minute must be numeric (between 0 to 59)");
        }

    }
    else 
    {
        return Err("Invalid Hour, Hour must be numeric (between 0 to 23)");
    }
}

fn dec_to_bcd(val: u8) -> u8
{
    ((val / 10) << 4) | (val % 10)
}