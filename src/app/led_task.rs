use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::peripherals;
use embassy_stm32::Peri;
use embassy_time::Timer;

// Usa a mesma funcao de monitoramento ja existente no projeto.
use crate::mark_led_execution;

pub struct  LedControl 
{
    enabled: bool,
    period_ms: u32,
}

impl  LedControl 
{
    pub const fn new() -> Self
    {
        Self 
        { 
            enabled: true, 
            period_ms: 1000, 
        }
    }

    pub fn toggle(&mut self)
    {
        self.enabled = !self.enabled;
    }

    //Liga ou desliga o LED 
    pub fn set_enabled(&mut self, enabled:bool)
    {
        self.enabled = enabled;
    }

    pub fn set_period(&mut self, period_ms: u32)
    {
        self.period_ms = period_ms;
    }

    pub fn is_enabled(&self) -> bool
    {
        self.enabled
    }

    pub fn period_ms(&self) -> u32
    {
        self.period_ms
    }
}

#[embassy_executor::task]
pub async fn led_task(pa5: Peri<'static, peripherals::PA5>) {
    let mut led = Output::new(pa5, Level::High, Speed::Low);

    loop 
    {
        let (enabled, period_ms) = {
            let control = crate::LED_CONTROL.lock().await;
            (control.is_enabled(), control.period_ms())
        };    

        if enabled
        {
            led.set_high();
            mark_led_execution().await;
            Timer::after_millis((period_ms / 2) as u64).await;

            led.set_low();
            Timer::after_millis((period_ms / 2) as u64).await;
        }
        else 
        {
            led.set_low();
            Timer::after_millis(100).await;    
        }
    }
}
