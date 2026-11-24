/// Standard and Extended SBS Command Codes (1-byte command byte on SMBus)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BE_Battery_SbsCommand 
{
    // --- Standard SBS Word / Block Commands (Accessible in SEALED, UNSEALED, FULL ACCESS) ---
    ManufacturerAccess = 0x00,      // R/W Word
    RemainingCapacityAlarm = 0x01,  // R/W Word (mAh or 10 mWh)
    RemainingTimeAlarm = 0x02,      // R/W Word (min)
    BatteryMode = 0x03,             // R/W Word (bitfield)
    AtRate = 0x04,                  // R/W Word (signed i16: mA or 10 mW)
    AtRateTimeToFull = 0x05,        // R   Word (min, 65535 = not charging)
    AtRateTimeToEmpty = 0x06,       // R   Word (min, 65535 = not discharging)
    AtRateOk = 0x07,                // R   Word (0 = False, >0 = True)
    Temperature = 0x08,             // R   Word (0.1 K)
    Voltage = 0x09,                 // R   Word (mV, scaled by VSCALE)
    Current = 0x0A,                 // R   Word (signed i16: mA, scaled by IPSCALE)
    AverageCurrent = 0x0B,          // R   Word (signed i16: mA, scaled by IPSCALE)
    MaxError = 0x0C,                // R   Word (u8 %, 0..100)
    RelativeStateOfCharge = 0x0D,   // R   Word (u8 %, 0..100)
    AbsoluteStateOfCharge = 0x0E,   // R   Word (u8 %, 0..100+)
    RemainingCapacity = 0x0F,       // R   Word (mAh or 10 mWh, scaled by IPSCALE)
    FullChargeCapacity = 0x10,      // R   Word (mAh or 10 mWh, scaled by IPSCALE)
    RunTimeToEmpty = 0x11,          // R   Word (min, 65535 = not discharging)
    AverageTimeToEmpty = 0x12,      // R   Word (min, 65535 = not discharging)
    AverageTimeToFull = 0x13,       // R   Word (min, 65535 = not charging)
    ChargingCurrent = 0x14,         // R   Word (mA, scaled by IPSCALE)
    ChargingVoltage = 0x15,         // R   Word (mV, scaled by VSCALE)
    BatteryStatus = 0x16,           // R   Word (bitfield + EC0..3)
    CycleCount = 0x17,              // R   Word (cycles)
    DesignCapacity = 0x18,          // R   Word (mAh or 10 mWh, scaled by IPSCALE)
    DesignVoltage = 0x19,           // R   Word (mV)
    SpecificationInfo = 0x1A,       // R   Word (IPSCALE | VSCALE | VER | REV)
    ManufacturerDate = 0x1B,        // R   Word (Day + Month*32 + (Year-1980)*512)
    SerialNumber = 0x1C,            // R   Word (u16 hex)
    ManufacturerName = 0x20,        // R   Block String (up to 20 bytes)
    DeviceName = 0x21,              // R   Block String (up to 7 bytes)
    DeviceChemistry = 0x22,         // R   Block String (up to 4 bytes, e.g. "LION")
    ManufacturerData = 0x23,        // R   Block (32 bytes; returns MAC response or default info)

    // --- Extended Control, GPIO, Auth & Cell Voltages ---
    HostFetControl = 0x2B,          // R/W Word (2-step unlock with 0x1197)
    GpioStatus = 0x2C,              // R   Word (bitfield)
    GpioControl = 0x2D,             // R/W Word (bitfield)
    VauxVoltage = 0x2E,             // R   Word (mV or 10 mV)
    Authenticate = 0x2F,            // R/W Block (20-byte SHA-1 HMAC / DF access)

    // NOTE: 0x30 is Reserved. Cells are ordered 15 down to 1!
    CellVoltage15 = 0x31,           // R   Word (mV, unscaled)
    CellVoltage14 = 0x32,           // R   Word (mV, unscaled)
    CellVoltage13 = 0x33,           // R   Word (mV, unscaled)
    CellVoltage12 = 0x34,           // R   Word (mV, unscaled)
    CellVoltage11 = 0x35,           // R   Word (mV, unscaled)
    CellVoltage10 = 0x36,           // R   Word (mV, unscaled)
    CellVoltage9 = 0x37,            // R   Word (mV, unscaled)
    CellVoltage8 = 0x38,            // R   Word (mV, unscaled)
    CellVoltage7 = 0x39,            // R   Word (mV, unscaled)
    CellVoltage6 = 0x3A,            // R   Word (mV, unscaled)
    CellVoltage5 = 0x3B,            // R   Word (mV, unscaled)
    CellVoltage4 = 0x3C,            // R   Word (mV, unscaled)
    CellVoltage3 = 0x3D,            // R   Word (mV, unscaled)
    CellVoltage2 = 0x3E,            // R   Word (mV, unscaled)
    CellVoltage1 = 0x3F,            // R   Word (mV, unscaled)

    ManufacturerBlockAccess = 0x44, // R/W Block (2-byte Little-Endian MAC command)
    DynamicPower = 0x4C,            // R   Word (signed i16: 10 mW, scaled)
    ExtAveCellVoltage = 0x4D,       // R   Word (mV)
    PendingEdv = 0x4E,              // R   Word (mV)
    StateOfHealth = 0x4F,           // R   Word (u8 %, 0..100)

    // --- Direct Block Reads (UNSEALED / FULL ACCESS only; use MAC 0x44 when SEALED) ---
    SafetyAlert = 0x50,             // R   Block (4 bytes)
    SafetyStatus = 0x51,            // R   Block (4 bytes)
    PfAlert = 0x52,                 // R   Block (2 bytes)
    PfStatus = 0x53,                // R   Block (4 bytes)
    OperationStatus = 0x54,         // R   Block (4 bytes)
    ChargingStatus = 0x55,          // R   Block (2 bytes)
    GaugingStatus = 0x56,           // R   Block (2 bytes)
    ManufacturingStatus = 0x57,     // R   Block (2 bytes)
    AfeStatus = 0x58,               // R   Block (1-2 bytes)
    AfeConfig = 0x59,               // R   Block (11 bytes)
    AfeVcx = 0x5A,                  // R   Block (30 bytes)
    AfeData = 0x5B,                 // R   Block (13 bytes)
    LifetimeDataBlock1 = 0x60,      // R   Block (30 bytes: Max Cell V 1..15)
    LifetimeDataBlock2 = 0x61,      // R   Block (30 bytes: Min Cell V 1..15)
    LifetimeDataBlock3 = 0x62,      // R   Block (14 bytes: Max Delta V, Currents, Power, Temps)
    LifetimeDataBlock4 = 0x63,      // R   Block (19 bytes: Shutdowns, CB Times)
    LifetimeDataBlock5 = 0x64,      // R   Block (14 bytes: Runtimes & Temp Region Times)
    LifetimeDataBlock6 = 0x65,      // R   Block (32 bytes: Safety Event Counts & Cycle Stamps)
    LifetimeDataBlock7 = 0x66,      // R   Block (32 bytes: OTF, VCT, FCC Update, CB Times)
    ManufacturerInfo = 0x70,        // R   Block (32 bytes - also readable in SEALED)
    DaStatus1 = 0x71,               // R   Block (32 bytes: Cell Voltages 1..15)
    DaStatus2 = 0x72,               // R   Block (16 bytes: ExtAveV, VAUX, TS1..3, Cell/FET/Int Temps)
    CuvSnapshot = 0x80,             // R   Block (32 bytes: Cell Voltages at CUV trip)
    CovSnapshot = 0x81,             // R   Block (32 bytes: Cell Voltages at COV trip)
}

