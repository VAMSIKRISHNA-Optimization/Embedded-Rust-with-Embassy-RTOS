#![no_std]
#![no_main]

#![allow(non_camel_case_types)]

use defmt::*;
use core::cell::RefCell;
use embassy_executor::Spawner;

use embassy_stm32::rcc::*;

use embassy_stm32::dma::NoDma;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::exti::{ExtiInput};

use embassy_stm32::peripherals::SPI1;

use embassy_stm32::spi::{Config as SpiConfig, Spi, MODE_0};
use embassy_stm32::time::Hertz;

use embassy_stm32::bind_interrupts;
use embassy_stm32::i2c::{self, Config as I2cConfig, I2c};
use embassy_stm32::peripherals::I2C1;



use embassy_stm32::Config;

use embassy_sync::blocking_mutex::raw::{NoopRawMutex, CriticalSectionRawMutex};
use embassy_sync::blocking_mutex::Mutex as SharedResourceLock;

use embassy_embedded_hal::shared_bus::blocking::spi::SpiDeviceWithConfig;
use embassy_sync::signal::Signal;

use {defmt_rtt as _, panic_probe as _};

use embassy_executor::InterruptExecutor;
use embassy_stm32::interrupt;
use embassy_stm32::interrupt::{InterruptExt, Priority};
use static_cell::StaticCell;

// Ref Cell : Interior Mutability in Single-Threaded Contexts without Locking
// Mutex    : Interior Mutability in Multi-Threaded Contexts with Locking

// Embassy requires RefCell + Blocking Mutex in single-threaded multi-tasking contexts. 
// The RefCell allows interior mutability of the SPI bus, while the Mutex ensures that only one task can access the SPI bus at a time, preventing data corruption and ensuring safe concurrent access

mod tasks;
mod display_integration;
mod battery_integration;

mod BE_Battery;

// Signals 
pub static TOUCH_SIGNAL : Signal<CriticalSectionRawMutex, bool> = Signal::new();

// Mutexes
static SPI_BUS: StaticCell<SharedResourceLock<NoopRawMutex, RefCell<Spi<'static, SPI1, NoDma, NoDma>>>> = StaticCell::new();

// Type alias for the BE_Battery I2C bus
bind_interrupts!(struct Irqs 
{
    I2C1_EV => i2c::EventInterruptHandler<I2C1>;
    I2C1_ER => i2c::ErrorInterruptHandler<I2C1>;
});
pub type BE_Battery_I2cBus = I2c<'static, I2C1, NoDma, NoDma>;

// Create a static instance of the high-priority executor
static EXECUTOR_HIGH: InterruptExecutor = InterruptExecutor::new();

// Bind UART4 interrupt to drive EXECUTOR_HIGH
#[interrupt]
unsafe fn UART4() 
{
    EXECUTOR_HIGH.on_interrupt();
}


