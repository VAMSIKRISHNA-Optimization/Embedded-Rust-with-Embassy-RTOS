#![no_std]
#![no_main]

use {defmt_rtt as _, panic_probe as _};
use embassy_executor::Spawner;
use embassy_stm32::spi::{Config as SpiConfig, Spi, MODE_0, MODE_1, MODE_2, MODE_3};
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::time::Hertz;
use defmt::info;

use embassy_stm32::bind_interrupts;
use embassy_stm32::i2c::{self, EventInterruptHandler, ErrorInterruptHandler};
use embassy_stm32::dcmi::InterruptHandler as DcmiInterruptHandler;

mod tasks;
// use tasks::touchscreen_display_task;

// Bind all necessary interrupts in one place
bind_interrupts!(struct Irqs {
    I2C2_EV => EventInterruptHandler<embassy_stm32::peripherals::I2C2>;
    I2C2_ER => ErrorInterruptHandler<embassy_stm32::peripherals::I2C2>;
    DCMI    => DcmiInterruptHandler<embassy_stm32::peripherals::DCMI>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) 
{
    info!("=== Camera and Display Testing ===");
    

    // 1. Configure the system clock and PLL settings for the STM32 microcontroller
    let mut config = embassy_stm32::Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.sys = Sysclk::PLL1_P;
        config.rcc.hse = Some(Hse {
            freq: embassy_stm32::time::Hertz(8_000_000), // Nucleo bypass clock
            mode: HseMode::Bypass,
        });
        config.rcc.pll_src = PllSource::HSE;
        config.rcc.pll = Some(Pll {
            prediv: PllPreDiv::DIV4,
            mul: PllMul::MUL180,
            divp: Some(PllPDiv::DIV2),
            divq: Some(PllQDiv::DIV2),
            divr: None,
        });
        config.rcc.ahb_pre  = AHBPrescaler::DIV1;
        config.rcc.apb1_pre = APBPrescaler::DIV4;
        config.rcc.apb2_pre = APBPrescaler::DIV2;

        config.enable_debug_during_sleep = true;
    }
    
        // 1.1 Initialize the STM32 microcontroller with the configured settings
        let p = embassy_stm32::init(config);
        info!("STM32F446RE Initialized Successfully!");


    // 2. Configure the SPI bus for the TouchScreen Display (Conflict-Free)
        // 2.1 Configure the Chip Select (CS) pin on PC4
        let cs_pin = Output::new(p.PC4, Level::High, Speed::VeryHigh);

        // 2.2 Configure the Data/Command (DC) pin on PC5
        let dc_pin = Output::new(p.PC5, Level::Low, Speed::VeryHigh);

        // 2.3 Configure the Reset (RST) pin on PB1
        let rst_pin = Output::new(p.PB1, Level::High, Speed::VeryHigh);

        // 2.4 Configure SPI1 for the Display with TX DMA enabled
        let mut spi_config  = SpiConfig::default();
        spi_config.frequency        = Hertz(45_000_000); // 45 MHz (Max for APB2 on F446RE)
        spi_config.mode             = MODE_0;

        let spi = Spi::new
        (
            p.SPI1,
            p.PA5, // SCK
            p.PA7, // MOSI
            p.PB4, // MISO (Re-routed to PB4 to avoid PA6 DCMI_PIXCLK conflict)
            p.DMA2_CH3, // TX DMA channel assigned for high-speed pixel pushing
            embassy_stm32::dma::NoDma, // RX DMA not strictly needed for TFTs unless reading GRAM
            spi_config,
        );
        info!("Display SPI configured with TX DMA!");


    //3. Initialize DCMI (Camera)
        // 3. 1 Provide Master Clock to the Camera (MCO1 on PA8)
        // This outputs the 8MHz HSE directly to the camera's XCLK pin
        let _xclk = embassy_stm32::rcc::Mco::new
        (
            p.MCO1,                                // 1. MCO Peripheral instance
            p.PA8,                                 // 2. The physical pin
            embassy_stm32::rcc::Mco1Source::HSE,   // 3. Source (Note the '1' in Mco1Source)
            embassy_stm32::rcc::McoPrescaler::DIV1 // 4. Prescaler
        );


    //4. Initialize I2C (Camera SCCB config)
        // 4.1 Configure I2C2 for the OV7670 SCCB (Configuration) Interface
        let mut i2c_config = embassy_stm32::i2c::Config::default();
        i2c_config.timeout = embassy_time::Duration::from_millis(100);
        
        let mut i2c = embassy_stm32::i2c::I2c::new
        (
            p.I2C2,
            p.PB10, // SCL
            p.PB3, // SDA
            Irqs,
            embassy_stm32::dma::NoDma, // No DMA needed for simple I2C config commands
            embassy_stm32::dma::NoDma,
            embassy_stm32::time::Hertz(100_000), // OV7670 SCCB runs at 100 kHz max
            i2c_config,
        );
        info!("Camera I2C (SCCB) Initialized!");
        
        // 4.2 --- I2C COMMUNICATION TEST ---

        // // --- I2C SCANNER TEST ---
        // info!("Starting I2C bus scan...");
        // let mut found_devices = 0;
        
        // for addr in 1..=127 {
        //     // We just do a 0-byte read to see if the address sends an ACK
        //     match i2c.blocking_read(addr, &mut []) {
        //         Ok(_) => {
        //             info!("Found I2C device at address: {:#04x}", addr);
        //             found_devices += 1;
        //         }
        //         Err(_) => {
        //             // Ignore Nacks, we expect them for empty addresses
        //         }
        //     }
        // }
        
        // if found_devices == 0 {
        //     defmt::error!("No devices found on the I2C bus!");
        // } else {
        //     info!("Scan complete. Found {} device(s).", found_devices);
        // }

        // --- I2C COMMUNICATION TEST ---
            let mut pid_buf = [0u8; 1];
            
            // Step A: Tell the camera we want to look at register 0x0A
            match i2c.blocking_write(0x21, &[0x0A]) {
                Ok(_) => {
                    // Step B: Ask the camera for the data at that register
                    match i2c.blocking_read(0x21, &mut pid_buf) {
                        Ok(_) => info!("SUCCESS: Camera detected! PID: {:#04x}", pid_buf[0]),
                        Err(e) => defmt::error!("FAILED on Read phase (Protocol quirk): {:?}", defmt::Debug2Format(&e)),
                    }
                }
                Err(e) => defmt::error!("FAILED on Write phase (Hardware issue): {:?}", defmt::Debug2Format(&e)),
            }


    // 5. Configure DCMI for parallel data capture
    let dcmi = embassy_stm32::dcmi::Dcmi::new_8bit
    (
        p.DCMI,
        p.DMA2_CH1,
        Irqs,
        p.PC6,      // d0 (Fixed: must be D0 pin)
        p.PC7,      // d1 (Fixed: must be D1 pin)
        p.PC8,      // d2
        p.PC9,      // d3
        p.PC11,     // d4
        p.PB6,      // d5 (or p.PB6 if not used by Display CS)
        p.PB8,      // d6
        p.PB9,      // d7
        p.PB7,      // v_sync
        p.PA4,      // h_sync
        p.PA6,      // pixclk
        embassy_stm32::dcmi::Config::default(),
    );

    info!("Camera DCMI Initialized!");

    // 2.5 Spawn the TouchScreen display task, passing the SPI bus and pins
    // spawner.spawn(touchscreen_display_task(spi, cs_pin, dc_pin, rst_pin)).unwrap();

    loop 
    {
        embassy_time::Timer::after_secs(10).await;
    }
}

