#![no_std]
#![no_main]

use {defmt_rtt as _, panic_probe as _};
use embassy_executor::Spawner;
use embassy_stm32::spi::{Config as SpiConfig, Spi};
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::time::Hertz;


mod tasks;
use tasks::sd_log_task;

#[embassy_executor::main]
async fn main(spawner: Spawner) 
{
    defmt::info!("=== SD Card Logger Starting ===");

    let mut config = embassy_stm32::Config::default();
    config.enable_debug_during_sleep = true;
    let p = embassy_stm32::init(config);

    // 1. Configure the Chip Select (CS) pin
    let cs_pin = Output::new(p.PB12, Level::High, Speed::VeryHigh);

// 2. Configure SPI2 for the SD Card
    let mut spi_config = SpiConfig::default();
    spi_config.frequency = Hertz(400_000); 

    let spi = Spi::new(
        p.SPI2, 
        p.PB13, // SCK
        p.PB15, // MOSI
        p.PB14, // MISO
        embassy_stm32::dma::NoDma, // <--- No TX DMA channel
        embassy_stm32::dma::NoDma, // <--- No RX DMA channel
        spi_config
    );
    // 3. Spawn the SD logging task, passing the SPI bus and CS pin
    spawner.spawn(sd_log_task(spi, cs_pin)).unwrap();

    loop {
        embassy_time::Timer::after_secs(10).await;
    }
}