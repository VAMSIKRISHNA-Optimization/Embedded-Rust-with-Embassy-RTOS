#![no_std]
#![no_main]



// use panic_halt as _;
use {defmt_rtt as _, panic_probe as _};
defmt::timestamp!("{=u64:us}", embassy_time::Instant::now().as_micros());
use embassy_executor::Spawner; // Call Tasks
use embassy_time::Timer;       // osDelay from RTOS
use embassy_stm32::time::Hertz;

use embassy_stm32::bind_interrupts;
// use embassy_stm32::peripherals;
use embassy_stm32::peripherals::{I2C2};
use embassy_stm32::i2c;
use embassy_stm32::dma::NoDma;


// Bind the I2C error and event interrupts
// bind_interrupts!(struct Irqs {
//     I2C1_EV => i2c::EventInterruptHandler<peripherals::I2C1>;
//     I2C1_ER => i2c::ErrorInterruptHandler<peripherals::I2C1>;
// });

// Bind interrupts for I2C2
bind_interrupts!(struct Irqs2 {
    I2C2_EV => i2c::EventInterruptHandler<I2C2>;
    I2C2_ER => i2c::ErrorInterruptHandler<I2C2>;
});

// Import the GPIO types from the Embassy STM32 HAL
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::i2c::{Config as I2cConfig};



mod tasks;
// use tasks::{blink_task, eeprom_task};
use tasks::{lcd_task};



#[embassy_executor::main]
async fn main(spawner: Spawner) 
{
    defmt::info!("=== PROGRAM STARTED ==="); // <--- Quick test

    // Initialize all peripherals
    let mut config = embassy_stm32::Config::default();
    config.enable_debug_during_sleep = true;

    let p = embassy_stm32::init(config);

    // 2. Configure Pin PA5 as a push-pull output
    // We set its initial state to High and its clock speed to Low
    let led = Output::new(p.PA5, Level::High, Speed::Low);

    // Setup I2C2 peripheral (PB10 = SCL, PB3 = SDA)
    let i2c2_config = I2cConfig::default();


    let i2c2 = i2c::I2c::new(
                                            p.I2C2,          // 1. I2C Peripheral
                                            p.PB10,          // 2. SCL Pin
                                            p.PB3,           // 3. SDA Pin
                                            Irqs2,            // 4. Interrupt bindings
                                            NoDma,           // 5. TX DMA channel (NoDma = blocking)
                                            NoDma,           // 6. RX DMA channel (NoDma = blocking)
                                            Hertz(100_000),  // 7. Clock frequency (Standard mode 100kHz)
                                            i2c2_config,      // 8. I2C Configuration struct
                                        );

                                        // Keep the SWD debugger alive during low-power sleep modes
    // cortex_m::asm::dbg_enable_sleep();


    // 3. Spawn the background task and hand over the LED peripheral
    // We use .unwrap() because spawning only fails if the task queue is full
    // spawner.spawn(blink_task(led)) .unwrap();
    spawner.spawn(lcd_task(i2c2)).unwrap();

    // 4. The main task can now do its own thing completely independently!
    loop 
    {
        // For now, we will just let main sleep in 5-second intervals.
        // Even while main is sleeping, blink_task will continue running perfectly.
        Timer::after_millis(5000).await;
    }
}