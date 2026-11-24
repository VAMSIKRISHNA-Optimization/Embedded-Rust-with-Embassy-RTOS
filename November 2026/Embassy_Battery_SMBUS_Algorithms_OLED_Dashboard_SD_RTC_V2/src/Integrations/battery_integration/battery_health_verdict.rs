#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HealthVerdict 
{
    Calibrating,
    Healthy(u8),    // SoH percentage
    Degraded(u8),   // SoH percentage
    TamperedBms,    // Triggered when BMS claims >95% SoH / low cycles, but true SoH is low
}