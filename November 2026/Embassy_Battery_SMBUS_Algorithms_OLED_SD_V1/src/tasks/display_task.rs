#![allow(non_snake_case)]
use super::*;


// -------------------------------------------------------------------
// 2. Display Task (QWERTY Keyboard Version)
// -------------------------------------------------------------------
#[embassy_executor::task]
pub async fn touchscreen_display_task(
    mut spi: DisplaySpiDevice, 
    dc: Output<'static, PC7>,
    rst: Output<'static, PA9>,
) 
{
    info!("BE_Battery: Starting OLED Display Task");

    let mut delay = Delay;
    let spi_interface = SPIInterface::new(spi, dc);

    let mut telemetry_rx = BE_BATTERY_TELEMETRY_WATCH
                            .receiver()
                            .expect("Increase BE_BATTERY_TELEMETRY_SUBSCRIBERS in tasks.rs");

    // Initialize the OLED display with the ILI9341 driver
    use mipidsi::options::{Orientation, Rotation};
    let builder = Builder::new(ILI9341Rgb565, spi_interface)
        .reset_pin(rst)
        .display_size(240, 320)
        .orientation(Orientation::new().rotate(Rotation::Deg90)); // Landscape

    let mut display = builder.init(&mut delay).unwrap();


    // 5. Idle loop
    loop 
    {
        // 1. Wait strictly for Task 2 (defmt_publish_task) to finish
        let tick_count = BE_BATTERY_SIG_DEFMT_DONE.wait().await;

        // 2. Read the latest BE_BatteryTelemetry snapshot
        let telemetry = telemetry_rx.changed().await;

        // 3. Render updated voltage, current, RSOC, and fault flags to OLED
        // display_integration::render_be_battery_screen(&mut display, &telemetry);
        let _ = (&mut spi, &mut dc, &mut rst, &telemetry);

        // 4. Hand off execution baton to Task 4 (sd_task)
        BE_BATTERY_SIG_DISPLAY_DONE.signal(tick_count);
    }
}

