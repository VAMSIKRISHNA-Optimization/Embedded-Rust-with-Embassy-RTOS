#![allow(non_snake_case)]

pub mod commands;
pub mod helpers;
pub mod protocol;
pub mod registers;

pub mod BE_Battery_Init_Integration;

pub use commands::{MacCommand, SbsCommand};
pub use helpers::{BatteryScaler, DaStatus1, DaStatus2, ManufactureDate};
pub use protocol::{Bq78350, SmbusError, BQ78350_I2C_ADDR_7BIT};
pub use registers::
{
    BatteryMode, BatteryStatus, ChargingStatus, GaugingStatus, ManufacturingStatus,
    OperationStatus, PfFlags, SafetyFlags, SpecificationInfo,
};