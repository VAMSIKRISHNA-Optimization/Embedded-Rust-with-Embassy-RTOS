#![allow(non_snake_case)]

// ============================================================================
// Imports
// ============================================================================
use core::fmt::Write;
use embedded_graphics::
{
    mono_font::
    {
        ascii::{FONT_10X20, FONT_6X10, FONT_7X13_BOLD},
        MonoTextStyle, MonoTextStyleBuilder,
    },
    pixelcolor::{Rgb565, WebColors},
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};
use heapless::String;

use crate::Integrations::ExtRTC_integration::ExtRTC_SharedData::{ExtRTC_DateTime, ExtRTC_State};

// ============================================================================
// Dashboard Color Palette
// ============================================================================
const OLED_BG_COLOR:     Rgb565 = Rgb565::BLACK;
const OLED_HEADER_BG:    Rgb565 = Rgb565::new(0, 10, 22);   // Deep navy
const OLED_CARD_BG:      Rgb565 = Rgb565::new(3, 7, 5);     // Dark slate
const OLED_BORDER_COLOR: Rgb565 = Rgb565::new(10, 22, 14);  // Muted steel
const OLED_LABEL_COLOR:  Rgb565 = Rgb565::CSS_LIGHT_GRAY;

// ============================================================================
// Dashboard Data Snapshot (Decoupled from SMBus internals)
// ============================================================================
#[derive(Clone, Copy, PartialEq)]
pub struct OLED_TelemetryView
{
    pub pack_voltage_mv:  u32,
    pub current_ma:       i32,
    pub rsoc_pct:         u8,
    pub soh_pct:          u8,
    pub cycle_count:      u16,
    pub min_cell_mv:      u16,
    pub max_cell_mv:      u16,
    pub cell_max_temp_c:  f32,
    pub fet_temp_c:       f32,
    pub chg_fet_on:       bool,
    pub dsg_fet_on:       bool,
    pub has_fault:        bool,
    pub safety_status:    u32,
    pub pf_status:        u32,
}

// ============================================================================
// OLED Dashboard Renderer (320 x 240 Landscape, Partial Updates)
// ============================================================================
pub struct OLED_Dashboard<D>
{
    display:   D,
    last_view: Option<OLED_TelemetryView>,
    last_rtc:  Option<ExtRTC_DateTime>,
    last_sync: Option<bool>,
}