#[embassy_executor::main]
async fn main(spawner: Spawner) 
{
    // 1. Basic Setup
    let mut config              = Config::default();
    config.enable_debug_during_sleep    = true;

    // 2. Configure System Clock to 180 MHz using PLL
    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.pll = Some(Pll {
            prediv: PllPreDiv::DIV16,
            mul: PllMul::MUL360,
            divp: Some(PllPDiv::DIV2),
            divq: None,
            divr: None,
        });

    // 3. Set required bus prescalers for 180 MHz operation
    config.rcc.ahb_pre  = AHBPrescaler::DIV1;   // AHB  = 180 MHz
    config.rcc.apb1_pre = APBPrescaler::DIV4;   // APB1 = 45 MHz (Max allowed)
    config.rcc.apb2_pre = APBPrescaler::DIV2;   // APB2 = 90 MHz (Max allowed)

    let p = embassy_stm32::init(config);

    info!("=== System Starting ===");

    // 4. Configure SPI 1 for both display and SD module
    let mut spi1_config  = SpiConfig::default();
    spi1_config.frequency        = Hertz(1_000_000); // Start at low speeds of 1 MHz
    spi1_config.mode             = MODE_0;

    let spi1_peripheral  = Spi::new
    (
        p.SPI1, 
        p.PA5, // SCK  (Serial Clock)
        p.PA7, // MOSI (Master Out Slave In - STM32 sending to Display)
        p.PA6, // MISO (Master In Slave Out - STM32 reading from Display)
        NoDma, NoDma,
        spi1_config,
    );

        // 4.1. Create the SPI BUS to be shared between the display and SD card module
        let spi_bus = SPI_BUS.init(SharedResourceLock::new(RefCell::new(spi1_peripheral)));

    
    // 5. Configure the CS for SD module
    let sd_cs = Output::new(p.PB0, Level::High, Speed::VeryHigh);
        
        // 5.1. SD Card Config (Starts Slow: 400 kHz for initialization)
        let mut sd_config   = SpiConfig::default();
        sd_config.frequency         = Hertz(400_000); // 400 kHz required for SD card identification
        sd_config.mode              = MODE_0;


    // 6. Configure Display's pins
    let display_cs_pin  = Output::new(p.PB6, Level::High, Speed::VeryHigh);
    let display_dc_pin  = Output::new(p.PC7, Level::Low, Speed::VeryHigh);
    let display_rst_pin = Output::new(p.PA9, Level::High, Speed::VeryHigh);
        
        // 6.1. Display SPI Config (Fast: 16 MHz to 24 MHz)
        let mut display_config  = SpiConfig::default();
        display_config.frequency        = Hertz(16_000_000); // 16 MHz for fast frame rendering
        display_config.mode             = MODE_0;


    // 7. Create individual type-safe handles for each task
    let display_spi_device   = SpiDeviceWithConfig::new(spi_bus, display_cs_pin, display_config);
    let sd_spi_device        = SpiDeviceWithConfig::new(spi_bus, sd_cs, sd_config);


    // 8. Configure Touch SPI (SPI2) + EXTI
    // let touch_pin_input     = Input::new(p.PC5, Pull::None);
    let touch_pin_input     = Input::new(p.PC5, Pull::Up);
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


    // --- 8.5 Initialize SMBus I2C1 at 100 kHz for BQ78350-R1A ---
    let mut be_i2c_config = I2cConfig::default();
    // Enable internal pull-ups as a bench safety net (keep 4.7k-10k external pull-ups on board)
    be_i2c_config.scl_pullup = true;
    be_i2c_config.sda_pullup = true;

    let be_i2c: BE_Battery_I2cBus = I2c::new
    (
            p.I2C1,
            p.PB8, // SMBC (SCL)
            p.PB9, // SMBD (SDA)
            Irqs,
            NoDma,
            NoDma,
            Hertz(100_000),
            be_i2c_config,
        );


    // // --- 9. Start High-Priority Executor for Touch ---
    //     // P1 is higher priority than default Thread Mode (P0)
    //     // 9.1. Set priority
    //     interrupt::UART4.set_priority(Priority::P1);

    //     // 9.2. Pass interrupt::UART4 directly into start()
    //     use tasks::touch_task::touchscreen_touch_task;
    //     let spawner_high = EXECUTOR_HIGH.start(interrupt::UART4);
    //     // Spawn touch task on HIGH priority spawner
    //     spawner_high
    //         .spawn(touchscreen_touch_task
    //         (
    //             touch_spi,
    //             touch_cs_pin,
    //             touch_irq_pin,
    //         ))
    //         .unwrap();

    //     info!("Spawned Touch Task on High-Priority Executor!");

    // --- 10. Spawn BE_Battery Tasks in Deterministic Pipeline Order ---
    // Consumers first (they immediately sleep at their Signal::wait().await)
    info!("Spawning SD Task...");
    use tasks::sd_task::sd_task;
    spawner.spawn(sd_task(sd_spi_device)).unwrap();

    info!("Spawning Display Task...");
    use tasks::display_task::touchscreen_display_task;
    spawner.spawn(touchscreen_display_task(display_spi_device, display_dc_pin, display_rst_pin)).unwrap();

    info!("Spawning defmt Publish Task...");
    spawner.spawn(tasks::defmt_publish_task::defmt_publish_task()).unwrap();

    // Producer last (starts the 1-second Ticker and initiates the first baton pass)
    info!("Spawning BE_Battery SMBus Data Acquisition Task...");
    let be_smbus_task =
        BE_Battery::BE_Battery_Init_Integration::BE_BatterySmbusTask::new(be_i2c, false);
    spawner
        .spawn
        (
            tasks::battery_tasks::BE_battery_smbus_task::BE_battery_smbus_data_acquisition_task
            (
                be_smbus_task,
            ),
        )
        .unwrap();  

    

    // Keep main task active on executor (matching v2)
    loop 
    {
        embassy_time::Timer::after_secs(10000).await;
    }
}