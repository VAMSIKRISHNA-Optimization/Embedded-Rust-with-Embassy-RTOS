use embassy_stm32::spi::Spi;
use embassy_stm32::peripherals::{SPI2, PB12};
use embassy_stm32::dma::NoDma;
use embassy_stm32::gpio::Output;
use embassy_time::{Delay, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_sdmmc::{Mode, SdCard, TimeSource, Timestamp, VolumeIdx, VolumeManager};
use defmt::{info, error};
use core::fmt::Write;
use heapless::String;

/// A dummy time source required by the FAT32 filesystem to timestamp files.
struct DummyClock;

impl TimeSource for DummyClock 
{
    fn get_timestamp(&self) -> Timestamp
    {
        Timestamp 
        {
            year_since_1970: 56, // 2026
            zero_indexed_month: 7, // August
            zero_indexed_day: 5,   // 5th
            hours: 10,
            minutes: 49,
            seconds: 0,
        }
    }
}

#[embassy_executor::task]
pub async fn sd_log_task(spi: Spi<'static, SPI2, NoDma, NoDma>, cs: Output<'static, PB12>) 
{
    info!("Initializing SD Card...");

    // Combine the SPI bus and CS pin into a single SpiDevice
    let spi_device = ExclusiveDevice::new(spi, cs, Delay).unwrap();

    // Initialize the SD Card driver
    let sdcard = SdCard::new(spi_device, Delay);
    
    // Attempt to read the card size to verify connection
    match sdcard.num_bytes() 
    {
        Ok(size) => info!("SD Card found! Size: {} bytes", size),
        Err(_) => 
        {
            error!("Failed to init SD card. Check wiring and formatting.");
            return;
        }
    }

    // Initialize the FAT32 Volume Manager
    let mut volume_mgr = VolumeManager::new(sdcard, DummyClock);

    let mut counter: u32 = 0;

    loop 
    {
        // embedded-sdmmc 0.8+ object-oriented API
        if let Ok(mut volume) = volume_mgr.open_volume(VolumeIdx(0)) 
        {
            if let Ok(mut root_dir) = volume.open_root_dir() 
            {
                if let Ok(mut file) = root_dir.open_file_in_dir(
                    "LOG.CSV", 
                    Mode::ReadWriteCreateOrAppend
                ) 
                {
                    let mut payload: String<32> = String::new();
                    let dummy_temp = 24.5 + (counter as f32 * 0.1);
                    
                    // Format: Timestamp, Counter, Temp\n
                    core::write!(&mut payload, "20260805,{},{}\n", counter, dummy_temp).unwrap();

                    // Write payload to the file
                    match file.write(payload.as_bytes()) 
                    {
                        Ok(_)  => info!("Logged CSV row: {}", payload.as_str()),
                        Err(_) => error!("Failed to write to file"),
                    }

                    // Flush buffer to ensure data is physically saved
                    let _ = file.flush();
                } 
                else 
                {
                    error!("Could not open or create LOG.CSV");
                }
            }
        }

        counter += 1;
        Timer::after_secs(2).await;
    }
}