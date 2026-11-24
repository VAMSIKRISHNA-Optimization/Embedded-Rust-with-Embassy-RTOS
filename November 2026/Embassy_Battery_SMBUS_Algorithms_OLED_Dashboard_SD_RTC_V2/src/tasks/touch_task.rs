use super::*;


// -------------------------------------------------------------------
// 1. Touch Task (Handles edge transitions + pre-existing low state)
// -------------------------------------------------------------------
#[embassy_executor::task]
pub async fn touchscreen_touch_task
(
    mut spi: Spi<'static, SPI2, NoDma, NoDma>,
    mut cs: Output<'static, PB12>,
    mut irq: ExtiInput<'static, PC5>, 
) 
{
    info!("Touch Task Started!");

    // CRITICAL: Ensure SPI frequency is reconfigured to <= 2 MHz here 
    // if this bus is shared with the display!

    cs.set_low();
    let _ = spi.blocking_transfer(&mut [0u8; 1], &[0x90]); // Send wake up
    cs.set_high();

    info!("Touch Task Initialized. Listening for touch events...");

    loop 
    {
        // if irq.is_high() 
        // {
        //     irq.wait_for_falling_edge().await;
        // }
        irq.wait_for_falling_edge().await;
    
        // 2. Glitch Filter: Wait 5ms for physical contact to stabilize
        embassy_time::Timer::after_millis(5).await;

        // 3. If the pin bounced back HIGH, it was SPI noise! Skip the read.
        if irq.is_high() {
            continue;
        }

        info!("Real Screen Touch Confirmed! Extracting coordinates...");
        block_for(Duration::from_millis(1));

        cs.set_low();
        
        // 1. Send X command (0x90)
        let _ = spi.blocking_transfer(&mut [0u8; 1], &[0x90]);
        // Block the CPU for 100 microseconds to let the ADC convert
        block_for(Duration::from_micros(100));
        
        // 2. Read 2 bytes for X
        let mut x_buf = [0u8; 2];
        let _ = spi.blocking_transfer(&mut x_buf, &[0x00, 0x00]);
        // Block the CPU for 100 microseconds to let the ADC convert
        block_for(Duration::from_micros(100));

        // 3. Send Y command (0xD0)
        let _ = spi.blocking_transfer(&mut [0u8; 1], &[0xD0]);
        // Block the CPU for 100 microseconds to let the ADC convert
        block_for(Duration::from_micros(100));
        
        // 4. Read 2 bytes for Y
        let mut y_buf = [0u8; 2];
        let _ = spi.blocking_transfer(&mut y_buf, &[0x00, 0x00]);
        // Block the CPU for 100 microseconds to let the ADC convert
        block_for(Duration::from_micros(100));


        // 5. Send command to power down ADC and enable PENIRQ again
        let _ = spi.blocking_transfer(&mut [0u8; 1], &[0x90]); 
        // Block the CPU for 100 microseconds to let the ADC convert
        block_for(Duration::from_micros(100));
        
        cs.set_high();

        // The XPT2046 returns 1 dummy bit, 12 data bits, and 3 trailing zeros.
        // Shifting by 3 correctly aligns the 12-bit value.
        let raw_x = (((x_buf[0] as u16) << 8) | (x_buf[1] as u16)) >> 3;
        let raw_y = (((y_buf[0] as u16) << 8) | (y_buf[1] as u16)) >> 3;

        info!("Raw Touch -> X: {}, Y: {}", raw_x, raw_y);

        TOUCH_CHANNEL.send((raw_x, raw_y)).await;

        // 4. Wait until the user completely LIFTS their finger off the screen
        while irq.is_low() 
        {
            embassy_time::Timer::after_millis(10).await;
        }
        info!("Touch Released!");

        // if raw_x < 4095 && raw_y < 4095 && raw_x > 100 && raw_y > 100 
        // {
        //     TOUCH_SIGNAL.signal(true);
        // }
        
        // embassy_time::Timer::after_millis(20).await; // Debounce delay to avoid multiple triggers from a single touch event
    }
}