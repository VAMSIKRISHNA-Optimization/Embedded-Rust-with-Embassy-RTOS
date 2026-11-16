use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::pubsub::PubSubChannel;

use crate::battery::battery_state::CurrentBatteryState;

pub static RAW_BMS_CHANNEL: Channel<CriticalSectionRawMutex, RawBmsData, 4> = Channel::new();
pub static CURRENT_PUBSUB : PubSubChannel<CriticalSectionRawMutex, CurrentBatteryState, 4, 3, 1> = PubSubChannel::new();

#[derive(Clone, Copy, Debug, Default)]
pub struct RawBmsData 
{
    // --- System & Timing ---
    pub Timestamp_SECONDS   : u32,
    pub Bms_Heartbeat_COUNT : u8,
    pub Bms_State_ENUM      : u8,         // e.g., 0=Idle, 1=Charging, 2=Discharging, 3=Fault
    pub Bus_Connected_BOOL  : bool,


    // --- Pack Electrical Measurements ---
    pub Pack_Voltage_VOLTS          : f32,
    pub Pack_Current_AMPS           : f32, // Signed: positive = discharge, negative = charge
    pub Pack_Power_WATTS            : f32,
    pub Insulation_Resistance_KOHMS : u32,


    // --- Cell Voltage Extremes & Balancing ---
    pub Cell_Voltage_Max_mV         : u16,
    pub Cell_Voltage_Min_mV         : u16,
    pub Cell_Voltage_Avg_mV         : u16,
    pub Cell_Voltage_Delta_mV       : u16, // Max minus Min (imbalance)
    pub Cell_Max_Id_INDEX           : u16, // Which cell has the highest voltage
    pub Cell_Min_Id_INDEX           : u16, // Which cell has the lowest voltage
    pub Cell_Balancing_Bitmask_BITS : u64, // Bitmask showing active balancing cells (up to 64S)


    // --- Thermal Measurements ---
    pub Pack_Temp_Avg_DEGC      : i16,
    pub Cell_Temp_Max_DEGC      : i16,
    pub Cell_Temp_Min_DEGC      : i16,
    pub Temp_Max_Sensor_INDEX   : u8,
    pub Temp_Min_Sensor_INDEX   : u8,
    pub Mosfet_Temp_DEGC        : i16,
    pub Ambient_Temp_DEGC       : i16,


    // --- State Estimations & Capacity ---
    pub Pack_Soc_PERCENT        : u16, // Often scaled (e.g., 0 to 1000 for 0.1% resolution)
    pub Pack_Soh_PERCENT        : u16,

    pub Pack_Sop_WATTS          : u32, // State of Power (available peak power)

    pub Bms_Rem_Cap_MAH         : u32, // Remaining Capacity
    pub Bms_Fcc_MAH             : u32, // Full Charge Capacity

    pub Bms_Design_Cap_MAH      : u32, // Original factory capacity
    pub Bms_Cycle_Count_COUNT   : u16,
    pub Time_To_Empty_SECONDS   : u32,
    pub Time_To_Full_SECONDS    : u32,


    // --- Dynamic Operating Limits (Sent to Charger/Inverter/VCU) ---
    pub Charge_Current_Limit_AMPS       : f32,
    pub Discharge_Current_Limit_AMPS    : f32,

    pub Charge_Voltage_Limit_VOLTS      : f32,
    pub Discharge_Voltage_Limit_VOLTS   : f32,


    // --- Hardware & Contactor Status ---
    pub Charge_Fet_Closed_BOOL      : bool,
    pub Discharge_Fet_Closed_BOOL   : bool,
    pub Precharge_Fet_Closed_BOOL   : bool,
    pub Heater_Active_BOOL          : bool,
    pub Charger_Connected_BOOL      : bool,


    // --- Protection & Fault Flags ---
    pub Fault_Over_Voltage_BOOL     : bool,
    pub Fault_Under_Voltage_BOOL    : bool,

    pub Fault_Charge_Over_Current_BOOL      : bool,
    pub Fault_Discharge_Over_Current_BOOL   : bool,

    pub Fault_Short_Circuit_BOOL                : bool,
    pub Fault_Over_Temperature_Charge_BOOL      : bool,
    pub Fault_Under_Temperature_Charge_BOOL     : bool,
    pub Fault_Over_Temperature_Discharge_BOOL   : bool,
    pub Fault_Under_Temperature_Discharge_BOOL  : bool,
    pub Fault_Cell_Imbalance_BOOL               : bool,
    pub Fault_Insulation_Low_BOOL               : bool,
    pub Fault_Internal_Comm_BOOL                : bool,
}
