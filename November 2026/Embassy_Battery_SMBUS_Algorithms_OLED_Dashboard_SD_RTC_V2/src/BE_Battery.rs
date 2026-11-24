#![allow(non_snake_case)]

pub mod commands;
pub mod helpers;
pub mod protocol;
pub mod registers;

pub mod BE_Battery_Init_Integration;

pub use commands::{BE_Battery_MacCommand, BE_Battery_SbsCommand};
pub use helpers::{BE_Battery_BatteryScaler, BE_Battery_DaStatus1, BE_Battery_DaStatus2, BE_Battery_ManufactureDate};
pub use protocol::{Bq78350, BE_Battery_SmbusError, BQ78350_I2C_ADDR_7BIT};
pub use registers::
{
    BE_Battery_BatteryMode, BE_Battery_BatteryStatus, BE_Battery_ChargingStatus, BE_Battery_GaugingStatus, BE_Battery_ManufacturingStatus,
    BE_Battery_OperationStatus, BE_Battery_PfFlags, BE_Battery_SafetyFlags, BE_Battery_SpecificationInfo,
};