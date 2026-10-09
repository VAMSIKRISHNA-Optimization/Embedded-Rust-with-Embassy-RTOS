#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;

use embassy_stm32::dma::NoDma;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::exti::{ExtiInput};


use embassy_stm32::spi::{Config as SpiConfig, Spi, MODE_0};
use embassy_stm32::time::Hertz;


use embassy_stm32::Config;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use {defmt_rtt as _, panic_probe as _};


//
mod tasks;

// Signals and Mutexes
pub static TOUCH_SIGNAL : Signal<CriticalSectionRawMutex, bool> = Signal::new();


#[embassy_executor::main]
async fn main(spawner: Spawner) 
{
    let mut config              = Config::default();
    config.enable_debug_during_sleep    = true;

    let p = embassy_stm32::init(config);

    info!("=== System Starting ===");

    // 1. Configure Display SPI (SPI1)
    let cs_pin  = Output::new(p.PB6, Level::High, Speed::VeryHigh);
    let dc_pin  = Output::new(p.PC7, Level::Low, Speed::VeryHigh);
    let rst_pin = Output::new(p.PA9, Level::High, Speed::VeryHigh);

    let mut spi_display_config  = SpiConfig::default();
    spi_display_config.frequency        = Hertz(16_000_000);
    spi_display_config.mode             = MODE_0;

    let display_spi = Spi::new
    (
        p.SPI1, 
        p.PA5, // SCK  (Serial Clock)
        p.PA7, // MOSI (Master Out Slave In - STM32 sending to Display)
        p.PA6, // MISO (Master In Slave Out - STM32 reading from Display)
        NoDma, NoDma,
        spi_display_config,
    );

    // 2. Configure Touch SPI (SPI2) + EXTI
    let touch_pin_input     = Input::new(p.PC5, Pull::None);
    let touch_irq_pin   = ExtiInput::new(touch_pin_input, p.EXTI5);
    let touch_cs_pin      = Output::new(p.PB12, Level::High, Speed::VeryHigh);
    let mut spi_touch_config        = SpiConfig::default();
    spi_touch_config.frequency              = Hertz(1_000_000);
    spi_touch_config.mode                   = MODE_0;


    let touch_spi = Spi::new
    (
        p.SPI2, 
        p.PB13, // SCK  (Serial Clock)
        p.PB15, // MOSI DIN(Master Out Slave In - STM32 sending to Touch chip)
        p.PB14, // MISO DOUT(Master In Slave Out - STM32 reading from Touch chip)
        NoDma, NoDma,
        spi_touch_config,
    );


    // Spawn all tasks
    info!("Spawn touch");
    spawner.spawn(tasks::touchscreen_touch_task(touch_spi, touch_cs_pin, touch_irq_pin)).unwrap();
    // embassy_time::Timer::after_millis(10).await;

    // info!("Spawn display");
    // spawner.spawn(tasks::touchscreen_display_task(display_spi, cs_pin, dc_pin, rst_pin)).unwrap();
    // embassy_time::Timer::after_millis(10).await;

    // Keep main task active on executor (matching v2)
    loop 
    {
        embassy_time::Timer::after_secs(1000).await;
    }
}