use super::registers::{BatteryMode, SpecificationInfo};

/// Applies VSCALE and IPSCALE multipliers from SpecificationInfo (0x1A) and BatteryMode (0x03)
#[derive(Debug, Clone, Copy)]
pub struct BatteryScaler 
{
    pub voltage_mult: u32,
    pub current_mult: u32,
    pub power_mode_10mwh: bool,
}

impl Default for BatteryScaler 
{
    fn default() -> Self 
    {
        Self 
        {
            voltage_mult: 1,
            current_mult: 1,
            power_mode_10mwh: false,
        }
    }
}

impl BatteryScaler 
{
    pub fn from_registers(spec: SpecificationInfo, mode: BatteryMode) -> Self 
    {
        let voltage_mult = match spec.v_scale_raw() 
        {
            0 => 1,
            _ => 10, // 1..=3 scales Voltage() & ChargingVoltage() by 10 mV
        };
        let current_mult = match spec.ip_scale_raw() 
        {
            0 => 1,
            1 => 10,
            _ => 100, // 2..=3 scales Current & Capacity by 100x
        };
        Self 
        {
            voltage_mult,
            current_mult,
            power_mode_10mwh: mode.capacity_mode_power(),
        }
    }

    /// Converts raw Voltage() or ChargingVoltage() to mV
    pub const fn pack_voltage_mv(self, raw: u16) -> u32 
    {
        (raw as u32) * self.voltage_mult
    }

    /// Converts raw Current() or AverageCurrent() to signed mA
    pub const fn current_ma(self, raw: i16) -> i32 
    {
        (raw as i32) * (self.current_mult as i32)
    }

    /// Converts raw RemainingCapacity() / FullChargeCapacity() / DesignCapacity() to mAh (when CAPM=0)
    pub const fn capacity_mah(self, raw: u16) -> u32 
    {
        (raw as u32) * self.current_mult
    }
}

/// Converts BQ78350 temperature (0.1 K) to degrees Celsius
pub fn deci_kelvin_to_celsius(raw_01k: u16) -> f32 
{
    (raw_01k as f32) * 0.1 - 273.15
}

/// Converts BQ78350 temperature (0.1 K) to integer 0.1 °C (avoids floating-point math)
pub const fn deci_kelvin_to_deci_celsius(raw_01k: u16) -> i32 
{
    (raw_01k as i32) - 2732
}

/// Decoded 0x1B ManufacturerDate()
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManufactureDate 
{
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl ManufactureDate 
{
    /// Format: Day + Month * 32 + (Year - 1980) * 512
    pub const fn decode(raw: u16) -> Self 
    {
        Self 
        {
            day: (raw & 0x001F) as u8,
            month: ((raw >> 5) & 0x000F) as u8,
            year: 1980 + ((raw >> 9) & 0x007F),
        }
    }
}

/// Decoded 0x0071 DAStatus1() (32-byte block: all 15 cell voltages in mV)
#[derive(Debug, Clone, Copy, Default)]
pub struct DaStatus1 
{
    pub cell_mv: [u16; 15],
}

impl DaStatus1 
{
    pub fn parse(payload: &[u8]) -> Option<Self> 
    {
        if payload.len() < 30 
        {
            return None;
        }
        let mut cell_mv = [0u16; 15];
        for (i, slot) in cell_mv.iter_mut().enumerate() 
        {
            let offset = i * 2;
            *slot = u16::from_le_bytes([payload[offset], payload[offset + 1]]);
        }
        Some(Self { cell_mv })
    }
}

/// Decoded 0x0072 DAStatus2() (16-byte block: auxiliary voltages and all temperature sensors in 0.1 K)
#[derive(Debug, Clone, Copy, Default)]
pub struct DaStatus2 
{
    pub ext_ave_cell_mv: u16,
    pub vaux_mv: u16,
    pub ts1_01k: u16,
    pub ts2_01k: u16,
    pub ts3_01k: u16,
    pub cell_temp_01k: u16,
    pub fet_temp_01k: u16,
    pub internal_temp_01k: u16,
}

impl DaStatus2 
{
    pub fn parse(payload: &[u8]) -> Option<Self> 
    {
        if payload.len() < 16 
        {
            return None;
        }
        let u16_at = |idx: usize| u16::from_le_bytes([payload[idx], payload[idx + 1]]);
        Some(Self 
        {
            ext_ave_cell_mv: u16_at(0),
            vaux_mv: u16_at(2),
            ts1_01k: u16_at(4),
            ts2_01k: u16_at(6),
            ts3_01k: u16_at(8),
            cell_temp_01k: u16_at(10),
            fet_temp_01k: u16_at(12),
            internal_temp_01k: u16_at(14),
        })
    }
}

/// Helper to decode 2-byte or 4-byte Little-Endian MAC status blocks (0x0050..0x0057)
pub fn decode_le_u16(payload: &[u8]) -> Option<u16> 
{
    if payload.len() < 2 
    {
        None
    } else {
        Some(u16::from_le_bytes([payload[0], payload[1]]))
    }
}

pub fn decode_le_u32(payload: &[u8]) -> Option<u32> 
{
    if payload.len() < 4 
    {
        None
    } else {
        Some(u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]))
    }
}