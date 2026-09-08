use embassy_stm32::i2c::I2c;
use embassy_stm32::peripherals::I2C2;
use embassy_time::Timer;
use defmt::info;

const LCD_ADDR : u8 = 0x27; // Default I2C Address from Robu.in spec sheet
const BACKLIGHT: u8 = 0x08; // Bit 3 controls backlight
const ENABLE   : u8 = 0x04; // Bit 2 controls Enable pulse
const RS_DATA  : u8 = 0x01; // Bit 0 controls Register Select (0 = Command, 1 = Data)

/// Pulse the Enable pin to latch 4 bits of data into the LCD
async fn write_nibble(i2c: &mut I2c<'static, I2C2>, nibble: u8, mode: u8) 
{
    let byte = nibble | mode | BACKLIGHT;
    let _ = i2c.blocking_write(LCD_ADDR, &[byte | ENABLE]);
    Timer::after_micros(1).await;
    let _ = i2c.blocking_write(LCD_ADDR, &[byte & !ENABLE]);
    Timer::after_micros(50).await;
}

/// Send a full byte (Command or Data) in two 4-bit nibbles
async fn send_byte(i2c: &mut I2c<'static, I2C2>, val: u8, mode: u8) 
{
    let high_nibble = val & 0xF0;
    let low_nibble = (val << 4) & 0xF0;
    write_nibble(i2c, high_nibble, mode).await;
    write_nibble(i2c, low_nibble, mode).await;
}

/// Send a command byte to HD44780
async fn send_cmd(i2c: &mut I2c<'static, I2C2>, cmd: u8) 
{
    send_byte(i2c, cmd, 0).await;
}

/// Send a character byte to HD44780
async fn send_char(i2c: &mut I2c<'static, I2C2>, ch: u8) 
{
    send_byte(i2c, ch, RS_DATA).await;
}

/// Set cursor position (col: 0-15, row: 0-1)
async fn set_cursor(i2c: &mut I2c<'static, I2C2>, col: u8, row: u8) 
{
    let row_offsets = [0x00, 0x40];
    let addr = row_offsets[(row & 0x01) as usize] + col;
    send_cmd(i2c, 0x80 | addr).await;
}

/// Print a text string
async fn print_str(i2c: &mut I2c<'static, I2C2>, text: &str) 
{
    for b in text.bytes() 
    {
        send_char(i2c, b).await;
    }
}

/// HD44780 Initialization Sequence
async fn init_lcd(i2c: &mut I2c<'static, I2C2>) 
{
    Timer::after_millis(50).await; // Wait for LCD power-up

    // Step 1: Soft reset sequence to force 4-bit mode
    write_nibble(i2c, 0x30, 0).await;
    Timer::after_millis(5).await;
    write_nibble(i2c, 0x30, 0).await;
    Timer::after_micros(150).await;
    write_nibble(i2c, 0x30, 0).await;
    write_nibble(i2c, 0x20, 0).await; // Set to 4-bit interface
    Timer::after_millis(1).await;

    // Step 2: Function set & Display Config
    send_cmd(i2c, 0x28).await; // 4-bit mode, 2 lines, 5x8 font
    send_cmd(i2c, 0x0C).await; // Display ON, Cursor OFF, Blink OFF
    send_cmd(i2c, 0x01).await; // Clear display
    Timer::after_millis(2).await;
    send_cmd(i2c, 0x06).await; // Entry mode: increment cursor
}

#[embassy_executor::task]
pub async fn lcd_task(mut i2c: I2c<'static, I2C2>) 
{
    info!("Initializing LCD1602 on I2C2...");
    init_lcd(&mut i2c).await;
    info!("LCD Initialized successfully!");

    // Print static header on Line 1
    set_cursor(&mut i2c, 0, 0).await;
    print_str(&mut i2c, "Vamsi Krishna").await;

    let mut seconds = 0u32;

    loop 
    {
        // Update Line 2 with uptime counter
        set_cursor(&mut i2c, 0, 1).await;
        
        // Print message and live counter
        print_str(&mut i2c, "Embassy: ").await;
        
        // Simple manual integer to ASCII conversion for display
        let mut buf = [0u8; 10];
        let s = format_u32(seconds, &mut buf);
        print_str(&mut i2c, s).await;
        print_str(&mut i2c, "s   ").await; // spaces to clear leftover digits

        info!("LCD updated: Vamsi Krishna | Embassy: {}s", seconds);

        seconds += 1;
        Timer::after_secs(1).await;
    }
}

/// Helper function to format u32 into a string slice without external allocators
fn format_u32(mut val: u32, buf: &mut [u8]) -> &str 
{
    if val == 0 
    {
        buf[0] = b'0';
        return core::str::from_utf8(&buf[..1]).unwrap();
    }
    let mut i = buf.len();
    while val > 0 
    {
        i -= 1;
        buf[i] = b'0' + (val % 10) as u8;
        val /= 10;
    }
    core::str::from_utf8(&buf[i..]).unwrap()
}