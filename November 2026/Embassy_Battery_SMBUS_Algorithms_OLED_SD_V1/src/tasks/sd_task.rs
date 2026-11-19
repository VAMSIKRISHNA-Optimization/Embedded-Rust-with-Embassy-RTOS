#![allow(non_snake_case)]
use super::*;

use core::fmt::Write;
use defmt::{error, info};
use embassy_time::Delay;
use embedded_sdmmc::{Mode, SdCard, TimeSource, Timestamp, VolumeIdx, VolumeManager};
use heapless::String;

use crate::tasks::{BE_BATTERY_LATEST_TELEMETRY, BE_BATTERY_SIG_DISPLAY_DONE};

/// Log interval divider: 5 ticks * 1 second/tick = 5.000 seconds
const BE_BATTERY_SD_LOG_INTERVAL_SEC: u32 = 5;

const BE_BATTERY_CSV_HEADER: &str = concat!(
    "S_No,Tick_s,Pack_V_mV,Current_mA,AvgCurrent_mA,RSOC_pct,ASOC_pct,SOH_pct,",
    "RemCap_mAh,FCC_mAh,CycleCount,PackTemp_C,CellMaxTemp_C,FetTemp_C,IcTemp_C,",
    "C1_mV,C2_mV,C3_mV,C4_mV,C5_mV,C6_mV,C7_mV,C8_mV,C9_mV,C10_mV,C11_mV,C12_mV,C13_mV,C14_mV,C15_mV,",
    "BatStatus_Hex,OpStatus_Hex,SafetyStatus_Hex,PFStatus_Hex\r\n"
);

pub struct BE_Battery_DummyTime;

impl TimeSource for BE_Battery_DummyTime 
{
    fn get_timestamp(&self) -> Timestamp 
    {
        Timestamp::from_calendar(2026, 11, 19, 9, 0, 0).unwrap()
    }
}

#[embassy_executor::task]
pub async fn sd_task(spi_device: SdSpiDevice) {
    info!("BE_Battery: Starting SD Card Task (5s logging interval)");

    let sdcard = SdCard::new(spi_device, Delay);
    let mut volume_mgr = VolumeManager::new(sdcard, BE_Battery_DummyTime);
    let mut s_no: u32 = 1;
    let mut csv_row: String<512> = String::new();

    loop {
        // 1. Wait strictly for touchscreen_display_task to complete
        let tick_count = BE_BATTERY_SIG_DISPLAY_DONE.wait().await;

        // 2. Only log every 5th second (ticks 5, 10, 15, ...)
        if tick_count % BE_BATTERY_SD_LOG_INTERVAL_SEC == 0 {
            let snapshot = {
                let guard = BE_BATTERY_LATEST_TELEMETRY.lock().await;
                guard.clone()
            };

            if let Some(t) = snapshot 
            {
                csv_row.clear();
                let _ = writeln!
                (
                    csv_row,
                    "{},{},{},{},{},{},{},{},{},{},{},{:.2},{:.2},{:.2},{:.2},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:#06x},{:#010x},{:#010x},{:#010x}\r",
                    s_no,
                    tick_count,
                    t.pack_voltage_mv,
                    t.current_ma,
                    t.avg_current_ma,
                    t.rsoc_pct,
                    t.asoc_pct,
                    t.soh_pct,
                    t.remaining_cap_mah,
                    t.full_charge_cap_mah,
                    t.cycle_count,
                    t.pack_temp_c,
                    t.cell_max_temp_c,
                    t.fet_temp_c,
                    t.ic_temp_c,
                    t.cell_voltages_mv[0],
                    t.cell_voltages_mv[1],
                    t.cell_voltages_mv[2],
                    t.cell_voltages_mv[3],
                    t.cell_voltages_mv[4],
                    t.cell_voltages_mv[5],
                    t.cell_voltages_mv[6],
                    t.cell_voltages_mv[7],
                    t.cell_voltages_mv[8],
                    t.cell_voltages_mv[9],
                    t.cell_voltages_mv[10],
                    t.cell_voltages_mv[11],
                    t.cell_voltages_mv[12],
                    t.cell_voltages_mv[13],
                    t.cell_voltages_mv[14],
                    t.battery_status.0,
                    t.operation_status.0,
                    t.safety_status.0,
                    t.pf_status.0,
                );

                match volume_mgr.open_volume(VolumeIdx(0)) 
                {
                    Ok(mut volume) => 
                    {
                        if let Ok(mut root_dir) = volume.open_root_dir() 
                        {
                            // Create /LOGS directory if it does not exist yet
                            let _ = root_dir.make_dir_in_dir("LOGS");

                            if let Ok(mut logs_dir) = root_dir.open_dir("LOGS") 
                            {
                                if let Ok(mut file) = logs_dir
                                    .open_file_in_dir("LOG.CSV", Mode::ReadWriteCreateOrAppend)
                                {
                                    // Write CSV header once if file is brand new / empty
                                    if file.length() == 0 
                                    {
                                        let _ = file.write(BE_BATTERY_CSV_HEADER.as_bytes());
                                    }

                                    if file.write(csv_row.as_bytes()).is_ok() 
                                    {
                                        info!(
                                            "BE_Battery [Tick {}s]: Logged row #{} to /LOGS/LOG.CSV",
                                            tick_count, s_no
                                        );
                                        s_no = s_no.wrapping_add(1);
                                    } else {
                                        error!("BE_Battery: SD file write failed");
                                    }
                                } else {
                                    error!("BE_Battery: Failed to open /LOGS/LOG.CSV");
                                }
                            } else {
                                error!("BE_Battery: Failed to open /LOGS directory");
                            }
                        }
                    }
                    Err(e) => 
                    {
                        error!
                        (
                            "BE_Battery: SD open_volume failed: {:?}",
                            defmt::Debug2Format(&e)
                        );
                    }
                }
            }
        }
    }
}