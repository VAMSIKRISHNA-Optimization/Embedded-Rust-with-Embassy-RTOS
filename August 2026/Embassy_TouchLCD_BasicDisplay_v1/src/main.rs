#![no_std]
#![no_main]

use {defmt_rtt as _, panic_probe as _};
use embassy_executor::Spawner;
use embassy_stm32::spi::{Config as SpiConfig, Spi, MODE_0, MODE_1, MODE_2, MODE_3};
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::time::Hertz;


mod tasks;
use tasks::touchscreen_display_task;

#[embassy_executor::main]
async fn main(spawner: Spawner) 
{
    defmt::info!("=== TouchScreen Display Starting ===");
    

    // Initialize the Embassy STM32 HAL
    let mut config = embassy_stm32::Config::default();
    config.enable_debug_during_sleep = true;
    let p = embassy_stm32::init(config);

    // 1. Configure the Chip Select (CS) pin
    let cs_pin = Output::new(p.PB6, Level::High, Speed::VeryHigh);

    // 2. Configure the Data/Command (DC) pin
    let dc_pin = Output::new(p.PC7, Level::Low, Speed::VeryHigh);

    // 3. Configure the Reset (RST) pin
    let rst_pin = Output::new(p.PA9, Level::High, Speed::VeryHigh);

    // 4. Configure SPI2 for the TouchScreen Display
    let mut spi_config = SpiConfig::default();
    spi_config.frequency       = Hertz(16_000_000); // 16 MHz
    spi_config.mode            = MODE_0;

    let spi = Spi::new(
        p.SPI1, 
        p.PA5, // SCK
        p.PA7, // MOSI
        p.PA6, // MISO
        embassy_stm32::dma::NoDma, // <--- No TX DMA channel
        embassy_stm32::dma::NoDma, // <--- No RX DMA channel
        spi_config
    );

    // 3. Spawn the TouchScreen display task, passing the SPI bus and pins
    spawner.spawn(touchscreen_display_task(spi, cs_pin, dc_pin, rst_pin)).unwrap();

    loop 
    {
        embassy_time::Timer::after_secs(10).await;
    }
}