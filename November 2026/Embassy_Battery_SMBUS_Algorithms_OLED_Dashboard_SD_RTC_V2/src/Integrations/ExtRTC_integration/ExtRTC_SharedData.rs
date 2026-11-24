use core::cell::RefCell;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex as SharedResourceLock;
use embassy_sync::channel::Channel;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExtRTC_DateTime
{
    pub year:   u16,
    pub month:  u8,
    pub day:    u8,
    pub hour:   u8,
    pub minute: u8,
    pub second: u8,
}

impl ExtRTC_DateTime
{
    pub const fn dummy() -> Self
    {
        Self
        {
            year:   2000,
            month:  1,
            day:    1,
            hour:   0,
            minute: 0,
            second: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ExtRTC_State
{
    pub datetime:   ExtRTC_DateTime,
    pub is_synced:  bool,
    pub is_present: bool,
}

impl ExtRTC_State
{
    pub const fn default_unsynced() -> Self
    {
        Self
        {
            datetime:   ExtRTC_DateTime::dummy(),
            is_synced:  false,
            is_present: false,
        }
    }
}

pub enum ExtRTC_Command
{
    SetTime(ExtRTC_DateTime),
}

// Command queue from UART task -> ExtRTC task
pub static EXT_RTC_CMD_CHANNEL: Channel<CriticalSectionRawMutex, ExtRTC_Command, 4> = Channel::new();

// Latest timestamp readable by OLED display_task and sd_task
pub static EXT_RTC_STATE: SharedResourceLock<CriticalSectionRawMutex, RefCell<ExtRTC_State>> =
    SharedResourceLock::new(RefCell::new(ExtRTC_State::default_unsynced()));

pub fn ext_rtc_get_state() -> ExtRTC_State
{
    EXT_RTC_STATE.lock(|cell| *cell.borrow())
}

pub fn ext_rtc_set_state(state: ExtRTC_State)
{
    EXT_RTC_STATE.lock(|cell| *cell.borrow_mut() = state);
}