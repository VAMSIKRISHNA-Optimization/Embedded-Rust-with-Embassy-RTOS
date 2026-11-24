/// 0x03 BatteryMode() bitfields
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BE_Battery_BatteryMode(pub u16);

impl BE_Battery_BatteryMode 
{
    pub const fn raw(self) -> u16 { self.0 }
    /// Bit 15: 0 = mA/mAh (default), 1 = 10 mW / 10 mWh
    pub const fn capacity_mode_power(self) -> bool { (self.0 & (1 << 15)) != 0 }
    /// Bit 14: 1 = Disable ChargingVoltage/ChargingCurrent broadcasts
    pub const fn charger_broadcast_disabled(self) -> bool { (self.0 & (1 << 14)) != 0 }
    /// Bit 13: 1 = Disable AlarmWarning broadcasts
    pub const fn alarm_broadcast_disabled(self) -> bool { (self.0 & (1 << 13)) != 0 }
    /// Bit 7: Condition Flag (1 = Conditioning cycle requested)
    pub const fn condition_flag(self) -> bool { (self.0 & (1 << 7)) != 0 }
}

/// 0x1A SpecificationInfo() bitfields
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BE_Battery_SpecificationInfo(pub u16);

impl BE_Battery_SpecificationInfo 
{
    /// Bits 15:12 — Current/Capacity scale exponent (0 -> 1x, 1 -> 10x, 2/3 -> 100x)
    pub const fn ip_scale_raw(self) -> u8 { ((self.0 >> 12) & 0x0F) as u8 }
    /// Bits 11:8 — Pack Voltage scale exponent (0 -> 1x, 1..3 -> 10x)
    pub const fn v_scale_raw(self) -> u8 { ((self.0 >> 8) & 0x0F) as u8 }
    /// Bits 7:4 — SBS Version (0x3 = v1.1 with optional PEC)
    pub const fn version(self) -> u8 { ((self.0 >> 4) & 0x0F) as u8 }
    /// Bits 3:0 — Revision
    pub const fn revision(self) -> u8 { (self.0 & 0x0F) as u8 }
}

/// 0x16 BatteryStatus() alarms, state flags, and SMBus error code
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BE_Battery_BatteryStatus(pub u16);

impl BE_Battery_BatteryStatus 
{
    pub const fn overcharged_alarm(self) -> bool { (self.0 & (1 << 15)) != 0 }         // OCA
    pub const fn terminate_charge_alarm(self) -> bool { (self.0 & (1 << 14)) != 0 }    // TCA
    pub const fn overtemperature_alarm(self) -> bool { (self.0 & (1 << 12)) != 0 }     // OTA
    pub const fn terminate_discharge_alarm(self) -> bool { (self.0 & (1 << 11)) != 0 } // TDA
    pub const fn remaining_capacity_alarm(self) -> bool { (self.0 & (1 << 9)) != 0 }   // RCA
    pub const fn remaining_time_alarm(self) -> bool { (self.0 & (1 << 8)) != 0 }       // RTA
    pub const fn initialized(self) -> bool { (self.0 & (1 << 7)) != 0 }                // INIT
    /// Bit 6: 1 = Discharging or at rest, 0 = Charging
    pub const fn discharging_or_rest(self) -> bool { (self.0 & (1 << 6)) != 0 }        // DSG
    pub const fn fully_charged(self) -> bool { (self.0 & (1 << 5)) != 0 }              // FC
    pub const fn fully_discharged(self) -> bool { (self.0 & (1 << 4)) != 0 }           // FD
    /// Bits 3:0: 0=OK, 1=Busy, 2=Reserved, 3=Unsupported, 4=AccessDenied, 5=Overflow, 6=BadSize, 7=Unknown
    pub const fn error_code(self) -> u8 { (self.0 & 0x000F) as u8 }
}

/// 0x0050 SafetyAlert() / 0x0051 SafetyStatus() 32-bit Protection Flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BE_Battery_SafetyFlags(pub u32);