impl BE_Battery_SbsCommand 
{
    /// Returns the SBS command byte for 1-based cell index `1..=15` (`0x3F` for Cell 1 .. `0x31` for Cell 15).
    pub const fn cell_voltage(cell_index_1_based: u8) -> Option<Self> 
    {
        match cell_index_1_based {
            1 => Some(Self::CellVoltage1),
            2 => Some(Self::CellVoltage2),
            3 => Some(Self::CellVoltage3),
            4 => Some(Self::CellVoltage4),
            5 => Some(Self::CellVoltage5),
            6 => Some(Self::CellVoltage6),
            7 => Some(Self::CellVoltage7),
            8 => Some(Self::CellVoltage8),
            9 => Some(Self::CellVoltage9),
            10 => Some(Self::CellVoltage10),
            11 => Some(Self::CellVoltage11),
            12 => Some(Self::CellVoltage12),
            13 => Some(Self::CellVoltage13),
            14 => Some(Self::CellVoltage14),
            15 => Some(Self::CellVoltage15),
            _ => None,
        }
    }
}

/// 16-bit ManufacturerAccess() / ManufacturerBlockAccess() (0x44) Subcommands
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BE_Battery_MacCommand 
{
    // Device Identification & Signatures (Available in SEALED mode)
    DeviceType = 0x0001,
    FirmwareVersion = 0x0002,
    HardwareVersion = 0x0003,
    IfChecksum = 0x0004,
    StaticDfSignature = 0x0005,
    ChemId = 0x0006,
    StaticChemDfSignature = 0x0008,
    AllDfSignature = 0x0009,

    // Power & Reset Controls (0x0010 works in SEALED if sent twice consecutively)
    ShutdownMode = 0x0010,
    SleepMode = 0x0011,
    DeviceReset = 0x0012,
    DeviceResetAlt = 0x0041,

    // Manufacturing & FET Test Toggles (UNSEALED / FULL ACCESS)
    CellBalanceToggle = 0x001B,
    AfeDelayDisable = 0x001C,
    SafeToggle = 0x001D,
    PrechargeFetToggle = 0x001E,
    ChargeFetToggle = 0x001F,
    DischargeFetToggle = 0x0020,
    GaugingToggle = 0x0021,
    FetControlToggle = 0x0022,
    LifetimeDataToggle = 0x0023,
    PermanentFailureToggle = 0x0024,
    BlackBoxRecorderToggle = 0x0025,
    SafeEnableToggle = 0x0026,
    LedDisplayEnableToggle = 0x0027,
    LifetimeDataReset = 0x0028,
    PermanentFailDataReset = 0x0029,
    BlackBoxRecorderReset = 0x002A,
    LedToggle = 0x002B,
    LedDisplayPress = 0x002C,
    CalibrationModeToggle = 0x002D,

    // Security & Boot ROM
    SealDevice = 0x0030,
    SecurityKeys = 0x0035,
    AuthenticationKey = 0x0037,
    RomMode = 0x0F00,

    // Status, Safety, Diagnostics & Snapshots (Available in SEALED mode via 0x44!)
    SafetyAlert = 0x0050,
    SafetyStatus = 0x0051,
    PfAlert = 0x0052,
    PfStatus = 0x0053,
    OperationStatus = 0x0054,
    ChargingStatus = 0x0055,
    GaugingStatus = 0x0056,
    ManufacturingStatus = 0x0057,
    AfeStatus = 0x0058,
    AfeConfig = 0x0059,
    AfeVcx = 0x005A,
    AfeData = 0x005B,
    LifetimeDataBlock1 = 0x0060,
    LifetimeDataBlock2 = 0x0061,
    LifetimeDataBlock3 = 0x0062,
    LifetimeDataBlock4 = 0x0063,
    LifetimeDataBlock5 = 0x0064,
    LifetimeDataBlock6 = 0x0065,
    LifetimeDataBlock7 = 0x0066,
    ManufacturerInfo = 0x0070,
    DaStatus1 = 0x0071,
    DaStatus2 = 0x0072,
    CuvSnapshot = 0x0080,
    CovSnapshot = 0x0081,

    // Calibration Output Modes
    ExitCalibrationOutput = 0xF080,
    OutputCellVoltageForCal = 0xF081,
    OutputCellVoltageCcTempForCal = 0xF082,
}