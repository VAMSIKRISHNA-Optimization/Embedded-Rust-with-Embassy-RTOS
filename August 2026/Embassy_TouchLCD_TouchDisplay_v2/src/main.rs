#![no_std]
#![no_main]

use defmt::*;
use {defmt_rtt as _, panic_probe as _};
use embassy_executor::Spawner;

use embassy_stm32::bind_interrupts;
use embassy_stm32::peripherals;
use embassy_stm32::dma::NoDma;
use embassy_stm32::i2c::{self, Config as I2cConfig, I2c};

use embassy_stm32::spi::{Config as SpiConfig, Spi, MODE_0};
use embassy_stm32::gpio::{Level, Output, Speed, Input, Pull};
use embassy_stm32::exti::ExtiInput;

use embassy_stm32::time::Hertz;


use embassy_sync::signal::Signal;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;


mod tasks;
use tasks::touchscreen_display_task;

// Wire up the hardware interrupts for I2C1
bind_interrupts!
(struct Irqs 
    {
    I2C1_EV => i2c::EventInterruptHandler<peripherals::I2C1>;
    I2C1_ER => i2c::ErrorInterruptHandler<peripherals::I2C1>;
});

// A global signal to notify the display task that a touch occurred
pub static TOUCH_SIGNAL: Signal<CriticalSectionRawMutex, bool> = Signal::new();

// SD3078 I2C 7-bit Bus Address
const SD3078_ADDRESS    : u8 = 0x32;
// Charge Control Register Address
const REG_CHARGE_CONTROL: u8 = 0x18;

#[embassy_executor::main]
async fn main(spawner: Spawner) 
{
    defmt::info!("=== TouchScreen Display Starting ===");

    // Initialize the Embassy STM32 HAL
    let mut config              = embassy_stm32::Config::default();
    config.enable_debug_during_sleep    = true;

    let p                  = embassy_stm32::init(config);

    // -------------------------------------------------------------------
    // 1. Configure the SD3078 RTC via I2C1 to disable the internal charging circuit
        // -------------------------------------------------------------------
        // 1.1 Configure I2C1 Bus for SD3078 RTC (PB6 = SCL, PB7 = SDA)
        // -------------------------------------------------------------------
        // let mut i2c = I2c::new
        // (
        //         p.I2C1,
        //         p.PB8, // SCL
        //         p.PB9, // SDA
        //         Irqs,  // <--- Pass the interrupts we bound above
        //         embassy_stm32::dma::NoDma, // <--- No TX DMA channel, // <--- No TX DMA channel
        //         embassy_stm32::dma::NoDma, // <--- No TX DMA channel, // <--- No RX DMA channel
        //         Hertz(100_000), // Standard 100 kHz I2C speed
        //         I2cConfig::default(),
        // );

        // info!("I2C1 Initialized. Sending Safety Command to SD3078...");

        // // -------------------------------------------------------------------
        // // 1.22. Disable RTC Battery Charging Circuit (SAFETY STEP)
        // // -------------------------------------------------------------------
        // // Writing 0x00 to Register 0x18 turns off the internal trickle charger
        // let disable_charge_cmd = [REG_CHARGE_CONTROL, 0x00];
        
        // match i2c.blocking_write(SD3078_ADDRESS, &disable_charge_cmd) 
        // {
        //     Ok(_)   => info!("SUCCESS: SD3078 Charging Circuit Disabled! It is now safe to insert CR coin cell."),
        //     Err(e)  => error!("Failed to write to SD3078 via I2C: {:?}", e),
        // }
        



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

        let display_spi = Spi::new
        (
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

        let touch_spi = Spi::new
        (
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
        info!("System heartbeat... RTC safety configured.");
    }
}