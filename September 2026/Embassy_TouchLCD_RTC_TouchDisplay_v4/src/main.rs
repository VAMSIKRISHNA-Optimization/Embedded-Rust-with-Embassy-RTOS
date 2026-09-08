#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::bind_interrupts;
use embassy_stm32::dma::NoDma;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::exti::{ExtiInput};
use embassy_stm32::i2c::{self, Config as I2cConfig, I2c};
use embassy_stm32::peripherals::{self, I2C1, I2C2};
use embassy_stm32::spi::{Config as SpiConfig, Spi, MODE_0};
use embassy_stm32::time::Hertz;
// use embassy_stm32::usart::{self, Uart};
use embassy_stm32::usart;
use embassy_stm32::Config;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use {defmt_rtt as _, panic_probe as _};

use embassy_stm32::usart::BufferedUart;

use static_cell::StaticCell;
//
mod tasks;

// Signals and Mutexes
pub static TOUCH_SIGNAL : Signal<CriticalSectionRawMutex, bool>                                     = Signal::new();
pub static I2C1_BUS      : Mutex<CriticalSectionRawMutex, Option<I2c<'static, I2C1, NoDma, NoDma>>>  = Mutex::new(None);
pub static I2C2_BUS     : Mutex<CriticalSectionRawMutex, Option<I2c<'static, I2C2, NoDma, NoDma>>>   = Mutex::new(None);

// Allocate static memory for the background UART buffers


static TX_BUF: StaticCell<[u8; 128]> = StaticCell::new();
static RX_BUF: StaticCell<[u8; 128]> = StaticCell::new();

// Bind Interrupts for I2C1 and USART2 (Virtual COM Port)
bind_interrupts!(struct Irqs {
    I2C1_EV => i2c::EventInterruptHandler<peripherals::I2C1>;
    I2C1_ER => i2c::ErrorInterruptHandler<peripherals::I2C1>;

    I2C2_EV => i2c::EventInterruptHandler<peripherals::I2C2>;
    I2C2_ER => i2c::ErrorInterruptHandler<peripherals::I2C2>;

    USART2  => usart::BufferedInterruptHandler<peripherals::USART2>; 
    // USART2  => usart::InterruptHandler<peripherals::USART2>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) 
{
    let mut config              = Config::default();
    config.enable_debug_during_sleep    = true;

    let p = embassy_stm32::init(config);

    info!("=== System Starting ===");

    // 1. Configure Display SPI (SPI1)
    let cs_pin  : Output<'_, peripherals::PB6>  = Output::new(p.PB6, Level::High, Speed::VeryHigh);
    let dc_pin  : Output<'_, peripherals::PC7>  = Output::new(p.PC7, Level::Low, Speed::VeryHigh);
    let rst_pin : Output<'_, peripherals::PA9>  = Output::new(p.PA9, Level::High, Speed::VeryHigh);

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
    // let touch_pin_input     = Input::new(p.PC5, Pull::Up);
    let touch_irq_pin   = ExtiInput::new(touch_pin_input, p.EXTI5);
    let touch_cs_pin      = Output::new(p.PB12, Level::High, Speed::VeryHigh);
    let mut spi_touch_config        = SpiConfig::default();
    spi_touch_config.frequency              = Hertz(1_000_000);
    spi_touch_config.mode                   = MODE_0;

    let touch_spi = Spi::new
    (
        p.SPI2, 
        p.PB13, // SCK  (Serial Clock)
        p.PB15, // MOSI (Master Out Slave In - STM32 sending to Touch chip)
        p.PB14, // MISO (Master In Slave Out - STM32 reading from Touch chip)
        NoDma, NoDma,
        spi_touch_config,
    );

    // 3. Configure I2C1 for SD3078 RTC and EEPROM (PB8 = SCL, PB9 = SDA)
    let i2c1 = I2c::new
    (
        p.I2C1, p.PB8, p.PB9,
        Irqs, NoDma, NoDma,
        Hertz(100_000),
        I2cConfig::default(),
    );

    // Store I2C peripheral inside global static Mutex
    {
        let mut i2c1_bus = I2C1_BUS.lock().await;
        *i2c1_bus = Some(i2c1);
    }

    // 3. Configure I2C2 for MPU6050 and Humidity Sensor (PB10 = SCL, PB3 = SDA)
    let i2c2 = I2c::new
    (
        p.I2C2, p.PB10, p.PB3,
        Irqs, NoDma, NoDma,
        Hertz(100_000),
        I2cConfig::default(),
    );

    // Store I2C peripheral inside global static Mutex
    {
        let mut i2c2_bus = I2C2_BUS.lock().await;
        *i2c2_bus = Some(i2c2);
    }



    // 4. Configure USART2 for Virtual COM Port (PA2 = TX, PA3 = RX)
    let mut uart_config = embassy_stm32::usart::Config::default();
    uart_config.baudrate = 115_200;

    // Initialize the safe static buffers
    let tx_buf = TX_BUF.init([0; 128]);
    let rx_buf = RX_BUF.init([0; 128]);

    let usart = BufferedUart::new
    (
        p.USART2,
        Irqs,
        p.PA3, // RX Pin
        p.PA2, // TX Pin
        tx_buf, // Completely safe, no unsafe block needed
        rx_buf,
        uart_config,
    ).unwrap();

    // // 4. Configure USART2 (Blocking Mode Test)
    // // Let Rust infer the type, no need for the BufferedUart annotation
    // let usart = Uart::new
    // (
    //     p.USART2,
    //     p.PA3, // RX Pin
    //     p.PA2, // TX Pin
    //     Irqs,
    //     NoDma, // No TX DMA
    //     NoDma, // No RX DMA
    //     embassy_stm32::usart::Config::default(),
    // ).unwrap();


    // Spawn all tasks
    info!("Spawn touch");
    spawner.spawn(tasks::touchscreen_touch_task(touch_spi, touch_cs_pin, touch_irq_pin)).unwrap();
    // embassy_time::Timer::after_millis(10).await;

    info!("Spawn display");
    spawner.spawn(tasks::touchscreen_display_task(display_spi, cs_pin, dc_pin, rst_pin)).unwrap();
    // embassy_time::Timer::after_millis(10).await;

    info!("Spawn UART");
    spawner.spawn(tasks::uart_time_config_task(usart)).unwrap();
    // embassy_time::Timer::after_millis(10).await;

    // Keep main task active on executor (matching v2)
    loop 
    {
        embassy_time::Timer::after_secs(10).await;
    }
}