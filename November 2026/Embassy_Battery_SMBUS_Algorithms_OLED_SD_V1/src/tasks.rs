#![allow(non_snake_case)]

use defmt::info;
use embassy_stm32::gpio::Output;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::spi::Spi;
use embassy_stm32::peripherals::{SPI1, SPI2, PB0, PB6, PB12, PC5, PC7, PA9};
use embassy_stm32::dma::NoDma;

use core::cell::RefCell;
use core::f32::consts::E;

use embassy_embedded_hal::shared_bus::blocking::spi::SpiDeviceWithConfig;
use embassy_sync::blocking_mutex::raw::{NoopRawMutex, CriticalSectionRawMutex};
use embassy_sync::channel::Channel;

// Alias core formatting trait for display text formatting
use core::fmt::Write as FmtWrite;

use embassy_time::{block_for, Duration, Timer, Delay, Ticker};

use embedded_graphics::
{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Rectangle, PrimitiveStyleBuilder},
    text::Text,
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
};

use display_interface_spi::SPIInterface;
use mipidsi::Builder;
use mipidsi::models::ILI9341Rgb565;
use embassy_time::{};

use crate::{TOUCH_SIGNAL};


// Type alias to keep the function signature clean
type DisplaySpiDevice = SpiDeviceWithConfig<'static, NoopRawMutex, Spi<'static, SPI1, NoDma, NoDma>, Output<'static, PB6>>;
type SdSpiDevice      = SpiDeviceWithConfig<'static, NoopRawMutex, Spi<'static, SPI1, NoDma, NoDma>, Output<'static, PB0>>;

// Global channel for touch events
pub static TOUCH_CHANNEL: Channel<CriticalSectionRawMutex, (u16, u16), 2> = Channel::new();

// Global channel for LOG events
static SD_CHANNEL: Channel<CriticalSectionRawMutex, heapless::String<256>, 2> = Channel::new();

// Declare the task modules
pub mod battery_tasks;
pub mod defmt_publish_task;
pub mod display_task;
pub mod sd_task;
// pub mod touch_task; // Commented out for initial SMBus verification


use embassy_sync::signal::Signal;
// use embassy_sync::watch::Watch;
use embassy_sync::mutex::Mutex;

use crate::BE_Battery::BE_Battery_Init_Integration::BE_BatteryTelemetry;

/// Number of downstream tasks subscribing to BE_BatteryTelemetry:
/// 1. defmt_publish_task
/// 2. touchscreen_display_task
/// 3. sd_task
pub const BE_BATTERY_TELEMETRY_SUBSCRIBERS: usize = 3;

/// Broadcast channel holding the latest BE_BatteryTelemetry snapshot.
pub static BE_BATTERY_LATEST_TELEMETRY: Mutex
<
    CriticalSectionRawMutex,
    Option<BE_BatteryTelemetry>,
> = Mutex::new(None);

/// Sequential execution baton signals (carrying the 1-second `tick_count`):
/// Order: SMBus -> defmt -> Display -> SD
pub static BE_BATTERY_SIG_SMBUS_DONE    : Signal<CriticalSectionRawMutex, u32> = Signal::new();
pub static BE_BATTERY_SIG_DEFMT_DONE    : Signal<CriticalSectionRawMutex, u32> = Signal::new();
pub static BE_BATTERY_SIG_DISPLAY_DONE  : Signal<CriticalSectionRawMutex, u32> = Signal::new();