impl BE_Battery_SafetyFlags 
{
    pub const fn any_active(self) -> bool { self.0 != 0 }
    pub const fn cuv(self) -> bool { (self.0 & (1 << 0)) != 0 }       // Cell Undervoltage
    pub const fn cov(self) -> bool { (self.0 & (1 << 1)) != 0 }       // Cell Overvoltage
    pub const fn occ(self) -> bool { (self.0 & (1 << 2)) != 0 }       // Overcurrent in Charge
    pub const fn ocd(self) -> bool { (self.0 & (1 << 3)) != 0 }       // Overcurrent in Discharge
    pub const fn aold(self) -> bool { (self.0 & (1 << 4)) != 0 }      // AFE Overload in Discharge
    pub const fn aold_latch(self) -> bool { (self.0 & (1 << 5)) != 0 }// AFE Overload Latch
    pub const fn ascd(self) -> bool { (self.0 & (1 << 6)) != 0 }      // AFE Short Circuit in Discharge
    pub const fn ascd_latch(self) -> bool { (self.0 & (1 << 7)) != 0 }// AFE Short Circuit Latch
    pub const fn otc(self) -> bool { (self.0 & (1 << 8)) != 0 }       // Overtemperature in Charge
    pub const fn otd(self) -> bool { (self.0 & (1 << 9)) != 0 }       // Overtemperature in Discharge
    pub const fn utc(self) -> bool { (self.0 & (1 << 10)) != 0 }      // Undertemperature in Charge
    pub const fn utd(self) -> bool { (self.0 & (1 << 11)) != 0 }      // Undertemperature in Discharge
    pub const fn afe_ovrd(self) -> bool { (self.0 & (1 << 12)) != 0 } // AFE External Override Alert
    pub const fn otf(self) -> bool { (self.0 & (1 << 13)) != 0 }      // Overtemperature FET
    pub const fn ocd_latch(self) -> bool { (self.0 & (1 << 14)) != 0 }// Overcurrent Discharge Latch
    pub const fn pto(self) -> bool { (self.0 & (1 << 16)) != 0 }      // Precharge Timeout (Status)
    pub const fn ptos(self) -> bool { (self.0 & (1 << 17)) != 0 }     // Precharge Timeout Suspend (Alert)
    pub const fn cto(self) -> bool { (self.0 & (1 << 18)) != 0 }      // Fast Charge Timeout (Status)
    pub const fn ctos(self) -> bool { (self.0 & (1 << 19)) != 0 }     // Charge Timeout Suspend (Alert)
    pub const fn oc(self) -> bool { (self.0 & (1 << 20)) != 0 }       // Overcharge
}

/// 0x0052 PFAlert() (lower 16 bits) / 0x0053 PFStatus() (32 bits) Permanent Fail Flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BE_Battery_PfFlags(pub u32);

