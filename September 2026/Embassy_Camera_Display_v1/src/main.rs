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
// use embassy_stm32::dma::InterruptHandler as DmaInterruptHandler;

mod tasks;
// use tasks::touchscreen_display_task;
use embassy_stm32::dcmi::{Config, VSyncDataInvalidLevel, HSyncDataInvalidLevel, PixelClockPolarity}; // Correct imports
use embassy_stm32::rcc::{Mco, McoPrescaler, Mco1Source};
use embassy_time::Duration;

// Bind all necessary interrupts in one place
bind_interrupts!(struct Irqs {
    I2C2_EV => EventInterruptHandler<embassy_stm32::peripherals::I2C2>;
    I2C2_ER => ErrorInterruptHandler<embassy_stm32::peripherals::I2C2>;
    DCMI    => DcmiInterruptHandler<embassy_stm32::peripherals::DCMI>;
    // DMA2_STREAM1 => DmaInterruptHandler<embassy_stm32::peripherals::DMA2_CH1>; // Add DMA mapping
});


const OV7670_INIT_REGS: &[(u8, u8)] = &[
    (0x12, 0x14), // COM7: QVGA base + RGB output
    (0x0c, 0x04), // COM3: Enable scaling
    (0x3e, 0x1a), // COM14: Divide PCLK by 4 for QQVGA
    (0x70, 0x3a), // SCALING_XSC
    (0x71, 0x35), // SCALING_YSC
    (0x72, 0x22), // SCALING_DCWCTR: Downsample by 4 (160x120)
    (0x73, 0xf2), // SCALING_PCLK_DIV
    (0xa2, 0x02), // SCALING_PCLK_DELAY
    (0x15, 0x20), // COM10: Gate PCLK during horizontal blanking
    (0x40, 0xd0), // COM15: Full RGB565 range
    (0x11, 0x03), // CLKRC: Divide internal clock
];
// OV7670 QQVGA (160x120) RGB565 Register Configuration
// const OV7670_QQVGA_RGB565: &[(u8, u8)] = &[
//     (0x12, 0x14), // COM7: Enable scaling + RGB output
//     // (0x12, 0x16), // COM7: Enable scaling, RGB output, AND Color Bar Test Pattern
//     (0x40, 0xD0), // COM15: RGB565 format (0x00-0xFF range)
//     (0x0C, 0x04), // COM3: Enable DCW scaling
//     (0x3E, 0x1A), // COM14: Divide PCLK clock by 4 for QQVGA

//     // --- NEW: Enable Auto-Exposure and Auto-Gain ---
//     (0x13, 0xE0), // COM8: Enable AGC (Auto Gain), AEC (Auto Exposure), and AWB
//     (0x00, 0x00), // GAIN: Reset AGC gain
//     (0x10, 0x00), // AECH: Reset AEC exposure
//     (0x14, 0x18), // COM9: Set automatic gain ceiling to 4x
//     (0x24, 0x95), // AEW: Auto exposure upper limit
//     (0x25, 0x33), // AEB: Auto exposure lower limit
//     (0x26, 0xE3), // VPT: Fast Auto-Exposure stabilization
    
//     // --- NEW: Exact Windowing for 160x120 ---
//     (0x32, 0x80), // HREF control
//     (0x17, 0x16), // HSTART
//     (0x18, 0x04), // HSTOP
//     (0x19, 0x02), // VSTART
//     (0x1A, 0x7b), // VSTOP
//     (0x03, 0x0A), // VREF
    
//     (0x70, 0x3A), // SCALING_XSC: Horizontal scale factor
//     (0x71, 0x35), // SCALING_YSC: Vertical scale factor
//     (0x72, 0x11), // SCALING_DCWCTR: Downsampling control
//     (0x73, 0xF1), // SCALING_PCLK_DIV: Clock divider ratio
//     (0x7A, 0x02), // SCALING_PCLK_DELAY: Delay compensation
// ];

// const OV7670_QQVGA_RGB565: &[(u8, u8)] = &[
// // --- FROM C CODE: Color Matrix Coefficients (Fixes purple/yellow hues) ---
//     (0x4f, 0x80),
//     (0x50, 0x80),
//     (0x51, 0x00),
//     (0x52, 0x22),
//     (0x53, 0x5e),
//     (0x54, 0x80),
//     (0x58, 0x9e),

