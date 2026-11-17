use embedded_hal::i2c::I2c;
use super::commands::{BE_Battery_MacCommand, BE_Battery_SbsCommand};

/// Default 7-bit SMBus slave address of the BQ78350-R1A (8-bit: 0x16 W / 0x17 R)
pub const BQ78350_I2C_ADDR_7BIT: u8 = 0x0B;

/// Two-step unlock key for 0x2B HostFETControl()
pub const HOST_FET_CONTROL_ACCESS_CODE: u16 = 0x1197;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BE_Battery_SmbusError<I2cErr> 
{
    I2c(I2cErr),
    BadBlockLength { expected_min: usize, actual: usize },
    MacCommandMismatch { expected: u16, actual: u16 },
    PecMismatch { expected: u8, actual: u8 },
}

/// Computes SMBus CRC-8 PEC (polynomial x^8 + x^2 + x + 1 = 0x07, init 0x00)
pub fn crc8_smbus(data: &[u8]) -> u8 
{
    let mut crc: u8 = 0x00;
    for &byte in data 
    {
        crc ^= byte;
        for _ in 0..8 
        {
            if (crc & 0x80) != 0 
            {
                crc = (crc << 1) ^ 0x07;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

pub struct Bq78350<I2C> 
{
    i2c: I2C,
    addr_7bit: u8,
    use_pec: bool,
}

impl<I2C, E> Bq78350<I2C>
where
    I2C: I2c<Error = E>,
{
    pub fn new(i2c: I2C, use_pec: bool) -> Self 
    {
        Self 
        {
            i2c,
            addr_7bit: BQ78350_I2C_ADDR_7BIT,
            use_pec,
        }
    }

    pub fn with_address(i2c: I2C, addr_7bit: u8, use_pec: bool) -> Self 
    {
        Self { i2c, addr_7bit, use_pec }
    }

    pub fn set_pec(&mut self, enabled: bool) 
    {
        self.use_pec = enabled;
    }

    /// SMBus Read Word (unsigned u16, Little-Endian)
    pub fn read_word(&mut self, cmd: BE_Battery_SbsCommand) -> Result<u16, BE_Battery_SmbusError<E>> 
    {
        let cmd_byte = cmd as u8;
        if self.use_pec 
        {
            let mut buf = [0u8; 3];
            self.i2c
                .write_read(self.addr_7bit, &[cmd_byte], &mut buf)
                .map_err(BE_Battery_SmbusError::I2c)?;
            let addr_w = self.addr_7bit << 1;
            let addr_r = addr_w | 1;
            let expected_pec = crc8_smbus(&[addr_w, cmd_byte, addr_r, buf[0], buf[1]]);
            if expected_pec != buf[2] 
            {
                return Err(BE_Battery_SmbusError::PecMismatch 
                    {
                        expected: expected_pec,
                        actual: buf[2],
                    });
            }
            Ok(u16::from_le_bytes([buf[0], buf[1]]))
        } else {
            let mut buf = [0u8; 2];
            self.i2c
                .write_read(self.addr_7bit, &[cmd_byte], &mut buf)
                .map_err(BE_Battery_SmbusError::I2c)?;
            Ok(u16::from_le_bytes(buf))
        }
    }

    /// SMBus Read Word (signed i16, Little-Endian — used for Current, AverageCurrent, DynamicPower)
    pub fn read_word_signed(&mut self, cmd: BE_Battery_SbsCommand) -> Result<i16, BE_Battery_SmbusError<E>> 
    {
        Ok(self.read_word(cmd)? as i16)
    }

    /// SMBus Write Word (Little-Endian)
    pub fn write_word(&mut self, cmd: BE_Battery_SbsCommand, value: u16) -> Result<(), BE_Battery_SmbusError<E>> 
    {
        let cmd_byte = cmd as u8;
        let [lsb, msb] = value.to_le_bytes();
        if self.use_pec 
        {
            let addr_w = self.addr_7bit << 1;
            let pec = crc8_smbus(&[addr_w, cmd_byte, lsb, msb]);
            self.i2c
                .write(self.addr_7bit, &[cmd_byte, lsb, msb, pec])
                .map_err(BE_Battery_SmbusError::I2c)
        } else {
            self.i2c
                .write(self.addr_7bit, &[cmd_byte, lsb, msb])
                .map_err(BE_Battery_SmbusError::I2c)
        }
    }

    /// SMBus Block Read: returns payload byte length written into `out_buf` (strips length byte & PEC)
    pub fn read_block(&mut self, cmd: BE_Battery_SbsCommand, out_buf: &mut [u8]) -> Result<usize, BE_Battery_SmbusError<E>> 
    {
        // Max SMBus block is 1 length byte + 32 payload bytes + 1 PEC byte = 34 bytes
        let mut raw = [0u8; 34];
        let cmd_byte = cmd as u8;
        self.i2c
            .write_read(self.addr_7bit, &[cmd_byte], &mut raw)
            .map_err(BE_Battery_SmbusError::I2c)?;

        let len = raw[0] as usize;
        if len > 32 || len > out_buf.len() 
        {
            return Err(BE_Battery_SmbusError::BadBlockLength 
                {
                    expected_min: len,
                    actual: out_buf.len(),
                });
        }

        if self.use_pec 
        {
            let addr_w = self.addr_7bit << 1;
            let addr_r = addr_w | 1;
            let mut pec_hdr = [0u8; 36];
            pec_hdr[0] = addr_w;
            pec_hdr[1] = cmd_byte;
            pec_hdr[2] = addr_r;
            pec_hdr[3..4 + len].copy_from_slice(&raw[0..1 + len]);
            let expected_pec = crc8_smbus(&pec_hdr[..4 + len]);
            let actual_pec = raw[1 + len];
            if expected_pec != actual_pec 
            {
                return Err(BE_Battery_SmbusError::PecMismatch 
                    {
                        expected: expected_pec,
                        actual: actual_pec,
                    });
            }
        }

        out_buf[..len].copy_from_slice(&raw[1..1 + len]);
        Ok(len)
    }

    /// Executes a write-only ManufacturerAccess command via 0x44 ManufacturerBlockAccess()
    pub fn mac_write(&mut self, mac: BE_Battery_MacCommand) -> Result<(), BE_Battery_SmbusError<E>> 
    {
        let [lsb, msb] = (mac as u16).to_le_bytes();
        let cmd_byte = BE_Battery_SbsCommand::ManufacturerBlockAccess as u8;
        if self.use_pec 
        {
            let addr_w = self.addr_7bit << 1;
            let pec = crc8_smbus(&[addr_w, cmd_byte, 0x02, lsb, msb]);
            self.i2c
                .write(self.addr_7bit, &[cmd_byte, 0x02, lsb, msb, pec])
                .map_err(BE_Battery_SmbusError::I2c)
        } else {
            self.i2c
                .write(self.addr_7bit, &[cmd_byte, 0x02, lsb, msb])
                .map_err(BE_Battery_SmbusError::I2c)
        }
    }

    /// Reads an extended register via 0x44 ManufacturerBlockAccess() (works in SEALED mode!).
    /// Automatically strips the 2-byte echoed MAC command prefix and copies data into `out_buf`.
    pub fn mac_read(&mut self, mac: BE_Battery_MacCommand, out_buf: &mut [u8]) -> Result<usize, BE_Battery_SmbusError<E>> 
    {
        self.mac_write(mac)?;

        let mut block = [0u8; 32];
        let total_len = self.read_block(BE_Battery_SbsCommand::ManufacturerBlockAccess, &mut block)?;
        if total_len < 2 
        {
            return Err(BE_Battery_SmbusError::BadBlockLength {
                expected_min: 2,
                actual: total_len,
            });
        }

        let echoed_cmd = u16::from_le_bytes([block[0], block[1]]);
        let expected_cmd = mac as u16;
        if echoed_cmd != expected_cmd 
        {
            return Err(BE_Battery_SmbusError::MacCommandMismatch 
                {
                    expected: expected_cmd,
                    actual: echoed_cmd,
                });
        }

        let payload_len = total_len - 2;
        if payload_len > out_buf.len() 
        {
            return Err(BE_Battery_SmbusError::BadBlockLength 
                {
                    expected_min: payload_len,
                    actual: out_buf.len(),
                });
        }
        out_buf[..payload_len].copy_from_slice(&block[2..total_len]);
        Ok(payload_len)
    }

    /// Two-step HostFETControl (0x2B) command (works in SEALED mode; must complete within 4s)
    pub fn set_host_fets(&mut self, pchg: bool, dsg: bool, chg: bool) -> Result<(), BE_Battery_SmbusError<E>> 
    {
        self.write_word(BE_Battery_SbsCommand::HostFetControl, HOST_FET_CONTROL_ACCESS_CODE)?;
        let mask = ((pchg as u16) << 2) | ((dsg as u16) << 1) | (chg as u16);
        self.write_word(BE_Battery_SbsCommand::HostFetControl, mask)
    }
}