use defmt::*;
use crate::battery::battery_bms_telemetry::RawBmsData;
use crate::battery::battery_health_verdict::HealthVerdict;

const NOMINAL_CAPACITY_AH: f32 = 5.6; // 11S-2P 5.6Ah pack

#[embassy_executor::task]
pub async fn battery_algorithm_task() {
    info!("Task 2: EKF + DCIR + RNN Estimation Engine Started");
    let publisher = FUSED_PUBSUB.publisher().unwrap();

    let mut acc_ah: f64 = 0.0;
    let mut prev_v: Option<f32> = None;
    let mut prev_i: Option<f32> = None;
    let mut est_dcir_mohm: f32 = 85.0; // Initial 11S-2P pack resistance prior
    let mut est_soc: f32 = 50.0;
    let mut est_soh: f32 = 100.0;

    loop {
        let raw = RAW_BMS_CHANNEL.receive().await;

        if !raw.bus_connected {
            prev_v = None;
            prev_i = None;
            continue;
        }

        let v_pack = (raw.voltage_mv as f32) / 1000.0;
        let i_pack = (raw.current_ma as f32) / 1000.0; // +Charge, -Discharge

        // 1. High-precision f64 Coulomb Counting (dt = 1.0s)
        acc_ah += (i_pack as f64) / 3600.0;

        // 2. Online DCIR step detection when current shifts > 0.3A
        if let (Some(vp), Some(ip)) = (prev_v, prev_i) {
            let di = i_pack - ip;
            if libm::fabsf(di) > 0.3 {
                let dv = v_pack - vp;
                let instant_dcir = (dv / di) * 1000.0; // mOhm
                if instant_dcir > 10.0 && instant_dcir < 1000.0 {
                    // Exponential moving average filter (replace with your EKF R0 state)
                    est_dcir_mohm = 0.85 * est_dcir_mohm + 0.15 * instant_dcir;
                }
            }
        }
        prev_v = Some(v_pack);
        prev_i = Some(i_pack);

        // 3. TODO: Insert your nalgebra EKF/UKF update & RNN forward pass here
        // For initial bench bring-up, track Coulomb delta against nominal 5.6Ah:
        est_soc = (est_soc + (i_pack / 3600.0) / NOMINAL_CAPACITY_AH * 100.0).clamp(0.0, 100.0);

        // 4. Tamper Detection Logic
        let bms_claimed_soh = (raw.bms_fcc_mah as f32 / (NOMINAL_CAPACITY_AH * 1000.0)) * 100.0;
        let verdict = if libm::fabsf(acc_ah as f32) < 0.25 {
            HealthVerdict::Calibrating
        } else if bms_claimed_soh > 95.0 && est_soh < 82.0 {
            HealthVerdict::TamperedBms
        } else if est_soh < 80.0 {
            HealthVerdict::Degraded
        } else {
            HealthVerdict::Healthy
        };

        let fused = FusedBatteryState {
            raw,
            accumulated_ah: acc_ah as f32,
            est_soc_pct: est_soc,
            est_soh_pct: est_soh,
            est_dcir_mohm,
            verdict,
        };

        // Fan out to SD Logger, LCD Display, and future BLE simultaneously
        publisher.publish_immediate(fused);
    }
}