impl<D, E> OLED_Dashboard<D>
where
    D: DrawTarget<Color = Rgb565, Error = E>,
{
    pub fn new(display: D) -> Self
    {
        Self
        {
            display,
            last_view: None,
            last_rtc:  None,
            last_sync: None,
        }
    }

    // ------------------------------------------------------------------------
    // 1. Static Layout (Called ONCE at startup)
    // ------------------------------------------------------------------------
    pub fn draw_static_layout(&mut self) -> Result<(), E>
    {
        self.display.clear(OLED_BG_COLOR)?;

        let card_style = PrimitiveStyle::with_fill(OLED_CARD_BG);
        let border_style = PrimitiveStyle::with_stroke(OLED_BORDER_COLOR, 1);
        let title_style = MonoTextStyle::new(&FONT_7X13_BOLD, Rgb565::CYAN);
        let label_style = MonoTextStyle::new(&FONT_6X10, OLED_LABEL_COLOR);

        // --- Header Bar (320 x 26) ---
        Rectangle::new(Point::new(0, 0), Size::new(320, 26))
            .into_styled(PrimitiveStyle::with_fill(OLED_HEADER_BG))
            .draw(&mut self.display)?;

        Text::new("BE BATTERY BMS", Point::new(8, 17), title_style)
            .draw(&mut self.display)?;

        // --- Row 1 Cards: Pack Voltage (Left) | Current (Right) ---
        self.draw_card(6, 32, 151, 54, "PACK VOLTAGE", card_style, border_style, label_style)?;
        self.draw_card(163, 32, 151, 54, "PACK CURRENT", card_style, border_style, label_style)?;

        // --- Row 2 Cards: RSOC (Left) | SOH & Cycles (Right) ---
        self.draw_card(6, 92, 151, 50, "STATE OF CHARGE", card_style, border_style, label_style)?;
        self.draw_card(163, 92, 151, 50, "HEALTH / CYCLES", card_style, border_style, label_style)?;

        // --- Row 3 Cards: Cell Min/Max/Delta (Left) | Temps (Right) ---
        self.draw_card(6, 148, 151, 52, "CELLS (MIN/MAX/dV)", card_style, border_style, label_style)?;
        self.draw_card(163, 148, 151, 52, "TEMPERATURES", card_style, border_style, label_style)?;

        // --- Footer Bar: FETs & Fault Status (320 x 32) ---
        Rectangle::new(Point::new(6, 206), Size::new(308, 28))
            .into_styled(card_style)
            .draw(&mut self.display)?;
        Rectangle::new(Point::new(6, 206), Size::new(308, 28))
            .into_styled(border_style)
            .draw(&mut self.display)?;

        Ok(())
    }

    fn draw_card
    (
        &mut self,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        label: &str,
        fill: PrimitiveStyle<Rgb565>,
        border: PrimitiveStyle<Rgb565>,
        label_style: MonoTextStyle<'_, Rgb565>,
    ) -> Result<(), E>
    {
        Rectangle::new(Point::new(x, y), Size::new(w, h))
            .into_styled(fill)
            .draw(&mut self.display)?;

        Rectangle::new(Point::new(x, y), Size::new(w, h))
            .into_styled(border)
            .draw(&mut self.display)?;

        Text::new(label, Point::new(x + 8, y + 13), label_style)
            .draw(&mut self.display)?;

        Ok(())
    }

    // ------------------------------------------------------------------------
    // 2. Partial Dynamic Update (Called every 1s tick)
    // ------------------------------------------------------------------------
    pub fn update_dashboard
    (
        &mut self,
        view: &OLED_TelemetryView,
        rtc:  &ExtRTC_State,
    ) -> Result<(), E>
    {
        let mut buf: String<48> = String::new();

        // Styles with background_color overwrite old characters in-place (zero flicker)
        let big_green = MonoTextStyleBuilder::new()
            .font(&FONT_10X20)
            .text_color(Rgb565::GREEN)
            .background_color(OLED_CARD_BG)
            .build();

        let big_white = MonoTextStyleBuilder::new()
            .font(&FONT_10X20)
            .text_color(Rgb565::WHITE)
            .background_color(OLED_CARD_BG)
            .build();

        let med_white = MonoTextStyleBuilder::new()
            .font(&FONT_7X13_BOLD)
            .text_color(Rgb565::WHITE)
            .background_color(OLED_CARD_BG)
            .build();

        let rtc_color = if rtc.is_synced { Rgb565::GREEN } else { Rgb565::YELLOW };
        let rtc_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(rtc_color)
            .background_color(OLED_HEADER_BG)
            .build();

        // --- A. Header RTC Timestamp (Right-aligned) ---
        if self.last_rtc != Some(rtc.datetime) || self.last_sync != Some(rtc.is_synced)
        {
            buf.clear();
            let tag = if rtc.is_synced { "RTC" } else { "UP " };
            let _ = write!
            (
                buf,
                "{} {:02}:{:02}:{:02}",
                tag,
                rtc.datetime.hour,
                rtc.datetime.minute,
                rtc.datetime.second
            );
            Text::new(&buf, Point::new(236, 16), rtc_style).draw(&mut self.display)?;
            self.last_rtc  = Some(rtc.datetime);
            self.last_sync = Some(rtc.is_synced);
        }

        let prev = self.last_view;

        // --- B. Pack Voltage ---
        if prev.map_or(true, |p| p.pack_voltage_mv != view.pack_voltage_mv)
        {
            buf.clear();
            let _ = write!
            (
                buf,
                "{:2}.{:03} V  ",
                view.pack_voltage_mv / 1000,
                view.pack_voltage_mv % 1000
            );
            Text::new(&buf, Point::new(14, 74), big_green).draw(&mut self.display)?;
        }

        // --- C. Pack Current ---
        if prev.map_or(true, |p| p.current_ma != view.current_ma)
        {
            buf.clear();
            let _ = write!(buf, "{:6} mA ", view.current_ma);
            Text::new(&buf, Point::new(171, 74), big_white).draw(&mut self.display)?;
        }

        // --- D. RSOC ---
        if prev.map_or(true, |p| p.rsoc_pct != view.rsoc_pct)
        {
            buf.clear();
            let _ = write!(buf, "{:3} %     ", view.rsoc_pct);
            Text::new(&buf, Point::new(14, 132), big_green).draw(&mut self.display)?;
        }

        // --- E. SOH & Cycle Count ---
        if prev.map_or(true, |p| p.soh_pct != view.soh_pct || p.cycle_count != view.cycle_count)
        {
            buf.clear();
            let _ = write!(buf, "SOH:{:3}%  Cyc:{:<4}", view.soh_pct, view.cycle_count);
            Text::new(&buf, Point::new(171, 130), med_white).draw(&mut self.display)?;
        }

        // --- F. Cell Min / Max / Delta ---
        if prev.map_or(true, |p| p.min_cell_mv != view.min_cell_mv || p.max_cell_mv != view.max_cell_mv)
        {
            let delta = view.max_cell_mv.saturating_sub(view.min_cell_mv);
            buf.clear();
            let _ = write!(buf, "{}..{} mV  ", view.min_cell_mv, view.max_cell_mv);
            Text::new(&buf, Point::new(14, 176), med_white).draw(&mut self.display)?;

            buf.clear();
            let _ = write!(buf, "Delta: {:3} mV   ", delta);
            Text::new(&buf, Point::new(14, 193), med_white).draw(&mut self.display)?;
        }

        // --- G. Temperatures ---
        if prev.map_or(true, |p| p.cell_max_temp_c != view.cell_max_temp_c || p.fet_temp_c != view.fet_temp_c)
        {
            buf.clear();
            let _ = write!(buf, "Cell: {:5.1} C   ", view.cell_max_temp_c);
            Text::new(&buf, Point::new(171, 176), med_white).draw(&mut self.display)?;

            buf.clear();
            let _ = write!(buf, "FET : {:5.1} C   ", view.fet_temp_c);
            Text::new(&buf, Point::new(171, 193), med_white).draw(&mut self.display)?;
        }

        // --- H. Footer Status & FETs ---
        if prev.map_or(true, |p| {
            p.has_fault      != view.has_fault
            || p.chg_fet_on  != view.chg_fet_on
            || p.dsg_fet_on  != view.dsg_fet_on
            || p.safety_status != view.safety_status
        })
        {
            let status_color = if view.has_fault { Rgb565::RED } else { Rgb565::GREEN };
            let status_style = MonoTextStyleBuilder::new()
                .font(&FONT_7X13_BOLD)
                .text_color(status_color)
                .background_color(OLED_CARD_BG)
                .build();

            buf.clear();
            if view.has_fault
            {
                let _ = write!
                (
                    buf,
                    "FAULT! Saf:{:#06x} PF:{:#06x}   ",
                    view.safety_status,
                    view.pf_status
                );
            }
            else
            {
                let _ = write!
                (
                    buf,
                    "STATUS: OK  |  CHG: {}   DSG: {}      ",
                    if view.chg_fet_on { "ON " } else { "OFF" },
                    if view.dsg_fet_on { "ON " } else { "OFF" }
                );
            }
            Text::new(&buf, Point::new(14, 224), status_style).draw(&mut self.display)?;
        }

        self.last_view = Some(*view);
        Ok(())
    }
}