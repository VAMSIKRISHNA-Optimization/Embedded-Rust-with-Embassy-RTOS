#![allow(non_snake_case)]

use embedded_hal::i2c::I2c;

use crate::BE_Battery::
{
    helpers::
    {
        deci_kelvin_to_celsius, decode_le_u16, decode_le_u32, BE_Battery_BatteryScaler, BE_Battery_DaStatus1, BE_Battery_DaStatus2,
        BE_Battery_ManufactureDate,
    },
    BE_Battery_BatteryMode, BE_Battery_BatteryStatus, Bq78350, BE_Battery_ChargingStatus, BE_Battery_GaugingStatus,
    BE_Battery_ManufacturingStatus, BE_Battery_OperationStatus, BE_Battery_PfFlags, BE_Battery_SafetyFlags, BE_Battery_SmbusError,
    BE_Battery_SpecificationInfo,
};

use crate::BE_Battery::commands::BE_Battery_MacCommand;
use crate::BE_Battery::commands::BE_Battery_SbsCommand;


/// Static pack identity & scaling read once at task startup
#[derive(Debug, Clone)]
pub struct BE_BatteryStaticInfo 
{
    pub spec_info: BE_Battery_SpecificationInfo,
    pub battery_mode: BE_Battery_BatteryMode,
    pub scaler: BE_Battery_BatteryScaler,
    pub design_capacity_mah: u32,
    pub design_voltage_mv: u16,
    pub serial_number: u16,
    pub manufacture_date: BE_Battery_ManufactureDate,
    pub manufacturer_name: [u8; 20],
    pub manufacturer_name_len: usize,
    pub device_name: [u8; 8],
    pub device_name_len: usize,
    pub device_chemistry: [u8; 5],
    pub device_chemistry_len: usize,
}

/// Complete real-time telemetry snapshot updated on every task cycle
#[derive(Debug, Clone)]
pub struct BE_BatteryTelemetry 
{
    // Pack Electrical & Thermal
    pub pack_voltage_mv: u32,
    pub current_ma: i32,
    pub avg_current_ma: i32,
    pub dynamic_power_raw: i16,
    pub pack_temp_c: f32,
    pub ext_ave_cell_mv: u16,
    pub vaux_mv: u16,
    pub pending_edv_mv: u16,

    // State of Charge, Health & Capacity
    pub rsoc_pct: u8,
    pub asoc_pct: u8,
    pub soh_pct: u8,
    pub max_error_pct: u8,
    pub remaining_cap_mah: u32,
    pub full_charge_cap_mah: u32,
    pub cycle_count: u16,

    // Time & Charger Setpoints
    pub run_time_to_empty_min: u16,
    pub avg_time_to_empty_min: u16,
    pub avg_time_to_full_min: u16,
    pub charging_current_ma: u32,
    pub charging_voltage_mv: u32,

    // Individual Cells & Multi-Sensor Temperatures (from DAStatus1 & DAStatus2)
    pub cell_voltages_mv: [u16; 15],
    pub ts1_temp_c: f32,
    pub ts2_temp_c: f32,
    pub ts3_temp_c: f32,
    pub cell_max_temp_c: f32,
    pub fet_temp_c: f32,
    pub ic_temp_c: f32,

    // Status, Safety, Permanent Fail & Gauging Flags
    pub battery_status: BE_Battery_BatteryStatus,
    pub operation_status: BE_Battery_OperationStatus,
    pub charging_status: BE_Battery_ChargingStatus,
    pub gauging_status: BE_Battery_GaugingStatus,
    pub manufacturing_status: BE_Battery_ManufacturingStatus,
    pub safety_alert: BE_Battery_SafetyFlags,
    pub safety_status: BE_Battery_SafetyFlags,
    pub pf_alert: BE_Battery_PfFlags,
    pub pf_status: BE_Battery_PfFlags,
}

pub struct BE_BatterySmbusTask<I2C> 
{
    driver: Bq78350<I2C>,
    pub static_info: Option<BE_BatteryStaticInfo>,
}

