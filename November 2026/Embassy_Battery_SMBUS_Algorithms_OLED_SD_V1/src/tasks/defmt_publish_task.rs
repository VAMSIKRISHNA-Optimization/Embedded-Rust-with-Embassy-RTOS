#![allow(non_snake_case)]

use defmt::{info, warn};

use crate::tasks::
{
    BE_BATTERY_SIG_DEFMT_DONE, BE_BATTERY_SIG_SMBUS_DONE, BE_BATTERY_TELEMETRY_WATCH,
};

/// Task 2: Publishes BE_BatteryTelemetry over RTT using defmt every 1 second,
/// strictly after BE_battery_smbus_data_acquisition_task completes.
#[embassy_executor::task]
pub async fn defmt_publish_task() 
{
    info!("BE_Battery: Starting defmt Publish Task");

    // Subscribe to the broadcast Watch
    let mut telemetry_rx = BE_BATTERY_TELEMETRY_WATCH
        .receiver()
        .expect("Increase BE_BATTERY_TELEMETRY_SUBSCRIBERS in tasks.rs");

    loop 
    {
        // 1. Wait strictly for Task 1 (SMBus acquisition) to finish this cycle
        let tick_count = BE_BATTERY_SIG_SMBUS_DONE.wait().await;

        // 2. Fetch the freshly published telemetry snapshot from the Watch
        let t = telemetry_rx.changed().await;

        // 3. Print structured telemetry to laptop via defmt
        info!
        (
            "=== BE_Battery [Tick {}s] ===",
            tick_count
        );

        info!
        (
            "PACK | V={} mV | I={} mA (Avg={} mA) | RSOC={}% | ASOC={}% | SOH={}% | RemCap={} mAh | FCC={} mAh",
            t.pack_voltage_mv,
            t.current_ma,
            t.avg_current_ma,
            t.rsoc_pct,
            t.asoc_pct,
            t.soh_pct,
            t.remaining_cap_mah,
            t.full_charge_cap_mah,
        );

        info!
        (
            "TEMP | Pack={} C | CellMax={} C | FET={} C | IC={} C | TS1={} C | TS2={} C | TS3={} C",
            t.pack_temp_c,
            t.cell_max_temp_c,
            t.fet_temp_c,
            t.ic_temp_c,
            t.ts1_temp_c,
            t.ts2_temp_c,
            t.ts3_temp_c,
        );

        info!
        (
            "CELLS (mV) | C1..5: [{}, {}, {}, {}, {}] | C6..10: [{}, {}, {}, {}, {}] | C11..15: [{}, {}, {}, {}, {}]",
            t.cell_voltages_mv[0], t.cell_voltages_mv[1], t.cell_voltages_mv[2], t.cell_voltages_mv[3], t.cell_voltages_mv[4],
            t.cell_voltages_mv[5], t.cell_voltages_mv[6], t.cell_voltages_mv[7], t.cell_voltages_mv[8], t.cell_voltages_mv[9],
            t.cell_voltages_mv[10], t.cell_voltages_mv[11], t.cell_voltages_mv[12], t.cell_voltages_mv[13], t.cell_voltages_mv[14],
        );

        if t.safety_status.any_active() || t.pf_status.any_active() 
        {
            warn!
            (
                "FAULTS | OpStatus={:#010x} | SafetyStatus={:#010x} | PFStatus={:#010x}",
                t.operation_status.0,
                t.safety_status.0,
                t.pf_status.0,
            );
        } else {
            info!
            (
                "STATUS | BatStatus={:#06x} | OpStatus={:#010x} | CHG_FET={} | DSG_FET={} | PCHG_FET={}",
                t.battery_status.0,
                t.operation_status.0,
                t.operation_status.chg_fet_on(),
                t.operation_status.dsg_fet_on(),
                t.operation_status.pchg_fet_on(),
            );
        }

        // 4. Hand off execution baton to Task 3 (touchscreen_display_task)
        BE_BATTERY_SIG_DEFMT_DONE.signal(tick_count);
    }
}