#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use defmt::info;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;

use crate::Integrations::battery_integration::battery_health_verdict::HealthVerdict;
use crate::tasks::BE_BATTERY_LATEST_TELEMETRY;
use crate::BE_Battery::BE_Battery_Init_Integration::BE_BatteryTelemetry;

const BE_BATTERY_NOMINAL_CAPACITY_AH: f32 = 5.6; // 11S-2P 5.6Ah pack

/// Optional trigger signal when you enable `BE_battery_algorithm_task` in the pipeline later
pub static BE_BATTERY_SIG_ALGO_START: Signal<CriticalSectionRawMutex, u32> = Signal::new();
pub static BE_BATTERY_SIG_ALGO_DONE: Signal<CriticalSectionRawMutex, u32>  = Signal::new();

#[derive(Debug, Clone)]
pub struct BE_BatteryFusedState 
{
    pub raw: BE_BatteryTelemetry,
    pub accumulated_ah: f32,
    pub est_soc_pct: f32,
    pub est_soh_pct: f32,
    pub est_dcir_mohm: f32,
    pub verdict: HealthVerdict,
}

pub static BE_BATTERY_LATEST_FUSED_STATE: Mutex
<
    CriticalSectionRawMutex,
    Option<BE_BatteryFusedState>,
> = Mutex::new(None);

#[inline]
const fn be_battery_abs_f32(x: f32) -> f32 
{
    if x < 0.0 
    {
        -x
    } else {
        x
    }
}

#[embassy_executor::task]
pub async fn BE_battery_algorithm_task() 
{
    info!("BE_Battery: EKF + DCIR Estimation Engine Ready (Standby)");

    let mut acc_ah: f64 = 0.0;
    let mut prev_v: Option<f32> = None;
    let mut prev_i: Option<f32> = None;
    let mut est_dcir_mohm: f32 = 85.0; // Initial 11S-2P pack resistance prior
    let mut est_soc: f32 = 50.0;
    let est_soh: f32 = 100.0;

    loop 
    {
        let tick_count = BE_BATTERY_SIG_ALGO_START.wait().await;

        let snapshot = 
        {
            let guard = BE_BATTERY_LATEST_TELEMETRY.lock().await;
            guard.clone()
        };

        if let Some(raw) = snapshot 
        {
            // Check PRES pin / system presence
            if !raw.operation_status.pres_low() 
            {
                prev_v = None;
                prev_i = None;
                BE_BATTERY_SIG_ALGO_DONE.signal(tick_count);
                continue;
            }

            let v_pack: f32 = (raw.pack_voltage_mv as f32) / 1000.0;
            let i_pack: f32 = (raw.current_ma as f32) / 1000.0; // +Charge, -Discharge

            // 1. High-precision f64 Coulomb Counting (dt = 1.0s)
            acc_ah += (i_pack as f64) / 3600.0;

            // 2. Online DCIR step detection when current shifts > 0.3A
            if let (Some(vp), Some(ip)) = (prev_v, prev_i) 
            {
                let di: f32 = i_pack - ip;
                if be_battery_abs_f32(di) > 0.3 
                {
                    let dv: f32 = v_pack - vp;
                    let instant_dcir: f32 = (dv / di) * 1000.0; // mOhm
                    if instant_dcir > 10.0 && instant_dcir < 1000.0 
                    {
                        est_dcir_mohm = 0.85 * est_dcir_mohm + 0.15 * instant_dcir;
                    }
                }
            }
            prev_v = Some(v_pack);
            prev_i = Some(i_pack);

            // 3. Coulomb delta against nominal 5.6Ah (placeholder for EKF/UKF update)
            est_soc = (est_soc + (i_pack / 3600.0) / BE_BATTERY_NOMINAL_CAPACITY_AH * 100.0)
                .clamp(0.0, 100.0);

            // 4. Tamper / Health Verdict Logic
            // let bms_claimed_soh: f32 =
            //     (raw.full_charge_cap_mah as f32 / (BE_BATTERY_NOMINAL_CAPACITY_AH * 1000.0)) * 100.0;
            // let verdict = if be_battery_abs_f32(acc_ah as f32) < 0.25 {
            //     HealthVerdict::Calibrating
            // } else if bms_claimed_soh > 95.0 && est_soc < 82.0 {
            //     HealthVerdict::TamperedBms
            // } else if est_soh < 80.0 {
            //     HealthVerdict::Degraded
            // } else {
            //     HealthVerdict::Healthy
            // };

            let fused = BE_BatteryFusedState 
            {
                raw,
                accumulated_ah: acc_ah as f32,
                est_soc_pct: est_soc,
                est_soh_pct: est_soh,
                est_dcir_mohm,
                verdict: HealthVerdict::Calibrating,
            };

            {
                let mut guard = BE_BATTERY_LATEST_FUSED_STATE.lock().await;
                *guard = Some(fused);
            }
        }

        BE_BATTERY_SIG_ALGO_DONE.signal(tick_count);
    }
}