impl<I2C, E> BE_BatterySmbusTask<I2C>
where
    I2C: I2c<Error = E>,
{
    pub fn new(i2c: I2C, use_pec: bool) -> Self 
    {
        Self 
        {
            driver: Bq78350::new(i2c, use_pec),
            static_info: None,
        }
    }

    /// Call once at startup (or after battery hot-plug) to cache scaling & pack metadata
    pub fn init(&mut self) -> Result<&BE_BatteryStaticInfo, BE_Battery_SmbusError<E>> 
    {
        let spec_info = BE_Battery_SpecificationInfo(self.driver.read_word(BE_Battery_SbsCommand::SpecificationInfo)?);
        let battery_mode = BE_Battery_BatteryMode(self.driver.read_word(BE_Battery_SbsCommand::BatteryMode)?);
        let scaler = BE_Battery_BatteryScaler::from_registers(spec_info, battery_mode);

        let raw_design_cap = self.driver.read_word(BE_Battery_SbsCommand::DesignCapacity)?;
        let design_voltage_mv = self.driver.read_word(BE_Battery_SbsCommand::DesignVoltage)?;
        let serial_number = self.driver.read_word(BE_Battery_SbsCommand::SerialNumber)?;
        let mfg_date_raw = self.driver.read_word(BE_Battery_SbsCommand::ManufacturerDate)?;

        let mut manufacturer_name = [0u8; 20];
        let manufacturer_name_len = self
            .driver
            .read_block(BE_Battery_SbsCommand::ManufacturerName, &mut manufacturer_name)?;

        let mut device_name = [0u8; 8];
        let device_name_len = self
            .driver
            .read_block(BE_Battery_SbsCommand::DeviceName, &mut device_name)?;

        let mut device_chemistry = [0u8; 5];
        let device_chemistry_len = self
            .driver
            .read_block(BE_Battery_SbsCommand::DeviceChemistry, &mut device_chemistry)?;

        self.static_info = Some(BE_BatteryStaticInfo 
        {
            spec_info,
            battery_mode,
            scaler,
            design_capacity_mah: scaler.capacity_mah(raw_design_cap),
            design_voltage_mv,
            serial_number,
            manufacture_date: BE_Battery_ManufactureDate::decode(mfg_date_raw),
            manufacturer_name,
            manufacturer_name_len,
            device_name,
            device_name_len,
            device_chemistry,
            device_chemistry_len,
        });

        Ok(self.static_info.as_ref().unwrap())
    }

    /// Call periodically (e.g. every 1000 ms) in your RTOS/async task loop to poll all telemetry
    pub fn poll_telemetry(&mut self) -> Result<BE_BatteryTelemetry, BE_Battery_SmbusError<E>> 
    {
        if self.static_info.is_none() 
        {
            self.init()?;
        }
        let scaler = self.static_info.as_ref().unwrap().scaler;

        // 1. Standard SBS Word Telemetry
        let raw_v = self.driver.read_word(BE_Battery_SbsCommand::Voltage)?;
        let raw_i = self.driver.read_word_signed(BE_Battery_SbsCommand::Current)?;
        let raw_avg_i = self.driver.read_word_signed(BE_Battery_SbsCommand::AverageCurrent)?;
        let dynamic_power_raw = self.driver.read_word_signed(BE_Battery_SbsCommand::DynamicPower)?;
        let raw_temp = self.driver.read_word(BE_Battery_SbsCommand::Temperature)?;

        let ext_ave_cell_mv = self.driver.read_word(BE_Battery_SbsCommand::ExtAveCellVoltage)?;
        let vaux_mv = self.driver.read_word(BE_Battery_SbsCommand::VauxVoltage)?;
        let pending_edv_mv = self.driver.read_word(BE_Battery_SbsCommand::PendingEdv)?;

        let rsoc_pct = (self.driver.read_word(BE_Battery_SbsCommand::RelativeStateOfCharge)? & 0xFF) as u8;
        let asoc_pct = (self.driver.read_word(BE_Battery_SbsCommand::AbsoluteStateOfCharge)? & 0xFF) as u8;
        let soh_pct = (self.driver.read_word(BE_Battery_SbsCommand::StateOfHealth)? & 0xFF) as u8;
        let max_error_pct = (self.driver.read_word(BE_Battery_SbsCommand::MaxError)? & 0xFF) as u8;

        let raw_rem_cap = self.driver.read_word(BE_Battery_SbsCommand::RemainingCapacity)?;
        let raw_fcc = self.driver.read_word(BE_Battery_SbsCommand::FullChargeCapacity)?;
        let cycle_count = self.driver.read_word(BE_Battery_SbsCommand::CycleCount)?;

        let run_time_to_empty_min = self.driver.read_word(BE_Battery_SbsCommand::RunTimeToEmpty)?;
        let avg_time_to_empty_min = self.driver.read_word(BE_Battery_SbsCommand::AverageTimeToEmpty)?;
        let avg_time_to_full_min = self.driver.read_word(BE_Battery_SbsCommand::AverageTimeToFull)?;
        let raw_chg_i = self.driver.read_word(BE_Battery_SbsCommand::ChargingCurrent)?;
        let raw_chg_v = self.driver.read_word(BE_Battery_SbsCommand::ChargingVoltage)?;

        let battery_status = BE_Battery_BatteryStatus(self.driver.read_word(BE_Battery_SbsCommand::BatteryStatus)?);

        // 2. Extended Diagnostics via 0x44 ManufacturerBlockAccess (Works even when SEALED)
        let mut buf = [0u8; 32];

        let n = self.driver.mac_read(BE_Battery_MacCommand::DaStatus1, &mut buf)?;
        let da1 = BE_Battery_DaStatus1::parse(&buf[..n]).unwrap_or_default();

        let n = self.driver.mac_read(BE_Battery_MacCommand::DaStatus2, &mut buf)?;
        let da2 = BE_Battery_DaStatus2::parse(&buf[..n]).unwrap_or_default();

        let n = self.driver.mac_read(BE_Battery_MacCommand::OperationStatus, &mut buf)?;
        let operation_status = BE_Battery_OperationStatus(decode_le_u32(&buf[..n]).unwrap_or(0));

        let n = self.driver.mac_read(BE_Battery_MacCommand::ChargingStatus, &mut buf)?;
        let charging_status = BE_Battery_ChargingStatus(decode_le_u16(&buf[..n]).unwrap_or(0));

        let n = self.driver.mac_read(BE_Battery_MacCommand::GaugingStatus, &mut buf)?;
        let gauging_status = BE_Battery_GaugingStatus(decode_le_u16(&buf[..n]).unwrap_or(0));

        let n = self.driver.mac_read(BE_Battery_MacCommand::ManufacturingStatus, &mut buf)?;
        let manufacturing_status = BE_Battery_ManufacturingStatus(decode_le_u16(&buf[..n]).unwrap_or(0));

        let n = self.driver.mac_read(BE_Battery_MacCommand::SafetyAlert, &mut buf)?;
        let safety_alert = BE_Battery_SafetyFlags(decode_le_u32(&buf[..n]).unwrap_or(0));

        let n = self.driver.mac_read(BE_Battery_MacCommand::SafetyStatus, &mut buf)?;
        let safety_status = BE_Battery_SafetyFlags(decode_le_u32(&buf[..n]).unwrap_or(0));

        let n = self.driver.mac_read(BE_Battery_MacCommand::PfAlert, &mut buf)?;
        let pf_alert = BE_Battery_PfFlags(decode_le_u16(&buf[..n]).unwrap_or(0) as u32);

        let n = self.driver.mac_read(BE_Battery_MacCommand::PfStatus, &mut buf)?;
        let pf_status = BE_Battery_PfFlags(decode_le_u32(&buf[..n]).unwrap_or(0));

        Ok(BE_BatteryTelemetry 
        {
            pack_voltage_mv: scaler.pack_voltage_mv(raw_v),
            current_ma: scaler.current_ma(raw_i),
            avg_current_ma: scaler.current_ma(raw_avg_i),
            dynamic_power_raw,
            pack_temp_c: deci_kelvin_to_celsius(raw_temp),
            ext_ave_cell_mv,
            vaux_mv,
            pending_edv_mv,
            rsoc_pct,
            asoc_pct,
            soh_pct,
            max_error_pct,
            remaining_cap_mah: scaler.capacity_mah(raw_rem_cap),
            full_charge_cap_mah: scaler.capacity_mah(raw_fcc),
            cycle_count,
            run_time_to_empty_min,
            avg_time_to_empty_min,
            avg_time_to_full_min,
            charging_current_ma: (raw_chg_i as u32) * scaler.current_mult,
            charging_voltage_mv: scaler.pack_voltage_mv(raw_chg_v),
            cell_voltages_mv: da1.cell_mv,
            ts1_temp_c: deci_kelvin_to_celsius(da2.ts1_01k),
            ts2_temp_c: deci_kelvin_to_celsius(da2.ts2_01k),
            ts3_temp_c: deci_kelvin_to_celsius(da2.ts3_01k),
            cell_max_temp_c: deci_kelvin_to_celsius(da2.cell_temp_01k),
            fet_temp_c: deci_kelvin_to_celsius(da2.fet_temp_01k),
            ic_temp_c: deci_kelvin_to_celsius(da2.internal_temp_01k),
            battery_status,
            operation_status,
            charging_status,
            gauging_status,
            manufacturing_status,
            safety_alert,
            safety_status,
            pf_alert,
            pf_status,
        })
    }

    /// Optional helper: Override CHG/DSG/PCHG FETs via HostFETControl (0x2B)
    pub fn command_fets(&mut self, pchg: bool, dsg: bool, chg: bool) -> Result<(), BE_Battery_SmbusError<E>> 
    {
        self.driver.set_host_fets(pchg, dsg, chg)
    }

    /// Optional helper: Enter SHUTDOWN mode (sends 0x0010 twice so it succeeds even if SEALED)
    pub fn request_shutdown(&mut self) -> Result<(), BE_Battery_SmbusError<E>> 
    {
        self.driver.mac_write(BE_Battery_MacCommand::ShutdownMode)?;
        self.driver.mac_write(BE_Battery_MacCommand::ShutdownMode)
    }

    /// Optional helper: Query AtRateTimeToEmpty (min) for a hypothetical load current (in scaled mA)
    pub fn query_at_rate_tte(&mut self, discharge_ma_scaled: u16) -> Result<u16, BE_Battery_SmbusError<E>> 
    {
        let neg_rate = -(discharge_ma_scaled as i16);
        self.driver.write_word(BE_Battery_SbsCommand::AtRate, neg_rate as u16)?;
        self.driver.read_word(BE_Battery_SbsCommand::AtRateTimeToEmpty)
    }
}