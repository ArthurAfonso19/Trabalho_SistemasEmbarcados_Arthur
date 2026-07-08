use defmt::Format;
use embassy_stm32::gpio::Pull;
use embassy_stm32::interrupt::typelevel::Binding;
use embassy_stm32::time::Hertz;
use embassy_stm32::timer::input_capture::{CapturePin, Ch2, InputCapture};
use embassy_stm32::timer::low_level::CountingMode;
use embassy_stm32::timer::{
    Channel,
    CaptureCompareInterruptHandler,
    GeneralInstance4Channel,
    TimerPin,
};
use embassy_stm32::Peri;
use embassy_time::{with_timeout, Duration};

// Erros iniciais da camada de medicao de pulso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum PulseMeasureError {
    // Nao houve inicio de eco.
    TimeoutWaitingForRise,
    // O eco comecou, mas nao terminou.
    TimeoutWaitingForFall,
    // Timestamps nao puderam ser interpretados.
    InvalidCapture,
}

// Implementacao generica 
pub struct TimPulseInput<'d, T, P>
where
    T: GeneralInstance4Channel<Word = u16>,
    P: TimerPin<T, Ch2>,
{
    // Pino ligado ao ECHO.
    pin: Peri<'d, P>,
    // Timer usado para medir o pulso HIGH.
    timer: Peri<'d, T>,
}

impl<'d, T, P> TimPulseInput<'d, T, P>
where
    T: GeneralInstance4Channel<Word = u16>,
    P: TimerPin<T, Ch2>,
{
    // Construtor minimo
    pub fn new(pin: Peri<'d, P>, timer: Peri<'d, T>) -> Self {
        Self { pin, timer }
    }

    //Reconfigura o pino como entrada de captura no canal 2
    fn make_capture<'a>(
        pin: &'a mut Peri<'d, P>,
        timer: &'a mut Peri<'d, T>,
        irq: impl  Binding<T::CaptureCompareInterrupt, CaptureCompareInterruptHandler<T>> + 'a,
    ) -> InputCapture<'a, T>
    {
        let ch2 = CapturePin::new(pin.reborrow(), Pull::None);

        InputCapture::new(
            timer.reborrow(),
            None,
            Some(ch2),
            None,
            None,
            irq,
            Hertz(1_000_000),
            CountingMode::EdgeAlignedUp,
        )
    }

    //Espera uma borda de subida no canal 2 e devolve o timestamp capturado 
    async fn wait_for_rise(
        capture: &mut InputCapture<'_, T>,
        timeout: Duration,
    ) -> Result<u16, PulseMeasureError>
    {
        with_timeout(timeout, capture.wait_for_rising_edge(Channel::Ch2))
            .await
            .map_err(|_| PulseMeasureError::TimeoutWaitingForRise)
    }

    //Espera a borda de descida no mesmo pulso HIGH e devolve o timestamp 
    async fn wait_for_fall(
        capture: &mut InputCapture<'_, T>,
        timeout: Duration,
    ) -> Result<u16, PulseMeasureError>
    {
        with_timeout(timeout, capture.wait_for_falling_edge(Channel::Ch2))
            .await
            .map_err(|_| PulseMeasureError::TimeoutWaitingForFall)
    }

    // Versao concreta desta fase: recebe explicitamente o binding de IRQ.
    pub async fn measure_high_pulse_with_irq<I>(
        &mut self,
        timeout: Duration,
        irq: I,
    ) -> Result<Duration, PulseMeasureError>
    where
        I: Binding<T::CaptureCompareInterrupt, CaptureCompareInterruptHandler<T>> + Copy + 'd,
    {
        let Self { pin, timer } = self;
        let mut capture = Self::make_capture(pin, timer, irq);

        let rise_tick = Self::wait_for_rise(&mut capture, timeout).await?;
        let fall_tick = Self::wait_for_fall(&mut capture, timeout).await?;

        let width_us = fall_tick.wrapping_sub(rise_tick);
        if width_us == 0 {
            return Err(PulseMeasureError::InvalidCapture);
        }

        Ok(Duration::from_micros(width_us as u64))
    }
}
