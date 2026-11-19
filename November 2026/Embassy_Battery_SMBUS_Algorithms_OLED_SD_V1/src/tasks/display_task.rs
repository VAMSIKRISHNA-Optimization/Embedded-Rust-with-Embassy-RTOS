#![allow(non_snake_case)]

use super::*;
use core::fmt::Write;
use defmt::{error, info};
use display_interface_spi::SPIInterface;
use embassy_time::Delay;
use embedded_graphics::
{
    mono_font::{ascii::FONT_9X15_BOLD, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    text::Text,
};
use heapless::String;
use mipidsi::{models::ILI9341Rgb565, options::Orientation, Builder};

use crate::tasks::{
    BE_BATTERY_LATEST_TELEMETRY, BE_BATTERY_SIG_DEFMT_DONE, BE_BATTERY_SIG_DISPLAY_DONE,
};

#[embassy_executor::task]
pub async fn touchscreen_display_task
(
    spi_device: DisplaySpiDevice, 
    dc_pin: Output<'static, PC7>,
    rst_pin: Output<'static, PA9>,
) 
{
    info!("BE_Battery: Starting Display Task");

    let di = SPIInterface::new(spi_device, dc_pin);
    let mut display = match Builder::new(ILI9341Rgb565, di)
        .reset_pin(rst_pin)
        .orientation(Orientation::new())
        .init(&mut Delay)
    {
        Ok(disp) => disp,
        Err(_) => 
        {
            error!("BE_Battery: Display init failed!");
            loop 
            {
                let tick = BE_BATTERY_SIG_DEFMT_DONE.wait().await;
                BE_BATTERY_SIG_DISPLAY_DONE.signal(tick);
            }
        }
    };

    let _ = display.clear(Rgb565::BLACK);

    let header_style = MonoTextStyle::new(&FONT_9X15_BOLD, Rgb565::CYAN);
    let text_style = MonoTextStyle::new(&FONT_9X15_BOLD, Rgb565::WHITE);
    let ok_style = MonoTextStyle::new(&FONT_9X15_BOLD, Rgb565::GREEN);
    let fault_style = MonoTextStyle::new(&FONT_9X15_BOLD, Rgb565::RED);

    let mut line: String<64> = String::new();

    loop 
    {
        let tick_count = BE_BATTERY_SIG_DEFMT_DONE.wait().await;

        let snapshot = 
        {
            let guard = BE_BATTERY_LATEST_TELEMETRY.lock().await;
            guard.clone()
        };

        if let Some(t) = snapshot 
        {
            // Compute min & max active cell voltages (ignoring unused 0 mV channels)
            let mut min_cell_mv = u16::MAX;
            let mut max_cell_mv = 0u16;
            for &v in &t.cell_voltages_mv 
            {
                if v > 0 
                {
                    if v < min_cell_mv 
                    {
                        min_cell_mv = v;
                    }
                    if v > max_cell_mv 
                    {
                        max_cell_mv = v;
                    }
                }
            }
            if min_cell_mv == u16::MAX 
            {
                min_cell_mv = 0;
            }

            let _ = display.clear(Rgb565::BLACK);

            // Line 1: Header + Tick
            line.clear();
            let _ = write!(line, "BE BATTERY  [{}s]", tick_count);
            let _ = Text::new(&line, Point::new(10, 22), header_style).draw(&mut display);

            // Line 2: Pack Voltage & Current
            line.clear();
            let _ = write!(line, "V: {} mV   I: {} mA", t.pack_voltage_mv, t.current_ma);
            let _ = Text::new(&line, Point::new(10, 52), text_style).draw(&mut display);

            // Line 3: RSOC, SOH & Cycles
            line.clear();
            let _ = write!
            (
                line,
                "RSOC: {}%  SOH: {}%  Cyc: {}",
                t.rsoc_pct, t.soh_pct, t.cycle_count
            );
            let _ = Text::new(&line, Point::new(10, 82), text_style).draw(&mut display);

            // Line 4: Cell Min / Max / Delta
            line.clear();
            let _ = write!
            (
                line,
                "Cell: {}..{} mV (d={}mV)",
                min_cell_mv,
                max_cell_mv,
                max_cell_mv.saturating_sub(min_cell_mv)
            );
            let _ = Text::new(&line, Point::new(10, 112), text_style).draw(&mut display);

            // Line 5: Temperatures
            line.clear();
            let _ = write!
            (
                line,
                "T_cell: {:.1}C  T_fet: {:.1}C",
                t.cell_max_temp_c, t.fet_temp_c
            );
            let _ = Text::new(&line, Point::new(10, 142), text_style).draw(&mut display);

            // Line 6: FETs & Fault Verdict
            line.clear();
            if t.safety_status.any_active() || t.pf_status.any_active() {
                let _ = write!
                (
                    line,
                    "FAULT! Saf:{:#x} PF:{:#x}",
                    t.safety_status.0, t.pf_status.0
                );
                let _ = Text::new(&line, Point::new(10, 175), fault_style).draw(&mut display);
            } else {
                let _ = write!
                (
                    line,
                    "STATUS: OK | CHG:{} DSG:{}",
                    t.operation_status.chg_fet_on() as u8,
                    t.operation_status.dsg_fet_on() as u8
                );
                let _ = Text::new(&line, Point::new(10, 175), ok_style).draw(&mut display);
            }
        }

        BE_BATTERY_SIG_DISPLAY_DONE.signal(tick_count);
    }
}