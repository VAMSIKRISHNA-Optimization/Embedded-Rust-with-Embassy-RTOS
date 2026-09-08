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
        embassy_time::Timer::after_millis(150).await;
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
pub async fn uart_time_config_task(mut usart: BufferedUart<'static, USART2>) {
    info!("UART Task Started!");
    
    let boot_msg = b"\r\n=== STM32 Touch Display Booted ===\r\nType something...\r\n";
    if let Err(e) = usart.write_all(boot_msg).await {
        defmt::error!("UART TX failed: {:?}", e);
    } else {
        info!("UART Boot Banner Sent!");
    }

    let mut buf = [0u8; 1];
    loop 
    {
        match usart.read(&mut buf).await 
        {
            Ok(1) => {
                info!("UART RX Interrupt fired! Received byte: {:#04x}", buf[0]);
                let _ = usart.write_all(&buf).await;
            }
            Ok(_) => {}
            Err(e) => defmt::error!("UART error: {:?}", e),
        }
    }
}