#![allow(non_snake_case)]
use super::*;


// -------------------------------------------------------------------
// 3. SD Card Task
// -------------------------------------------------------------------
use core::fmt::Write;
use embedded_sdmmc::{Mode, SdCard, TimeSource, Timestamp, VolumeIdx, VolumeManager};
use heapless::String;

// 3.1. Create a Dummy TimeSource for FAT file timestamps
struct DummyTime;

impl TimeSource for DummyTime 
{
    fn get_timestamp(&self) -> Timestamp 
    {
        Timestamp {
            year_since_1970: 56,   // 1970 + 56 = 2026
            zero_indexed_month: 9, // October (0-indexed)
            zero_indexed_day: 23,  // 24th (0-indexed)
            hours: 14,
            minutes: 49,
            seconds: 0,
        }
    }
}


// 3.2. The SD Card Task
#[embassy_executor::task]
pub async fn sd_task(spi_device: SdSpiDevice) 
{
    info!("BE_Battery: Starting SD Card Task (5s logging interval)");

    let mut telemetry_rx = BE_BATTERY_TELEMETRY_WATCH
        .receiver()
        .expect("Increase BE_BATTERY_TELEMETRY_SUBSCRIBERS in tasks.rs");

    // Initialize the SD card and volume manager
    let sdcard = SdCard::new(spi_device, Delay);
    let mut volume_mgr = VolumeManager::new(sdcard, DummyTime);
    let mut s_no = 1;

    loop 
    {
        // 1. Wait strictly for Task 3 (touchscreen_display_task) to finish
        let tick_count = BE_BATTERY_SIG_DISPLAY_DONE.wait().await;

        // 2. Only log every 5th second (ticks 5, 10, 15, ...)
        if tick_count % BE_BATTERY_SD_LOG_INTERVAL_SEC == 0 
        {
            // Pull the latest snapshot from the Watch (updates receiver's version_id)
            let telemetry = telemetry_rx.changed().await;

            info!
            (
                "BE_Battery [Tick {}s]: Writing telemetry row to SD card (V={} mV, I={} mA, RSOC={}%)",
                tick_count,
                telemetry.pack_voltage_mv,
                telemetry.current_ma,
                telemetry.rsoc_pct,
            );

            // Perform your SD card CSV/binary append here:
            // sd_logger.write_be_battery_record(tick_count, &telemetry);
            // Attempt to mount the volume and open the file
            if let Ok(mut volume) = volume_mgr.open_volume(VolumeIdx(0)) 
            {
                if let Ok(mut root_dir) = volume.open_root_dir() 
                {
                    if let Ok(mut logs_dir) = root_dir.open_dir("LOGS") 
                    {
                        if let Ok(mut file) = logs_dir.open_file_in_dir
                        (
                            "LOG.CSV", 
                            Mode::ReadWriteCreateOrAppend
                        ) 
                        {
                            // Write the CSV header only if the file was just created (length is 0)
                            if file.length() == 0 
                            {
                                info!("New file detected, writing CSV header...");
                                let _ = file.write(b"S.No,Phone Number,Date,Time\n");
                            }

                            // Format and write the data row
                            let mut buffer: String<64> = String::new();
                            if core::write!(&mut buffer, "{},{},2026-10-29,10:14:00\n", s_no, phone_number.as_str()).is_ok() 
                            {
                                info!("Writing data row to LOG.CSV...");
                                let _ = file.write(buffer.as_bytes());
                                let _ = file.flush(); // Required to update FAT directory entry
                                info!("LOG.CSV updated successfully!");
                                s_no += 1;
                            }
                        }
                    }
                }
            }
        }



    }
}