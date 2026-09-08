use embassy_stm32::gpio::{Output};
use embassy_stm32::peripherals::{I2C1, PA5};
use embassy_time::Timer;       // osDelay from RTOS
use embassy_stm32::i2c::{I2c};

// use core::sync::atomic::{AtomicU8, Ordering};

// Global statics that your debugger can easily watch
// pub static LAST_WRITTEN_VALUE:  AtomicU8 = AtomicU8::new(0);
// pub static LAST_READ_VALUE:     AtomicU8 = AtomicU8::new(0);

// // Global statics exposed directly for the debugger
// #[unsafe(no_mangle)]
// #[used]
// pub static mut LAST_WRITTEN_VALUE: u8 = 0;

// #[unsafe(no_mangle)]
// #[used]
// pub static mut LAST_READ_VALUE   : u8 = 0;


// use core::sync::atomic::{AtomicU8, Ordering};

// #[unsafe(no_mangle)]
// #[used]
// pub static LAST_WRITTEN_VALUE: AtomicU8 = AtomicU8::new(0);

// #[unsafe(no_mangle)]
// #[used]
// pub static LAST_READ_VALUE: AtomicU8 = AtomicU8::new(0);

// 1. Define the background task
// We pass in an Output pin configured with a static lifetime.
// #[embassy_executor::task]
// pub async fn blink_task(mut led: Output<'static, PA5>)
// {
//     loop 
//     {
//         // Toggle the LED
//         led.toggle();
        
//         // Yield the CPU for 1000 milliseconds
//         Timer::after_millis(1000).await;
//     }
// }


const EEPROM_ADDR: u8 = 0x50; // Default I2C 7-bit address for SparkFun Qwiic EEPROM

#[embassy_executor::task]
pub async fn eeprom_task(mut i2c: I2c<'static, I2C1>, mut led: Output<'static, PA5>) 
{
    let mut test_counter: u8 = 1;

    loop 
    {
        // Target memory location: 0x0000 (Address High: 0x00, Address Low: 0x00)
        let mem_addr_hi: u8 = 0x00;
        let mem_addr_lo: u8 = 0x00;

        // --- 1. WRITE OPERATION ---
        // Payload format: [Mem_Addr_Hi, Mem_Addr_Lo, Data_Byte]
        let write_buf: [u8; 3] = [mem_addr_hi, mem_addr_lo, test_counter];
        
        // UPDATE GLOBAL WRITTEN VALUE HERE
        // LAST_WRITTEN_VALUE.store(test_counter, Ordering::Relaxed);
        
        // Raw unsafe write for debugger visibility
        // UPDATE GLOBAL WRITTEN VALUE HERE
        // Force the compiler to write to RAM so the debugger sees it
        // unsafe 
        // { 
        //     core::ptr::write_volatile(&raw mut LAST_WRITTEN_VALUE, test_counter); 
        // }

        // // Force the compiler to write directly to RAM so the debugger sees it
        // unsafe 
        // {    
        //     core::ptr::write_volatile(core::ptr::addr_of_mut!(LAST_WRITTEN_VALUE), test_counter);
        // }


        // Send write packet over I2C
        if let Err(_e) = i2c.blocking_write(EEPROM_ADDR, &write_buf)
        {
            // Write failed or device not connected
        }

        // CRITICAL: Give the EEPROM 10ms to finish its internal write cycle
        Timer::after_millis(10).await;

        // --- 2. READ OPERATION ---
        let mut read_buf : [u8; 1] = [0];
        let address_bytes: [u8; 2] = [mem_addr_hi, mem_addr_lo];

        // write_read transmits memory address bytes, then reads response into read_buf
        if let Ok(_) = i2c.blocking_write_read(EEPROM_ADDR, &address_bytes, &mut read_buf)
        {
            let read_value: u8 = read_buf[0];
            
            // UPDATE GLOBAL READ VALUE HERE
            // LAST_READ_VALUE.store(read_value, Ordering::Relaxed);

            // Raw unsafe write for debugger visibility
        // UPDATE GLOBAL READ VALUE HERE
        // Force the compiler to write to RAM so the debugger sees it
        // unsafe 
        // { 
        //     core::ptr::write_volatile(&raw mut LAST_READ_VALUE, read_value); 
        // }
        // unsafe 
        // {    
        //     core::ptr::write_volatile(core::ptr::addr_of_mut!(LAST_READ_VALUE), read_value);
        // }

            // --- LIVE RTT LOGGING ---
            defmt::info!("Live Data -> Written: {}, Read: {}", test_counter, read_value);

            if read_value == test_counter
            {
                // Read matched written value!
                led.toggle();
            }
        }

        // Increment payload for next cycle
        test_counter = test_counter.wrapping_add(1);

        // Sleep task for 10 seconds
        Timer::after_secs(1).await;
    }
}


