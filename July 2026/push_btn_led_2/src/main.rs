#![no_std]
#![no_main]

use cortex_m_rt::entry;
use panic_halt as _;

// Bring in the Core Peripherals to access the SysTick timer
use cortex_m::peripheral::Peripherals as CorePeripherals;

use stm32f4xx_hal::pac;
use stm32f4xx_hal::prelude::*;
use stm32f4xx_hal::rcc::Config;

#[entry]
fn main() -> ! 
{
    let dp = pac::Peripherals::take().unwrap();
    let cp = CorePeripherals::take().unwrap();

    let mut rcc = dp.RCC.freeze(Config::default());
    let mut delay = cp.SYST.delay(&rcc.clocks);

    // 1. Push Button on PC13
    let gpioc = dp.GPIOC.split(&mut rcc);
    // Use a pull-up to ensure a stable High state when unpressed
    let btn = gpioc.pc13.into_pull_up_input();
    

    // 2. LED on PA5
    let gpioa = dp.GPIOA.split(&mut rcc);
    let mut led = gpioa.pa5.into_push_pull_output();

    loop 
    {
        if btn.is_low()
        {
            led.toggle();
            delay.delay_ms(100_u32);

            while btn.is_low() {}
            delay.delay_ms(100_u32);
        }

    }
}