//     // --- FROM C CODE: Edge enhancement, de-noise, AWB gain enabled ---
//     (0x41, 0x38),

//     // --- FROM C CODE: Gamma Curve (Crucial for ambient light visibility) ---
//     (0x7b, 16), (0x7c, 30), (0x7d, 53), (0x7e, 90), 
//     (0x7f, 105), (0x80, 118), (0x81, 130), (0x82, 140), 
//     (0x83, 150), (0x84, 160), (0x85, 180), (0x86, 195), 
//     (0x87, 215), (0x88, 230), (0x89, 244), (0x7a, 16),
// ];

const OV7670_QQVGA_RGB565: &[(u8, u8)] = &[
    // --- SCALING AND WINDOWING (160x120) ---
    (0x12, 0x14), // COM7: Enable scaling + RGB output
    (0x40, 0xD0), // COM15: RGB565 format
    (0x0C, 0x04), // COM3: Enable DCW scaling
    (0x3E, 0x1A), // COM14: Divide PCLK clock by 4 for QQVGA
    (0x32, 0x80), // HREF control
    (0x17, 0x16), // HSTART
    (0x18, 0x04), // HSTOP
    (0x19, 0x02), // VSTART
    (0x1A, 0x7b), // VSTOP
    (0x03, 0x0A), // VREF
    (0x70, 0x3A), // SCALING_XSC
    (0x71, 0x35), // SCALING_YSC
    (0x72, 0x11), // SCALING_DCWCTR
    (0x73, 0xF1), // SCALING_PCLK_DIV
    (0x7A, 0x02), // SCALING_PCLK_DELAY
    
    // --- COLOR MATRIX & GAMMA ---
    (0x4f, 0x80), (0x50, 0x80), (0x51, 0x00), (0x52, 0x22),
    (0x53, 0x5e), (0x54, 0x80), (0x58, 0x9e), (0x41, 0x38),
    (0x7b, 16), (0x7c, 30), (0x7d, 53), (0x7e, 90), 
    (0x7f, 105), (0x80, 118), (0x81, 130), (0x82, 140), 
    (0x83, 150), (0x84, 160), (0x85, 180), (0x86, 195), 
    (0x87, 215), (0x88, 230), (0x89, 244), (0x7a, 16),
];


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

        // Output a 16 MHz clock signal on PA8 (MCO1) for the camera's XCLK
        // config.rcc.mco1 = Some(Mco1 
        //     {
        //     source: Mco1Source::HSI,
        //     prescaler: McoPrescaler::DIV1,
        // });
    

        config.enable_debug_during_sleep = true;
    }
    
        // 1.1 Initialize the STM32 microcontroller with the configured settings
        let p = embassy_stm32::init(config);
        info!("STM32F446RE Initialized Successfully!");

        // 1.2 Output clock on PA8 (e.g., for OV7670 XCLK)
        // let _mco = Mco::new
        // (
        //     p.MCO1,
        //     p.PA8,
        //     Mco1Source::HSI,       // Choose source: HSI, HSE, SYSCLK, or PLL
        //     McoPrescaler::DIV1,  // Prescaler division factor
        // );


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

        let mut spi = Spi::new
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


    // 3. Initialize DCMI (Camera)
        // 3. 1 Provide Master Clock to the Camera (MCO1 on PA8)
        // This outputs the 8MHz HSE directly to the camera's XCLK pin
        let _xclk = embassy_stm32::rcc::Mco::new
        (
            p.MCO1,                                // 1. MCO Peripheral instance
            p.PA8,                                 // 2. The physical pin
            embassy_stm32::rcc::Mco1Source::HSE,   // 3. Source (Note the '1' in Mco1Source)
            embassy_stm32::rcc::McoPrescaler::DIV1 // 4. Prescaler
        );


        // 1. Initialize GPIO pin for Camera RESET (starting LOW)
        let mut cam_reset = Output::new(p.PC0, Level::Low, Speed::Low);

        // 2. WAKE THE CAMERA UP IMMEDIATELY
        cam_reset.set_low();
        embassy_time::block_for(Duration::from_millis(100));
        cam_reset.set_high(); // Release from reset
        embassy_time::block_for(Duration::from_millis(100)); // Give DSP time to boot
        
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

        // Add blocking delay to ensure the camera has time to respond before reading its PID
        // Give the camera SCCB engine 50ms (blocking) to stabilize after power-up/clock init
        embassy_time::block_for(embassy_time::Duration::from_millis(1000));

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
    let mut dcmi_config = Config::default();
    // Combination A (Most common for default OV7670 register sets)
    // dcmi_config.vsync_level = VSyncDataInvalidLevel::Low;
    // dcmi_config.hsync_level = HSyncDataInvalidLevel::Low;
    // dcmi_config.pixclk_polarity = PixelClockPolarity::RisingEdge;

    // Combination B
    // dcmi_config.vsync_level = VSyncDataInvalidLevel::Low;
    // dcmi_config.hsync_level = HSyncDataInvalidLevel::High;
    // dcmi_config.pixclk_polarity = PixelClockPolarity::RisingEdge;


    // Combination C
    dcmi_config.vsync_level = VSyncDataInvalidLevel::High;
    dcmi_config.hsync_level = HSyncDataInvalidLevel::High;
    dcmi_config.pixclk_polarity = PixelClockPolarity::RisingEdge;


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
        dcmi_config
    );

    info!("Camera DCMI Initialized!");

    // 6. --- CAMERA CONFIGURATION ---
    info!("Writing Initialization registers to OV7670...");

    // 2. Drive RESET pin LOW for 100ms
    // cam_reset.set_low();
    // embassy_time::block_for(Duration::from_millis(100));

    // // 3. Drive RESET pin HIGH for 100ms (Triggers internal DSP boot latch)
    // cam_reset.set_high();
    // embassy_time::block_for(Duration::from_millis(100));

    for &(reg, val) in OV7670_INIT_REGS 
    {
        // SCCB Write: [Register Address, Data Value]
        match i2c.blocking_write(0x21, &[reg, val]) 
        {
            Ok(_) => 
            {
                // If it's the reset command, the camera needs a few milliseconds to reboot
                if reg == 0x12 && val == 0x80 
                {
                    embassy_time::block_for(embassy_time::Duration::from_millis(100));
                }
            }
            Err(e) => 
            {
                defmt::error!("Failed to write reg {:#04x}. Error: {:?}", reg, defmt::Debug2Format(&e));
                // break; // Stop trying if the bus crashes
            }
        }

        embassy_time::block_for(embassy_time::Duration::from_millis(2));
    }

    info!("Initialization registers written. Now configuring QQVGA RGB565 mode...");

    embassy_time::block_for(embassy_time::Duration::from_millis(50));
    
    for &(reg, val) in OV7670_QQVGA_RGB565 
    {
        // SCCB Write: [Register Address, Data Value]
        match i2c.blocking_write(0x21, &[reg, val]) 
        {
            Ok(_) => 
            {
                // If it's the reset command, the camera needs a few milliseconds to reboot
                if reg == 0x12 && val == 0x80 
                {
                    embassy_time::block_for(embassy_time::Duration::from_millis(100));
                }
            }
            Err(e) => 
            {
                defmt::error!("Failed to write reg {:#04x}. Error: {:?}", reg, defmt::Debug2Format(&e));
                // break; // Stop trying if the bus crashes
            }
        }

        embassy_time::block_for(embassy_time::Duration::from_millis(2));
    }
    info!("Camera configuration complete!");



    // 7. Spawn the camera streaming task, passing dcmi, spi, and display pins
    spawner.spawn(tasks::camera_capture_task(dcmi)).unwrap();
    spawner.spawn(tasks::display_render_task(spi, cs_pin, dc_pin, rst_pin)).unwrap();

    // // 7. Spawn the TouchScreen display task, passing the SPI bus and pins
    // info!("Strating Display Task!");
    // spawner.spawn(touchscreen_display_task(spi, cs_pin, dc_pin, rst_pin)).unwrap();

    loop 
    {
        embassy_time::Timer::after_secs(100).await;
    }
}

