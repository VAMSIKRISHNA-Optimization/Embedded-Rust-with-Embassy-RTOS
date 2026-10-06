use defmt::info;
use embassy_stm32::timer::simple_pwm::SimplePwm;
use embassy_stm32::timer::Channel;
use embassy_stm32::peripherals::TIM1;
use embassy_time::{Timer, Duration};
use embassy_stm32::time::Hertz;

// Common musical frequencies (Hz)
const NOTE_C4: u32 = 262;
const NOTE_E4: u32 = 330;
const NOTE_G4: u32 = 392;
const NOTE_C5: u32 = 523;

async fn play_tone(pwm: &mut SimplePwm<'static, TIM1>, freq_hz: u32, duration_ms: u64) 
{
    if freq_hz == 0 
    {
        pwm.set_duty(Channel::Ch1, 0);
    } else 
    {
        pwm.set_frequency(Hertz(freq_hz));
        let max_duty = pwm.get_max_duty();
        pwm.set_duty(Channel::Ch1, max_duty / 2); // 50% duty cycle for square wave
    }
    Timer::after(Duration::from_millis(duration_ms)).await;
}

async fn mute(pwm: &mut SimplePwm<'static, TIM1>, duration_ms: u64) 
{
    pwm.set_duty(Channel::Ch1, 0);
    Timer::after(Duration::from_millis(duration_ms)).await;
}

#[embassy_executor::task]
pub async fn speaker_task(mut pwm: SimplePwm<'static, TIM1>) 
{
    info!("Speaker Task Started!");

    // Enable PWM channel output
    pwm.enable(Channel::Ch1);

    // Play startup chime
    play_tone(&mut pwm, NOTE_C4, 150).await;
    play_tone(&mut pwm, NOTE_E4, 150).await;
    play_tone(&mut pwm, NOTE_G4, 150).await;
    play_tone(&mut pwm, NOTE_C5, 300).await;
    mute(&mut pwm, 500).await;

    loop 
    {
        // Play periodic 1 kHz alert beep
        info!("Playing 1 kHz beep...");
        play_tone(&mut pwm, 1000, 200).await;
        mute(&mut pwm, 200).await;
        play_tone(&mut pwm, 1000, 200).await;
        
        // Silence for 3 seconds before next beep
        mute(&mut pwm, 3000).await;
    }
}