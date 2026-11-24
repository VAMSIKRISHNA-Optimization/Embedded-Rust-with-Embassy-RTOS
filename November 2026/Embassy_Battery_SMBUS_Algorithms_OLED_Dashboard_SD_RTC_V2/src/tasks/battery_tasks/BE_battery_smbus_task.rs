#![allow(non_snake_case)]

use defmt::{error, info, warn};
use embassy_time::{Duration, Ticker};

use crate::tasks::{BE_BATTERY_LATEST_TELEMETRY, BE_BATTERY_SIG_SMBUS_DONE};
use crate::BE_Battery::BE_Battery_Init_Integration::BE_BatterySmbusTask;
use crate::BE_Battery_I2cBus;

#[embassy_executor::task]
pub async fn BE_battery_smbus_data_acquisition_task
(
    mut be_smbus_driver: BE_BatterySmbusTask<BE_Battery_I2cBus>,
) 
{
    info!("BE_Battery: Starting SMBus Data Acquisition Task (1s Ticker)");

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

    let mut ticker = Ticker::every(Duration::from_secs(1));
    let mut tick_count: u32 = 0;

    loop 
    {
        ticker.next().await;
        tick_count = tick_count.wrapping_add(1);


        match be_smbus_driver.poll_telemetry() 
        {
            Ok(telemetry) => 
            {
                // Store latest telemetry snapshot in shared Mutex
                {
                    let mut guard = BE_BATTERY_LATEST_TELEMETRY.lock().await;
                    *guard = Some(telemetry);
                }
                // Pass execution baton to defmt_publish_task
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
            }
        }
    }
}