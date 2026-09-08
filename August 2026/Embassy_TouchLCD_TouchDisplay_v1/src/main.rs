#![no_std]
#![no_main]

use {defmt_rtt as _, panic_probe as _};
use embassy_executor::Spawner;
use embassy_stm32::spi::{Config as SpiConfig, Spi, MODE_0};
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::time::Hertz;

use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Input, Pull};

use embassy_sync::signal::Signal;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;


mod tasks;
use tasks::touchscreen_display_task;

// A global signal to notify the display task that a touch occurred
pub static TOUCH_SIGNAL: Signal<CriticalSectionRawMutex, bool> = Signal::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) 
{
    defmt::info!("=== TouchScreen Display Starting ===");
    

    // Initialize the Embassy STM32 HAL
    let mut config = embassy_stm32::Config::default();
    config.enable_debug_during_sleep = true;
    let p = embassy_stm32::init(config);

    // 1. Configure the Display SPI bus (SPI1) for the TouchScreen Display
        // 1.1 Configure the Chip Select (CS) pin
        let cs_pin = Output::new(p.PB6, Level::High, Speed::VeryHigh);

        // 1.2 Configure the Data/Command (DC) pin
        let dc_pin = Output::new(p.PC7, Level::Low, Speed::VeryHigh);

        // 1.3 Configure the Reset (RST) pin
        let rst_pin = Output::new(p.PA9, Level::High, Speed::VeryHigh);

        // 1.4 Configure SPI2 for the TouchScreen Display
        let mut spi_display_config = SpiConfig::default();
        spi_display_config.frequency       = Hertz(16_000_000); // 16 MHz
        spi_display_config.mode            = MODE_0;

        let display_spi = Spi::new(
            p.SPI1, 
            p.PA5, // SCK
            p.PA7, // MOSI
            p.PA6, // MISO
            embassy_stm32::dma::NoDma, // <--- No TX DMA channel
            embassy_stm32::dma::NoDma, // <--- No RX DMA channel
            spi_display_config
        );

    // 2. Configure the TouchScreen SPI bus (SPI2) for the TouchScreen Controller
        // 2.1 Configure EXTI for T_IRQ (PC5)
        // T_IRQ goes LOW when pressed, so we pull it UP normally.
        // First, configure the raw pin as a standard Input with a Pull-Up
        let touch_pin_input = Input::new(p.PC5, Pull::None);

        // Then, wrap that Input pin with EXTI functionality
        let touch_irq_pin = ExtiInput::new(touch_pin_input, p.EXTI5);

        // 2.2 Configure the Chip Select (CS) pin
        let touch_cs_pin = Output::new(p.PB12, Level::High, Speed::VeryHigh);

        // 2.3 Configure SPI2 for the TouchScreen Controller
        let mut spi_touch_config = SpiConfig::default();
        spi_touch_config.frequency       = Hertz(1_000_000); // 1 MHz
        spi_touch_config.mode            = MODE_0;

        let touch_spi = Spi::new(
            p.SPI2, 
            p.PB13, // SCK
            p.PB15, // MOSI
            p.PB14, // MISO
            embassy_stm32::dma::NoDma, // <--- No TX DMA channel
            embassy_stm32::dma::NoDma, // <--- No RX DMA channel
            spi_touch_config
        );

    // 3. Spawn the various tasks for the TouchScreen Display and TouchScreen Controller\
        // 3.1 Spawn the TouchScreen touch task, passing the SPI bus, CS pin, and EXTI pin
        spawner.spawn(tasks::touchscreen_touch_task(touch_spi, touch_cs_pin, touch_irq_pin)).unwrap();
        // 3.2 Spawn the TouchScreen display task, passing the SPI bus and pins
        spawner.spawn(touchscreen_display_task(display_spi, cs_pin, dc_pin, rst_pin)).unwrap();

    loop 
    {
        embassy_time::Timer::after_secs(1).await;
    }
}