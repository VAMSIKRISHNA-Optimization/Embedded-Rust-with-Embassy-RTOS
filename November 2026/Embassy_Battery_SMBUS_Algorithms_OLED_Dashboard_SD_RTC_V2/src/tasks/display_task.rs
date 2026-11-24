#![allow(non_snake_case)]

// ============================================================================
// Imports
// ============================================================================
use super::*;
use defmt::{error, info};
use display_interface_spi::SPIInterface;
use embassy_time::Delay;
use mipidsi::
{
    models::ILI9341Rgb565,
    options::{Orientation, Rotation},
    Builder,
};

use crate::Integrations::display_integration::{OLED_Dashboard, OLED_TelemetryView};
use crate::Integrations::ExtRTC_integration::ExtRTC_SharedData::ext_rtc_get_state;
use crate::tasks::
{
    BE_BATTERY_LATEST_TELEMETRY,
    BE_BATTERY_SIG_DEFMT_DONE,
    BE_BATTERY_SIG_DISPLAY_DONE,
};

// ============================================================================
// Display Task
// ============================================================================
#[embassy_executor::task]
pub async fn touchscreen_display_task
(
    spi_device: DisplaySpiDevice,
    dc_pin:     Output<'static, PC7>,
    rst_pin:    Output<'static, PA9>,
)
{
    info!("OLED: Starting Landscape Dashboard Task");

    // --- 1. Initialize ILI9341 in Landscape (320 x 240) ---
    let di      = SPIInterface::new(spi_device, dc_pin);
    let display = match Builder::new(ILI9341Rgb565, di)
        .reset_pin(rst_pin)
        .orientation(Orientation::new().rotate(Rotation::Deg90))
        .init(&mut Delay)
    {
        Ok(disp) => disp,
        Err(_) =>
        {
            error!("OLED: Display init failed!");
            loop
            {
                let tick = BE_BATTERY_SIG_DEFMT_DONE.wait().await;
                BE_BATTERY_SIG_DISPLAY_DONE.signal(tick);
            }
        }
    };

    // --- 2. Draw Static Dashboard Frame Once ---
    let mut dashboard = OLED_Dashboard::new(display);
    let _ = dashboard.draw_static_layout();

    // --- 3. Main Pipeline Loop (Partial Updates Only) ---
    loop
    {
        let tick_count = BE_BATTERY_SIG_DEFMT_DONE.wait().await;

        let snapshot =
        {
            let guard = BE_BATTERY_LATEST_TELEMETRY.lock().await;
            guard.clone()
        };

        let rtc_state = ext_rtc_get_state();

        if let Some(t) = snapshot
        {
            let mut min_cell_mv = u16::MAX;
            let mut max_cell_mv = 0u16;

            for &v in &t.cell_voltages_mv
            {
                if v > 0
                {
                    if v < min_cell_mv
                    {
                        min_cell_mv = v;
                    }
                    if v > max_cell_mv
                    {
                        max_cell_mv = v;
                    }
                }
            }

            if min_cell_mv == u16::MAX
            {
                min_cell_mv = 0;
            }

            let view = OLED_TelemetryView
            {
                pack_voltage_mv: t.pack_voltage_mv,
                current_ma:      t.current_ma,
                rsoc_pct:        t.rsoc_pct,
                soh_pct:         t.soh_pct,
                cycle_count:     t.cycle_count,
                min_cell_mv,
                max_cell_mv,
                cell_max_temp_c: t.cell_max_temp_c,
                fet_temp_c:      t.fet_temp_c,
                chg_fet_on:      t.operation_status.chg_fet_on(),
                dsg_fet_on:      t.operation_status.dsg_fet_on(),
                has_fault:       t.safety_status.any_active() || t.pf_status.any_active(),
                safety_status:   t.safety_status.0,
                pf_status:       t.pf_status.0,
            };

            let _ = dashboard.update_dashboard(&view, &rtc_state);
        }

        BE_BATTERY_SIG_DISPLAY_DONE.signal(tick_count);
    }
}