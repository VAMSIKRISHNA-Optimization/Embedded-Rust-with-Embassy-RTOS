#![allow(non_snake_case)]

use defmt::{error, info, warn};
use embedded_hal::i2c::I2c;

use embassy_stm32::mode::Async;
use embassy_stm32::peripherals::I2C1;
use embassy_time::{Instant, Ticker, Duration};
use crate::telemetry::{RawBmsPacket, RAW_BMS_CHANNEL};

use crate::tasks::{BE_BATTERY_SIG_SMBUS_DONE, BE_BATTERY_TELEMETRY_WATCH};
use crate::BE_Battery::BE_Battery_Init_Integration::BE_BatterySmbusTask;

/// Task 1: Master 1-second SMBus polling task for the BQ78350-R1A.
/// Acquires battery telemetry every 1.000 s (drift-free), broadcasts it via Watch,
/// and triggers the sequential downstream pipeline.
#[embassy_executor::task]
pub async fn BE_battery_smbus_data_acquisition_task
(mut be_smbus_driver: I2c<'static, I2C1, Async>) 
{
    info!("BE_Battery: Starting SMBus Data Acquisition Task (1s Ticker)");

    // Obtain the broadcast sender handle once before entering the loop
    let telemetry_tx = BE_BATTERY_TELEMETRY_WATCH.sender();

    // Perform initial static pack metadata read (SpecificationInfo, Scaling, Serial, Strings)
    match be_smbus_driver.init() 
    {
        Ok(static_info) => 
        {
            info!
            (
                "BE_Battery: Init OK | DesignCap={} mAh | DesignV={} mV | Serial={:#06x} | V_mult={} | I_mult={}",
                static_info.design_capacity_mah,
                static_info.design_voltage_mv,
                static_info.serial_number,
                static_info.scaler.voltage_mult,
                static_info.scaler.current_mult,
            );
        }
        Err(e) => 
        {
            warn!
            (
                "BE_Battery: Initial SMBus read failed ({:?}), will retry inside 1s loop",
                defmt::Debug2Format(&e)
            );
        }
    }

    // Drift-free 1-second hardware ticker (equivalent to FreeRTOS vTaskDelayUntil)
    let mut ticker = Ticker::every(Duration::from_secs(1));
    let mut tick_count: u32 = 0;

    loop 
    {
        // 1. Sleep until the exact next 1.000-second boundary
        ticker.next().await;
        tick_count = tick_count.wrapping_add(1);

        // 2. Poll all SMBus telemetry & extended MAC diagnostics from BQ78350
        match be_smbus_driver.poll_telemetry() 
        {
            Ok(telemetry) => 
            {
                // 3. Broadcast latest snapshot to all subscribers (non-blocking overwrite)
                telemetry_tx.send(telemetry);

                // 4. Hand off execution baton to Task 2 (defmt_publish_task)
                BE_BATTERY_SIG_SMBUS_DONE.signal(tick_count);
            }
            Err(err) => 
            {
                error!
                (
                    "BE_Battery [Tick {}]: SMBus poll error: {:?}",
                    tick_count,
                    defmt::Debug2Format(&err)
                );
                // Note: We intentionally do NOT signal downstream tasks on an I2C failure
                // so stale/corrupt data is not logged or displayed as fresh.
            }
        }
    }
}