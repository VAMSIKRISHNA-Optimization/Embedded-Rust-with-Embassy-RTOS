#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_stm32::gpio::OutputType;
use embassy_stm32::time::Hertz;
use embassy_stm32::timer::simple_pwm::{PwmPin, SimplePwm};

mod tasks;

#[embassy_executor::main]
async fn main(spawner: Spawner) 
{
    defmt::info!("=== Speaker Testing Setup ===");

    // Initialize embassy-stm32 peripheral access
    let p = embassy_stm32::init(Default::default());

    // Configure PA8 as Timer 1, Channel 1 PWM output pin
    let ch1 = PwmPin::new_ch1(p.PA8, OutputType::PushPull);

    // Initialize Timer 1 for SimplePwm with initial frequency 1000 Hz
    let pwm = SimplePwm::new
    (
        p.TIM1,
        Some(ch1),
        None,
        None,
        None,
        Hertz(1000),
        Default::default(),
    );

    // Spawn the audio speaker task
    spawner.spawn(tasks::speaker_task(pwm)).unwrap();

    loop 
    {
        embassy_time::Timer::after_secs(10).await;
    }
}