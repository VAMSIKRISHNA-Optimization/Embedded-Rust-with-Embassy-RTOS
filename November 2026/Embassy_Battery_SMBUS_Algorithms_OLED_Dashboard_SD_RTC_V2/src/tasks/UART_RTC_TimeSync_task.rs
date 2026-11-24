#![allow(non_snake_case)]

// ============================================================================
// Imports
// ============================================================================
use defmt::{info, warn};
use embassy_stm32::peripherals::USART2;
use embassy_stm32::usart::BufferedUartRx;
use embedded_io_async::Read;
use heapless::String;


use crate::Integrations::ExtRTC_integration::ExtRTC_SharedData::
{
    ExtRTC_Command,
    ExtRTC_DateTime,
    EXT_RTC_CMD_CHANNEL,
};

// ============================================================================
// UART RTC Time Sync Task
// ============================================================================
#[embassy_executor::task]
pub async fn uart_rtc_sync_task(mut uart_rx: BufferedUartRx<'static, USART2>)
{
    info!("UART_RTC: Listening on USART2 (115200) for 'SET YYYY-MM-DD HH:MM:SS'");

    let mut rx_byte = [0u8; 1];
    let mut line: String<64> = String::new();

    loop
    {
        match uart_rx.read(&mut rx_byte).await
        {
            Ok(1) =>
            {
                let ch = rx_byte[0];

                if ch == b'\n' || ch == b'\r'
                {
                    if !line.is_empty()
                    {
                        if let Some(dt) = parse_set_command(line.as_str())
                        {
                            info!
                            (
                                "UART_RTC: Parsed {}-{:02}-{:02} {:02}:{:02}:{:02}",
                                dt.year, dt.month, dt.day, dt.hour, dt.minute, dt.second
                            );
                            EXT_RTC_CMD_CHANNEL.send(ExtRTC_Command::SetTime(dt)).await;
                        }
                        else
                        {
                            warn!("UART_RTC: Invalid format. Expected: SET YYYY-MM-DD HH:MM:SS");
                        }
                        line.clear();
                    }
                }
                else if ch.is_ascii()
                {
                    if line.push(ch as char).is_err()
                    {
                        // Buffer overflow: reset line
                        line.clear();
                    }
                }
            }
            Ok(_) => {}
            Err(_) =>
            {
                line.clear();
            }
        }
    }
}

// ============================================================================
// Command Parser ("SET YYYY-MM-DD HH:MM:SS")
// ============================================================================
fn parse_set_command(input: &str) -> Option<ExtRTC_DateTime>
{
    let s = input.trim();
    if !s.starts_with("SET ") || s.len() < 23
    {
        return None;
    }

    let b = s.as_bytes();
    if b[8] != b'-' || b[11] != b'-' || b[14] != b' ' || b[17] != b':' || b[20] != b':'
    {
        return None;
    }

    let year   = s.get(4..8)?.parse::<u16>().ok()?;
    let month  = s.get(9..11)?.parse::<u8>().ok()?;
    let day    = s.get(12..14)?.parse::<u8>().ok()?;
    let hour   = s.get(15..17)?.parse::<u8>().ok()?;
    let minute = s.get(18..20)?.parse::<u8>().ok()?;
    let second = s.get(21..23)?.parse::<u8>().ok()?;

    if year < 2025 || month < 1 || month > 12 || day < 1 || day > 31
        || hour > 23 || minute > 59 || second > 59
    {
        return None;
    }

    Some(ExtRTC_DateTime
    {
        year,
        month,
        day,
        hour,
        minute,
        second,
    })
}