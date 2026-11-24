use crate::battery_integration::battery_bms_telemetry::RawBmsData;
use crate::battery_integration::battery_health_verdict::HealthVerdict;

#[derive(Clone, Copy, Debug)]
pub struct CurrentBatteryState
{
    pub current_raw_BMS_data    : RawBmsData,
    pub current_accumulated_ah  : f32,
    pub current_est_soc_pct     : f32,
    pub current_est_soh_pct     : f32,
    pub current_est_dcir_mohm   : f32,
    pub current_verdict         : HealthVerdict,
}