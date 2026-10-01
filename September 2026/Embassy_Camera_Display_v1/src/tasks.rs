use core::sync::atomic::{AtomicU8, Ordering};
use core::ptr::addr_of_mut;
use defmt::{info, warn};

use embassy_stm32::dcmi::Dcmi;
use embassy_stm32::peripherals::{DCMI, DMA2_CH1, SPI1, DMA2_CH3, PC4, PC5, PB1};
use embassy_stm32::spi::Spi;
use embassy_stm32::gpio::Output;
use embassy_stm32::dma::NoDma;
use embassy_time::{Delay, Timer};

use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::pixelcolor::raw::RawU16;

use display_interface_spi::SPIInterface;
use embedded_hal_bus::spi::ExclusiveDevice;
use mipidsi::Builder;
use mipidsi::models::ILI9341Rgb565;

use embassy_time::{ Duration};

pub const FRAME_WIDTH: u32 = 160;
pub const FRAME_HEIGHT: u32 = 120;
pub const FRAME_SIZE_WORDS: usize = 9600; // (160 * 120 * 2 / 4) + 2 padding = 38.4 KB

// Two global buffers (~76.8 KB total — fits in 128 KB RAM)
static mut BUF_A: [u32; FRAME_SIZE_WORDS] = [0; FRAME_SIZE_WORDS];
static mut BUF_B: [u32; FRAME_SIZE_WORDS] = [0; FRAME_SIZE_WORDS];

// Atomic signal: 0 = None, 1 = BUF_A ready, 2 = BUF_B ready
static READY_BUF: AtomicU8 = AtomicU8::new(0);

#[embassy_executor::task]
pub async fn camera_capture_task(mut dcmi: Dcmi<'static, DCMI, DMA2_CH1>) {
    info!("Starting Camera Capture Task...");
    let mut write_to_a = true;

    loop {
        let target_buf = unsafe {
            if write_to_a {
                &mut *addr_of_mut!(BUF_A)
            } else {
                &mut *addr_of_mut!(BUF_B)
            }
        };

        match dcmi.capture(target_buf).await {
            Ok(_) => {
                // Mark buffer as ready for rendering
                READY_BUF.store(if write_to_a { 1 } else { 2 }, Ordering::Release);
                write_to_a = !write_to_a; // Swap target buffer for next frame
            }
            Err(e) => {
                warn!("Capture overrun/error: {:?}. Retrying on next VSYNC.", defmt::Debug2Format(&e));
                Timer::after_millis(5).await; // Brief yield to let signals stabilize
            }
        }

        
        // Inside your loop:
        Timer::after(Duration::from_millis(100)).await;
    }
}

#[embassy_executor::task]
pub async fn display_render_task(
    spi: Spi<'static, SPI1, DMA2_CH3, NoDma>,
    cs: Output<'static, PC4>,
    dc: Output<'static, PC5>,
    rst: Output<'static, PB1>,
) {
    info!("Starting Display Render Task...");
    let mut delay = Delay;

    let spi_device = ExclusiveDevice::new(spi, cs, Delay).unwrap();
    let spi_interface = SPIInterface::new(spi_device, dc);

    let builder = Builder::new(ILI9341Rgb565, spi_interface)
        .reset_pin(rst)
        .display_size(240, 320);

    let mut display = builder.init(&mut delay).unwrap();
    
    // Centered 160x120 target area on 240x320 display
    let area = Rectangle::new(Point::new(40, 60), Size::new(FRAME_WIDTH, FRAME_HEIGHT));

    loop {
        // Check if a new frame is ready
        let ready = READY_BUF.swap(0, Ordering::Acquire);
        if ready == 0 {
            Timer::after_millis(2).await;
            continue;
        }

        let target_buf = unsafe {
            if ready == 1 {
                &mut *addr_of_mut!(BUF_A)
            } else {
                &mut *addr_of_mut!(BUF_B)
            }
        };

        // Get byte view of frame
        let raw_bytes: &mut [u8] = unsafe {
            core::slice::from_raw_parts_mut(
                target_buf.as_mut_ptr() as *mut u8,
                38400
            )
        };

        // Fast in-place byte swap for RGB565 endianness
        for chunk in raw_bytes.chunks_exact_mut(2) {
            chunk.swap(0, 1);
        }

        // Render buffer to screen
        let raw_u16_slice: &[RawU16] = unsafe {
            core::slice::from_raw_parts(
                raw_bytes.as_ptr() as *const RawU16,
                19200
            )
        };

        let colors = raw_u16_slice.iter().map(|&r| Rgb565::from(r));
        let _ = display.fill_contiguous(&area, colors);

        // Inside your loop:
        Timer::after(Duration::from_millis(100)).await;
    }
}