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
use embassy_time::{block_for, Duration};

// SD Logging
use embedded_sdmmc::{Mode, SdCard, VolumeManager};
// use core::fmt::Write;
use heapless::String;
use embedded_sdmmc::{TimeSource, Timestamp};
use embedded_hal_bus::spi::ExclusiveDevice;
use core::fmt::Write   as FmtWrite;   // Enables write!() macro formatting on heapless::String
use embedded_io::Write as IoWrite;  // Enables .write() and .flush() on log_file
use embedded_sdmmc::sdcard::DummyCsPin;


//
mod tasks;

// Signals and Mutexes
pub static TOUCH_SIGNAL  : Signal<CriticalSectionRawMutex, bool>                                      = Signal::new();
pub static I2C1_BUS      : Mutex<CriticalSectionRawMutex, Option<I2c<'static, I2C1, NoDma, NoDma>>>   = Mutex::new(None);
pub static I2C2_BUS      : Mutex<CriticalSectionRawMutex, Option<I2c<'static, I2C2, NoDma, NoDma>>>   = Mutex::new(None);



// Allocate static memory for the background UART buffers


static TX_BUF: StaticCell<[u8; 128]> = StaticCell::new();
static RX_BUF: StaticCell<[u8; 128]> = StaticCell::new();

pub struct RtcTimeSource;
impl TimeSource for RtcTimeSource 
{
    fn get_timestamp(&self) -> Timestamp 
    {
        // Fallback static timestamp or query your RTC state (e.g. 2026-09-08 17:35:00)
        Timestamp 
        {
            year_since_1970: 56, // 2026 - 1970
            zero_indexed_month: 8, // September
            zero_indexed_day: 8,
            hours: 17,
            minutes: 35,
            seconds: 0,
        }
    }
}

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

    #[cfg(feature = "sd_card")]
    {
        // Configure the third SPI for SD card (SPI3) if needed in the future
        let third_spi_cs = Output::new(p.PB0, Level::High, Speed::VeryHigh);

        let mut spi3_config = SpiConfig::default();
        spi3_config.frequency       = Hertz(400_000);
        spi3_config.mode            = MODE_0;

        let mut third_spi = Spi::new
        (
            p.SPI3,
            p.PC10, // SCK
            p.PC12, // MOSI
            p.PC11, // MISO
            NoDma, NoDma,
            spi3_config,
        );

        // 2. Initialize SD Card Driver & Volume Manager
        // Combine the SPI3 bus, CS pin, and Delay into an SpiDevice
        let mut spi_sd_device = ExclusiveDevice::new(third_spi, third_spi_cs, embassy_time::Delay).unwrap();
        let mut sdcard = SdCard::new(spi_sd_device, DummyCsPin, embassy_time::Delay);
        let mut volume_mgr = VolumeManager::new(sdcard, RtcTimeSource);

        info!("Initializing SD card...");
        match volume_mgr.device().num_bytes() 
        {
            Ok(size) => info!("SD Card Capacity: {} MB", size / (1024 * 1024)),
            Err(e)   => defmt::error!("SD Card Init Failed: {:?}", defmt::Debug2Format(&e)),
        }

        // 3. Mount FAT32 Volume 0
        let mut volume   = volume_mgr.open_volume(embedded_sdmmc::VolumeIdx(0)).expect("Failed to mount FAT volume");
        let mut root_dir = volume.open_root_dir().expect("Failed to open root directory");

    // 4. Create or Open the "TEST" Directory
        if root_dir.open_dir("TEST").is_err() 
        {
            info!("Creating /TEST directory...");
            root_dir.make_dir_in_dir("TEST").expect("Failed to create TEST folder");
        }


        {
            let mut test_dir = root_dir.open_dir("TEST").expect("Failed to open TEST directory");
            
            // 5. Open / Create "LOG.CSV" inside /TEST
            let mut log_file = test_dir
                .open_file_in_dir("LOG.CSV", Mode::ReadWriteCreateOrTruncate)
                .expect("Failed to open LOG.CSV");

            // Write CSV Header line
            let _ = log_file.write(b"Date,Time\n");

            info!("SD Card Logger Ready. Entering 5-second loop...");
        }
    }


    // 3. Configure I2C1 for SD3078 RTC and EEPROM (PB8 = SCL, PB9 = SDA)
    let mut i2c1_config = I2cConfig::default();
    i2c1_config.timeout         = embassy_time::Duration::from_millis(100);  // Timeout config set to 100 ms.
    let i2c1 = I2c::new
    (
        p.I2C1, p.PB8, p.PB9,
        Irqs, NoDma, NoDma,
        Hertz(100_000),
        i2c1_config //
    );
    // Store I2C peripheral inside global static Mutex
    {
        let mut i2c1_bus = I2C1_BUS.lock().await;
        *i2c1_bus = Some(i2c1);
    }

    // Check if I2C1 is working by reading the RTC time (optional)
    {
        info!("Checking RTC on I2C1");
        let mut bus_guard = I2C1_BUS.lock().await;
        if let Some(ref mut i2c1_RTC) = *bus_guard 
        {
            let mut read_buf = [0u8; 7];
            if i2c1_RTC.blocking_write_read(0x32, &[0x00], &mut read_buf).is_ok() 
            {
                info!("RTC Handshake Successful!");
            }
            else 
            {
                info!("RTC Handshake Failure!");
            }
        }
    }

    // Check if I2C1 is working by reading the RTC time (optional)
    {
        info!("Checking EEPROM on I2C1");
        let mut bus_guard = I2C1_BUS.lock().await;
        if let Some(ref mut i2c1_EEPROM) = *bus_guard 
        {
            let mut read_buf = [0u8; 1];
            if i2c1_EEPROM.blocking_write_read(0x50, &[0x00, 0x00], &mut read_buf).is_ok() 
            {
                info!("EEPROM Handshake Successful!");
            }
            else 
            {
                info!("EEPROM Handshake Failure!");
            }
        }
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


    // // Spawn all tasks
    // info!("Spawn touch");
    // spawner.spawn(tasks::touchscreen_touch_task(touch_spi, touch_cs_pin, touch_irq_pin)).unwrap();
    // // embassy_time::Timer::after_millis(10).await;

    // info!("Spawn display");
    // spawner.spawn(tasks::touchscreen_display_task(display_spi, cs_pin, dc_pin, rst_pin)).unwrap();
    // // embassy_time::Timer::after_millis(10).await;

    // info!("Spawn UART");
    // spawner.spawn(tasks::uart_time_config_task(usart)).unwrap();
    // embassy_time::Timer::after_millis(10).await;

// --- Main 5-Second Logging Loop ---
loop 
{
    let mut read_buf = [0u8; 7];
    let mut logged_successfully = false;

    // Acquire I2C1 Mutex and fetch RTC time
    {
        let mut bus_guard = I2C1_BUS.lock().await;
        if let Some(ref mut i2c1_rtc) = *bus_guard 
        {
            if i2c1_rtc.blocking_write_read(0x32, &[0x00], &mut read_buf).is_ok() 
            {
                // Decode BCD values
                let second = (read_buf[0] >> 4) * 10 + (read_buf[0] & 0x0F);
                let minute = (read_buf[1] >> 4) * 10 + (read_buf[1] & 0x0F);
                let hour   = (read_buf[2] >> 4) * 10 + (read_buf[2] & 0x0F);
                let day    = (read_buf[4] >> 4) * 10 + (read_buf[4] & 0x0F);
                let month  = (read_buf[5] >> 4) * 10 + (read_buf[5] & 0x0F);
                let year   = ((read_buf[6] >> 4) * 10 + (read_buf[6] & 0x0F)) as u16 + 2000;

                // Format string buffer using heapless (no heap allocations)
                let mut line: String<64> = String::new();
                let _ = core::write!
                (
                    line,
                    "{:04}-{:02}-{:02},{:02}:{:02}:{:02}\n",
                    year, month, day, hour, minute, second
                );



                // // SD LOGGING EVERY 5 SECONDS
                // #[cfg(feature = "sd_card")]
                // {
                //     let mut test_dir = root_dir.open_dir("TEST").expect("Failed to open TEST directory");
                //     // Open the file locally in this scope
                //     if let Ok(mut log_file) = test_dir.open_file_in_dir("log.csv", Mode::ReadWriteCreateOrAppend) 
                //     {
                //         let _ = log_file.write(line.as_bytes());
                //         // let _ = log_file.flush();
                //         info!("Logged to SD: {}", line.as_str().trim_end());
                //         logged_successfully = true;
                //     } // `log_file` goes out of scope and releases `test_dir` here
                // }
            }
        }
    }

    #[cfg(feature = "sd_card")]
    {
        if !logged_successfully 
        {
            defmt::warn!("Failed to log entry to SD card!");
        }
    }


    // Wait 5 seconds using blocking delay
    block_for(Duration::from_secs(5));
}
}