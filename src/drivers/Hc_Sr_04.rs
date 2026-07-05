use crate::drivers::hcsr04::{PulseMeasureError, TimPulseInput};
use defmt::Format;

use embassy_stm32::interrupt::typelevel::Binding;
use embassy_stm32::timer::input_capture::Ch2;
use embassy_stm32::timer::{
    CaptureCompareInterruptHandler,
    GeneralInstance4Channel,
    TimerPin,
};
use embassy_time::Duration;
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::OutputPin;

// Timeout inicial para ausencia de eco.
const ECHO_TIMEOUT: Duration = Duration::from_millis(40);

// Erros de alto nivel do driver do HC-SR04.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum HcSr04Error<E> {
    // Falha no GPIO de trigger.
    TriggerPin,
    // Falha propagada da camada de medicao do echo.
    Echo(E),
    // Pulso invalido para a conversao atual.
    InvalidPulse,
}

// Driver secundario do sensor ultrassonico.
pub struct HcSr04<TRIG, ECHO> {
    // Saida digital para o TRIG.
    trig: TRIG,
    // Adaptador de medicao do ECHO.
    echo: ECHO,
}

impl<TRIG, ECHO> HcSr04<TRIG, ECHO>
where
    TRIG: OutputPin,
{
    // Construtor minimo do driver.
    pub fn new(trig: TRIG, echo: ECHO) -> Self {
        Self { trig, echo }
    }

    // Gera o pulso de trigger de 10 us.
    fn trigger_sensor(&mut self) -> Result<(), HcSr04Error<PulseMeasureError>> {
        let mut delay = HcSr04TriggerDelay;

        self.trig.set_low().map_err(|_| HcSr04Error::TriggerPin)?;
        delay.delay_us(2);

        self.trig.set_high().map_err(|_| HcSr04Error::TriggerPin)?;
        delay.delay_us(10);
        self.trig.set_low().map_err(|_| HcSr04Error::TriggerPin)?;

        Ok(())
    }

    // Converte largura de pulso em distancia aproximada.
    fn pulse_to_distance_cm(pulse: Duration) -> Result<f32, HcSr04Error<PulseMeasureError>> {
        let pulse_us = pulse.as_micros();

        if pulse_us == 0 {
            return Err(HcSr04Error::InvalidPulse);
        }

        Ok(pulse_us as f32 / 58.0)
    }
}

impl<'d, TRIG, T, P> HcSr04<TRIG, TimPulseInput<'d, T, P>>
where
    TRIG: OutputPin,
    T: GeneralInstance4Channel<Word = u16>,
    P: TimerPin<T, Ch2>,
{
    // Primeira versao concreta do driver usando o adaptador do passo 4.
    pub async fn measure_distance_cm_with_irq<I>(
        &mut self,
        irq: I,
    ) -> Result<f32, HcSr04Error<PulseMeasureError>>
    where
        I: Binding<T::CaptureCompareInterrupt, CaptureCompareInterruptHandler<T>> + Copy + 'd,
    {
        self.trigger_sensor()?;

        let pulse = self
            .echo
            .measure_high_pulse_with_irq(ECHO_TIMEOUT, irq)
            .await
            .map_err(HcSr04Error::Echo)?;

        Self::pulse_to_distance_cm(pulse)
    }
}

struct HcSr04TriggerDelay;

impl DelayNs for HcSr04TriggerDelay {
    fn delay_ns(&mut self, ns: u32) {
        // Converte nanossegundos em ciclos de CPU a 170 MHz.
        let cycles = (170_000_000u64 * ns as u64) / 1_000_000_000u64;
        cortex_m::asm::delay(cycles as u32);
    }

    fn delay_us(&mut self, us: u32) {
        // 170 ciclos por microssegundo com clock de 170 MHz.
        cortex_m::asm::delay(170 * us);
    }

    fn delay_ms(&mut self, ms: u32) {
        // Mantem a implementacao simples reaproveitando delay_us.
        for _ in 0..ms {
            self.delay_us(1000);
        }
    }
}