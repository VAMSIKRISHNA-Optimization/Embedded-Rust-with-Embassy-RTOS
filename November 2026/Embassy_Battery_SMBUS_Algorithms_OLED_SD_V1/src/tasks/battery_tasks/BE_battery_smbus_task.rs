use defmt::*;
use embassy_stm32::i2c::I2c;
use embassy_stm32::mode::Async;
use embassy_stm32::peripherals::I2C1;
use embassy_time::{Instant, Ticker, Duration};
use crate::telemetry::{RawBmsPacket, RAW_BMS_CHANNEL};

const SBS_ADDR: u8 = 0x0B;

// Standard SBS Command Codes
const CMD_TEMP: u8 = 0x08;
const CMD_VOLT: u8 = 0x09;
const CMD_CURR: u8 = 0x0A;
const CMD_RSOC: u8 = 0x0D;
const CMD_REM_CAP: u8 = 0x0F;
const CMD_FCC: u8 = 0x10;
const CMD_CYCLES: u8 = 0x17;

async fn read_sbs_u16(i2c: &mut I2c<'static, I2C1, Async>, cmd: u8) -> Result<u16, ()> {
    let mut buf = [0u8; 2];
    i2c.write_read(SBS_ADDR, &[cmd], &mut buf)
        .await
        .map_err(|_| ())?;
    Ok(u16::from_le_bytes(buf))
}

#[embassy_executor::task]
pub async fn BE_battery_smbus_data_acquisition_task(mut i2c: I2c<'static, I2C1, Async>) 
{
    info!("Task 1: SMBus Acquisition Started (0x0B @ 50 kHz)");
    let mut ticker = Ticker::every(Duration::from_secs(1));
    let start_instant = Instant::now();

    loop 
    {
        ticker.next().await;
        let elapsed_s = start_instant.elapsed().as_secs() as u32;

        // Read core SBS registers; if battery is unplugged, log warning and retry next tick
        let packet = match 
        (
            read_sbs_u16(&mut i2c, CMD_VOLT).await,
            read_sbs_u16(&mut i2c, CMD_CURR).await,
            read_sbs_u16(&mut i2c, CMD_TEMP).await,
            read_sbs_u16(&mut i2c, CMD_RSOC).await,
            read_sbs_u16(&mut i2c, CMD_REM_CAP).await,
            read_sbs_u16(&mut i2c, CMD_FCC).await,
            read_sbs_u16(&mut i2c, CMD_CYCLES).await,
        ) 
        {
            (Ok(v), Ok(i), Ok(t), Ok(rsoc), Ok(rem), Ok(fcc), Ok(cyc)) => RawBmsPacket 
            {
                timestamp_s: elapsed_s,
                voltage_mv: v,
                current_ma: i as i16, // SBS Current is signed two's complement
                temp_deci_k: t,
                bms_rsoc_pct: rsoc,
                bms_rem_cap_mah: rem,
                bms_fcc_mah: fcc,
                bms_cycle_count: cyc,
                bus_connected: true,
            },
            _ => 
            {
                warn!("SMBus NACK/Timeout - Waiting for battery pack...");
                RawBmsPacket {
                    timestamp_s: elapsed_s,
                    bus_connected: false,
                    ..Default::default()
                }
            }
        };

        RAW_BMS_CHANNEL.send(packet).await;
    }
}