impl BE_Battery_PfFlags 
{
    pub const fn any_active(self) -> bool { self.0 != 0 }
    pub const fn suv(self) -> bool { (self.0 & (1 << 0)) != 0 }       // Safety Cell Undervoltage
    pub const fn sov(self) -> bool { (self.0 & (1 << 1)) != 0 }       // Safety Cell Overvoltage
    pub const fn socc(self) -> bool { (self.0 & (1 << 2)) != 0 }      // Safety Overcurrent Charge
    pub const fn socd(self) -> bool { (self.0 & (1 << 3)) != 0 }      // Safety Overcurrent Discharge
    pub const fn sot(self) -> bool { (self.0 & (1 << 4)) != 0 }       // Safety Overtemperature Cell
    pub const fn vimr(self) -> bool { (self.0 & (1 << 5)) != 0 }      // Voltage Imbalance at Rest
    pub const fn cfetf(self) -> bool { (self.0 & (1 << 6)) != 0 }     // Charge FET Failure
    pub const fn dfetf(self) -> bool { (self.0 & (1 << 7)) != 0 }     // Discharge FET Failure
    pub const fn afer(self) -> bool { (self.0 & (1 << 8)) != 0 }      // AFE Register Failure
    pub const fn afec(self) -> bool { (self.0 & (1 << 9)) != 0 }      // AFE Communication Failure
    pub const fn afe_ovrd(self) -> bool { (self.0 & (1 << 10)) != 0 } // AFE Override Failure
    pub const fn afe_xrdy(self) -> bool { (self.0 & (1 << 11)) != 0 } // AFE XREADY Failure
    pub const fn ts1_open(self) -> bool { (self.0 & (1 << 12)) != 0 } // Open Thermistor TS1
    pub const fn ts2_open(self) -> bool { (self.0 & (1 << 13)) != 0 } // Open Thermistor TS2
    pub const fn ts3_open(self) -> bool { (self.0 & (1 << 14)) != 0 } // Open Thermistor TS3
    pub const fn sotf(self) -> bool { (self.0 & (1 << 15)) != 0 }     // Safety Overtemperature FET
    pub const fn ifc(self) -> bool { (self.0 & (1 << 16)) != 0 }      // Instruction Flash Checksum (PFStatus)
    pub const fn dfw(self) -> bool { (self.0 & (1 << 17)) != 0 }      // Data Flash Wearout (PFStatus)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BE_Battery_SecurityMode 
{
    Reserved,
    FullAccess,
    Unsealed,
    Sealed,
}

/// 0x0054 OperationStatus() 32-bit System & FET State
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BE_Battery_OperationStatus(pub u32);

impl BE_Battery_OperationStatus 
{
    pub const fn pres_low(self) -> bool { (self.0 & (1 << 0)) != 0 }         // System present
    pub const fn chg_fet_on(self) -> bool { (self.0 & (1 << 1)) != 0 }       // CHG FET active
    pub const fn dsg_fet_on(self) -> bool { (self.0 & (1 << 2)) != 0 }       // DSG FET active
    pub const fn pchg_fet_on(self) -> bool { (self.0 & (1 << 3)) != 0 }      // Precharge FET active
    pub const fn host_fet_control(self) -> bool { (self.0 & (1 << 4)) != 0 } // HCFET
    pub const fn safe_pin_active(self) -> bool { (self.0 & (1 << 5)) != 0 }  // SAFE
    pub const fn security_mode(self) -> BE_Battery_SecurityMode 
    {
        match (self.0 >> 8) & 0x03 
        {
            0b01 => BE_Battery_SecurityMode::FullAccess,
            0b10 => BE_Battery_SecurityMode::Unsealed,
            0b11 => BE_Battery_SecurityMode::Sealed,
            _ => BE_Battery_SecurityMode::Reserved,
        }
    }
    pub const fn shutdown_low_v(self) -> bool { (self.0 & (1 << 10)) != 0 }  // SDV
    pub const fn safety_mode(self) -> bool { (self.0 & (1 << 11)) != 0 }     // SS
    pub const fn permanent_fail(self) -> bool { (self.0 & (1 << 12)) != 0 }  // PF
    pub const fn xdsg(self) -> bool { (self.0 & (1 << 13)) != 0 }            // Discharging disabled
    pub const fn xchg(self) -> bool { (self.0 & (1 << 14)) != 0 }            // Charging disabled
    pub const fn sleep_conditions_met(self) -> bool { (self.0 & (1 << 15)) != 0 }
    pub const fn shutdown_cmd(self) -> bool { (self.0 & (1 << 16)) != 0 }    // SDM
    pub const fn sleep_mode(self) -> bool { (self.0 & (1 << 23)) != 0 }      // SLEEPM
    pub const fn init_active(self) -> bool { (self.0 & (1 << 24)) != 0 }     // INIT
    pub const fn cell_balancing(self) -> bool { (self.0 & (1 << 28)) != 0 }  // CB
    pub const fn keyin_detected(self) -> bool { (self.0 & (1 << 31)) != 0 }  // KEYIN
}

/// 0x0055 ChargingStatus() 16-bit Charge Algorithm & Temp Region Flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BE_Battery_ChargingStatus(pub u16);

impl BE_Battery_ChargingStatus 
{
    pub const fn under_temp(self) -> bool { (self.0 & (1 << 0)) != 0 }        // UT
    pub const fn low_temp(self) -> bool { (self.0 & (1 << 1)) != 0 }          // LT
    pub const fn standard_temp(self) -> bool { (self.0 & (1 << 2)) != 0 }     // ST
    pub const fn high_temp(self) -> bool { (self.0 & (1 << 3)) != 0 }         // HT
    pub const fn over_temp(self) -> bool { (self.0 & (1 << 4)) != 0 }         // OT
    pub const fn precharge_mode(self) -> bool { (self.0 & (1 << 8)) != 0 }    // PCHG
    pub const fn fast_charge_mode(self) -> bool { (self.0 & (1 << 9)) != 0 }  // FCHG
    pub const fn charge_inhibit(self) -> bool { (self.0 & (1 << 12)) != 0 }   // IN
    pub const fn charge_suspend(self) -> bool { (self.0 & (1 << 13)) != 0 }   // SU
    pub const fn valid_charge_term(self) -> bool { (self.0 & (1 << 15)) != 0 }// VCT
}

/// 0x0056 GaugingStatus() 16-bit CEDV Gauging Flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BE_Battery_GaugingStatus(pub u16);

impl BE_Battery_GaugingStatus 
{
    pub const fn fully_discharged(self) -> bool { (self.0 & (1 << 0)) != 0 }  // FD
    pub const fn fully_charged(self) -> bool { (self.0 & (1 << 1)) != 0 }     // FC
    pub const fn terminate_discharge(self) -> bool { (self.0 & (1 << 2)) != 0 }// TD
    pub const fn terminate_charge(self) -> bool { (self.0 & (1 << 3)) != 0 }  // TC
    pub const fn bal_ok(self) -> bool { (self.0 & (1 << 4)) != 0 }            // BAL_OK
    pub const fn edv0(self) -> bool { (self.0 & (1 << 5)) != 0 }              // EDV0 reached
    pub const fn dsg(self) -> bool { (self.0 & (1 << 6)) != 0 }               // 1 = Discharge/Relax
    pub const fn condition_flag(self) -> bool { (self.0 & (1 << 7)) != 0 }    // CF
    pub const fn rest(self) -> bool { (self.0 & (1 << 8)) != 0 }              // REST (OCV taken)
    pub const fn fcc_updated(self) -> bool { (self.0 & (1 << 10)) != 0 }      // FCCX
    pub const fn edv1(self) -> bool { (self.0 & (1 << 13)) != 0 }             // EDV1 reached
    pub const fn edv2(self) -> bool { (self.0 & (1 << 14)) != 0 }             // EDV2 reached
    pub const fn discharge_qualified(self) -> bool { (self.0 & (1 << 15)) != 0 } // VDQ
}

/// 0x0057 ManufacturingStatus() 16-bit Feature Enable Flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BE_Battery_ManufacturingStatus(pub u16);

impl BE_Battery_ManufacturingStatus 
{
    pub const fn fet_action_enabled(self) -> bool { (self.0 & (1 << 4)) != 0 }      // FET_EN
    pub const fn lifetime_enabled(self) -> bool { (self.0 & (1 << 5)) != 0 }        // LF_EN
    pub const fn permanent_fail_enabled(self) -> bool { (self.0 & (1 << 6)) != 0 }  // PF_EN
    pub const fn black_box_enabled(self) -> bool { (self.0 & (1 << 7)) != 0 }       // BBR_EN
    pub const fn safe_action_enabled(self) -> bool { (self.0 & (1 << 8)) != 0 }     // SAFE_EN
    pub const fn led_enabled(self) -> bool { (self.0 & (1 << 9)) != 0 }             // LED_EN
    pub const fn cal_enabled(self) -> bool { (self.0 & (1 << 15)) != 0 }            // CAL_EN
}