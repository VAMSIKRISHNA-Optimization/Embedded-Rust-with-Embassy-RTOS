use defmt::*;
use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Ticker};

use crate::Integrations::ExtRTC_integration::ExtRTC_Driver::ExtRTC_Sd3078;
use crate::Integrations::ExtRTC_integration::ExtRTC_SharedData::
{
    ext_rtc_set_state,
    ExtRTC_Command,
    ExtRTC_DateTime,
    ExtRTC_State,
    EXT_RTC_CMD_CHANNEL,
};

use crate::ExtRTC_I2cBus;

#[embassy_executor::task]
pub async fn ext_rtc_task(i2c: ExtRTC_I2cBus)
{
    let mut rtc         = ExtRTC_Sd3078::new(i2c);
    let mut ticker      = Ticker::every(Duration::from_secs(1));
    let mut uptime_secs = 0u32;

    // 1. Initial boot ping
    match rtc.read_datetime().await
    {
        Ok(dt) =>
        {
            let valid = ExtRTC_Sd3078::<ExtRTC_I2cBus>::is_time_valid(&dt);
            info!("ExtRTC SD3078 detected! Valid time: {}", valid);
            ext_rtc_set_state(ExtRTC_State
            {
                datetime:   dt,
                is_synced:  valid,
                is_present: true,
            });
        }
        Err(_) =>
        {
            warn!("ExtRTC SD3078 not responding on I2C3; using dummy fallback timestamp.");
        }
    }

    // 2. Main loop: react immediately to UART SetTime OR 1-second tick
    loop
    {
        match select(EXT_RTC_CMD_CHANNEL.receive(), ticker.next()).await
        {
            Either::First(ExtRTC_Command::SetTime(new_dt)) =>
            {
                match rtc.set_datetime(new_dt).await
                {
                    Ok(_) =>
                    {
                        info!("ExtRTC updated via UART!");
                        ext_rtc_set_state(ExtRTC_State
                        {
                            datetime:   new_dt,
                            is_synced:  true,
                            is_present: true,
                        });
                    }
                    Err(_) =>
                    {
                        error!("ExtRTC I2C write failed!");
                    }
                }
            }
            Either::Second(_) =>
            {
                uptime_secs = uptime_secs.wrapping_add(1);

                match rtc.read_datetime().await
                {
                    Ok(dt) =>
                    {
                        let valid = ExtRTC_Sd3078::<ExtRTC_I2cBus>::is_time_valid(&dt);
                        ext_rtc_set_state(ExtRTC_State
                        {
                            datetime:   if valid { dt } else { dummy_from_uptime(uptime_secs) },
                            is_synced:  valid,
                            is_present: true,
                        });
                    }
                    Err(_) =>
                    {
                        ext_rtc_set_state(ExtRTC_State
                        {
                            datetime:   dummy_from_uptime(uptime_secs),
                            is_synced:  false,
                            is_present: false,
                        });
                    }
                }
            }
        }
    }
}

fn dummy_from_uptime(secs: u32) -> ExtRTC_DateTime
{
    ExtRTC_DateTime
    {
        year:   2000,
        month:  1,
        day:    1,
        hour:   ((secs / 3600) % 24) as u8,
        minute: ((secs / 60) % 60) as u8,
        second: (secs % 60) as u8,
    }
}