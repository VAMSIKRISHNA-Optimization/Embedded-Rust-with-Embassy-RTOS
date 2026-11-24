#![allow(non_snake_case)]

// ============================================================================
// Imports
// ============================================================================
use embedded_hal::i2c::I2c;

use crate::Integrations::ExtRTC_integration::ExtRTC_SharedData::ExtRTC_DateTime;

// ============================================================================
// SD3078 Register Map & Constants
// ============================================================================
const SD3078_I2C_ADDR:    u8 = 0x32;
const REG_TIME_START:     u8 = 0x00; // Sec, Min, Hour, Wday, Day, Month, Year
const REG_CTR1:           u8 = 0x0F; // WRTC2 (bit 7), WRTC3 (bit 2)
const REG_CTR2:           u8 = 0x10; // WRTC1 (bit 7)

const WRTC1_ENABLE:       u8 = 0x80;
const WRTC23_ENABLE:      u8 = 0x84;
const HOUR_24H_FLAG:      u8 = 0x80;

// ============================================================================
// Driver Definition
// ============================================================================
pub struct ExtRTC_Sd3078<I2C>
{
    i2c: I2C,
}

impl<I2C, E> ExtRTC_Sd3078<I2C>
where
    I2C: I2c<Error = E>,
{
    pub fn new(i2c: I2C) -> Self
    {
        Self { i2c }
    }

    // ------------------------------------------------------------------------
    // Public API
    // ------------------------------------------------------------------------
    pub fn is_time_valid(dt: &ExtRTC_DateTime) -> bool
    {
        dt.year >= 2025
            && dt.month >= 1
            && dt.month <= 12
            && dt.day >= 1
            && dt.day <= 31
            && dt.hour < 24
            && dt.minute < 60
            && dt.second < 60
    }

    pub async fn read_datetime(&mut self) -> Result<ExtRTC_DateTime, E>
    {
        let mut buf = [0u8; 7];
        self.i2c.write_read(SD3078_I2C_ADDR, &[REG_TIME_START], &mut buf)?;

        Ok(ExtRTC_DateTime
        {
            second: bcd_to_bin(buf[0] & 0x7F),
            minute: bcd_to_bin(buf[1] & 0x7F),
            hour:   bcd_to_bin(buf[2] & 0x3F), // Mask 24h flag (bit 7)
            day:    bcd_to_bin(buf[4] & 0x3F),
            month:  bcd_to_bin(buf[5] & 0x1F),
            year:   2000u16 + (bcd_to_bin(buf[6]) as u16),
        })
    }

    pub async fn set_datetime(&mut self, dt: ExtRTC_DateTime) -> Result<(), E>
    {
        // 1. Unlock SD3078 write protection (CTR2 first, then CTR1)
        self.i2c.write(SD3078_I2C_ADDR, &[REG_CTR2, WRTC1_ENABLE])?;
        self.i2c.write(SD3078_I2C_ADDR, &[REG_CTR1, WRTC23_ENABLE])?;

        // 2. Write time registers in 24-hour BCD format
        let year_2d = (dt.year.saturating_sub(2000) % 100) as u8;
        let payload =
        [
            REG_TIME_START,
            bin_to_bcd(dt.second),
            bin_to_bcd(dt.minute),
            bin_to_bcd(dt.hour) | HOUR_24H_FLAG,
            0x00, // Weekday (unused)
            bin_to_bcd(dt.day),
            bin_to_bcd(dt.month),
            bin_to_bcd(year_2d),
        ];
        self.i2c.write(SD3078_I2C_ADDR, &payload)?;

        // 3. Re-lock write protection (CTR1 first, then CTR2)
        self.i2c.write(SD3078_I2C_ADDR, &[REG_CTR1, 0x00])?;
        self.i2c.write(SD3078_I2C_ADDR, &[REG_CTR2, 0x00])?;

        Ok(())
    }
}

// ============================================================================
// BCD Helpers
// ============================================================================
fn bcd_to_bin(val: u8) -> u8
{
    ((val >> 4) * 10) + (val & 0x0F)
}

fn bin_to_bcd(val: u8) -> u8
{
    ((val / 10) << 4) | (val % 10)
}