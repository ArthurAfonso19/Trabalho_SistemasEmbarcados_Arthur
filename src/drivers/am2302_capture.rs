use defmt::{info, Format};
use embassy_stm32::dma;
use embassy_stm32::gpio::{Flex, Pull, Speed};
use embassy_stm32::interrupt::typelevel::Binding;
use embassy_stm32::pac::timer::TimGp16;
use embassy_stm32::time::Hertz;
use embassy_stm32::timer::input_capture::{CapturePin, Ch1, InputCapture};
use embassy_stm32::timer::low_level::CountingMode;
use embassy_stm32::timer::{
    CaptureCompareInterruptHandler, Dma as TimerDma, GeneralInstance4Channel, TimerPin,
};
use embassy_stm32::Peri;
use embassy_time::{with_timeout, Duration};
use embedded_hal::delay::DelayNs;

// Host segura a linha em LOW por 2 ms para iniciar o protocolo 
const START_LOW_MS: u32 = 2;
//Janela total de captura do frame inteiro 
const FRAME_TIMEOUT_US: u64 = 6_000;
//Limiar entre HIGH de bit 0 (~26 us) e HIGH de bit 1 (~70 us)
const BIT_ONE_THRESHOLD_US: u16 = 50; 
//Faixa valida para os pulsos de resposta de ~80 us do sensor
const RESPONSE_PULSE_MIN_US: u16 = 60;   
const RESPONSE_PULSE_MAX_US: u16 = 100;
//Faixa válida para o LOW fixo de ~50 us que antecede cada bit 
const BIT_START_LOW_MIN_US: u16 = 35;   
const BIT_START_LOW_MAX_US: u16 = 65;   
//Número de bordas capturadas na janela unica que fechou corretamente em hardware 
const RAW_EDGE_COUNT: usize = 83; 
//Mantido igual ao total para simplificar a busca do alinhamento do frame
const VALID_FRAME_EDGE_COUNT: usize = 83;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum Am2302CaptureError
{
    Timeout,
    Protocol{dt01_us: u16, dt12_us: u16},
    Checksum,
}

//Driver generico sobre timer, pino e canal DMA
pub struct Am2302Capture<'d, T, P, D>
where 
    T: GeneralInstance4Channel,
    P: TimerPin<T, Ch1>,
    D:TimerDma<T, Ch1>,
{
    pin: Peri<'d, P>,
    timer: Peri<'d, T>,
    dma: Peri<'d, D>,
}

impl<'d, T, P, D> Am2302Capture<'d, T, P, D>
where 
    T: GeneralInstance4Channel<Word = u16>, 
    P: TimerPin<T, Ch1>,
    D: TimerDma<T, Ch1>,
{
    //Construtor simple do driver 
    pub fn new(pin: Peri<'d, P>, timer: Peri<'d, T>, dma: Peri<'d,D>) -> Self
    {
        Self { pin, timer, dma}
    }

    //Faz o start do protocolo em GPIO open-drain 
    fn begin_signal(pin: &mut Peri<'d, P>, delay: &mut impl DelayNs)
    {
        let mut line = Flex::new(pin.reborrow());

        line.set_as_input_output_pull(Speed::Low, Pull::None);

        line.set_high();
        delay.delay_us(5);

        line.set_low();
        delay.delay_ms(START_LOW_MS);

        line.set_high();
    }

    //Reconfigura o pino como entrada de captura do timer 
    fn make_capture<'a>(
        pin: &'a mut Peri<'d, P>,
        timer: &'a mut Peri<'d, T>,
        irq: impl Binding<T::CaptureCompareInterrupt, CaptureCompareInterruptHandler<T>> + 'a,
    ) -> InputCapture<'a, T>
    {
        //Liga o pino ao canal 1 do timer em função alternativa 
        let ch1 = CapturePin::new(pin.reborrow(), Pull::None);

        InputCapture::new(
            timer.reborrow(),
            Some(ch1),
            None,
            None,
            None,
            irq,
            Hertz(1_000_000),
            CountingMode::EdgeAlignedUp,
        )
    }

    //Helpel para comparar deltas de tempo com faixas válidas 
    fn in_range(value: u16, min: u16, max: u16) -> bool
    {
        value >= min && value <= max
    }

    //Limpa CC1IF antes de armar uma nova captura para não reaproveitar evento antigo 
    fn clear_ccif() {
        let regs = unsafe { TimGp16::from_ptr(T::regs()) };
        regs.sr().modify(|r| r.set_ccif(0, false));
    }

    //Captura o frame inteiro como uma janela unica de timestamps em RAM
    async fn capture_edges(
        dma: &mut Peri<'d, D>,
        capture: &mut InputCapture<'_, T>,
        irq: impl Binding<D::Interrupt, dma::InterruptHandler<D>> + Copy,
    ) -> Result<[u16; RAW_EDGE_COUNT], Am2302CaptureError> {
        // Buffer que recebera os timestamps das bordas.
        let mut edges = [0u16; RAW_EDGE_COUNT];

        Self::clear_ccif();

        with_timeout(
            Duration::from_micros(FRAME_TIMEOUT_US),
            // O DMA copia cada novo valor capturado no canal 1 para o buffer edges.
            capture.receive_waveform::<Ch1, D>(dma.reborrow(), irq, &mut edges),
        )
        .await
        .map_err(|_| Am2302CaptureError::Timeout)?;

        // Se completou, o frame bruto esta inteiro em edges.
        Ok(edges)
    }

    //Decide se o HIGH observado representa bit 0 ou bit 1 
    fn classify_high_width(high_width_us: u16) -> u8
    {
        if high_width_us > BIT_ONE_THRESHOLD_US { 1 } else { 0 }
    }

    //Procura dentro do buffer onde esta o primeiro rising edge do primeiro bit real 
    fn find_first_bit_edge(edges: &[u16; RAW_EDGE_COUNT]) -> Result<usize, Am2302CaptureError>
    {
        for start in 0..=(RAW_EDGE_COUNT - VALID_FRAME_EDGE_COUNT) 
        {
            let dt01 = edges[start + 1].wrapping_sub(edges[start]);
            let dt12 = edges[start + 2].wrapping_sub(edges[start + 1]);
            let dt23 = edges[start + 3].wrapping_sub(edges[start + 2]);

            // Caso 1: o buffer ainda contem HIGH de resposta, LOW de resposta e LOW do primeiro bit.
            if Self::in_range(dt01, RESPONSE_PULSE_MIN_US, RESPONSE_PULSE_MAX_US)
                && Self::in_range(dt12, RESPONSE_PULSE_MIN_US, RESPONSE_PULSE_MAX_US)
                && Self::in_range(dt23, BIT_START_LOW_MIN_US, BIT_START_LOW_MAX_US)
            {
                // O primeiro HIGH de bit esta 3 bordas depois.
                return Ok(start + 3);
            }

            // Caso 2: o buffer ja comecou mais tarde, no HIGH de resposta seguido do LOW do primeiro bit.
            if Self::in_range(dt01, RESPONSE_PULSE_MIN_US, RESPONSE_PULSE_MAX_US)
                && Self::in_range(dt12, BIT_START_LOW_MIN_US, BIT_START_LOW_MAX_US)
            {
                // O primeiro HIGH de bit esta 2 bordas depois.
                return Ok(start + 2);
            }
        }

        //Se nenhum caso bater, devolve os primeiros deltas para facilitar debug 
        let dt01 = edges[1].wrapping_sub(edges[0]);
        let dt12 = edges[2].wrapping_sub(edges[1]);
        Err(Am2302CaptureError::Protocol { dt01_us: dt01, dt12_us: dt12 })
    }

    //Converter os timestamps capturados em 5 bytes do frame do sensor 
    fn decode_bytes(edges: &[u16; RAW_EDGE_COUNT], first_bit_rise: usize) -> [u8; 5]
    {
        let mut bytes = [0u8; 5];

        for bit_index in 0..40
        {
            let rise = edges[first_bit_rise + bit_index * 2];
            let fall = edges[first_bit_rise + bit_index * 2 + 1];
            let high_width_us = fall.wrapping_sub(rise);
            let bit = Self::classify_high_width(high_width_us);

            let byte_index = bit_index / 8;
            bytes[byte_index] = (bytes[byte_index] << 1) | bit;
        }

        bytes
    }

    //Valida o checksum do frame 
    fn validate_checksum(bytes: &[u8; 5]) -> Result<(), Am2302CaptureError>
    {
        let expected = bytes[0]
            .wrapping_add(bytes[1])
            .wrapping_add(bytes[2])
            .wrapping_add(bytes[3]);
        
        if expected != bytes[4]
        {
            return  Err(Am2302CaptureError::Checksum);
        }

        Ok(())
    }

    //Converte os bytes crus para temperatura e umidade em unidades físicas 
    fn decode_measurements(bytes: &[u8; 5]) -> (f32, f32) {
        // Umidade em decimos de %RH.
        let raw_humidity = (u16::from(bytes[0]) << 8) | u16::from(bytes[1]);
        let humidity_rh = raw_humidity as f32 / 10.0;

        // Temperatura com bit de sinal no bit 15.
        let raw_temperature = (u16::from(bytes[2]) << 8) | u16::from(bytes[3]);
        let negative = (raw_temperature & 0x8000) != 0;
        let magnitude = raw_temperature & 0x7FFF;

        let mut temperature_c = magnitude as f32 / 10.0;
        if negative {
            temperature_c = -temperature_c;
        }

        (temperature_c, humidity_rh)
    }

    //Fluxo completo da leitura do sensor 
    pub async  fn read<I>(&mut self, irq: I) -> Result<(f32,f32), Am2302CaptureError>
    where 
        I:Binding<T::CaptureCompareInterrupt, CaptureCompareInterruptHandler<T>>
            + Binding<D::Interrupt, dma::InterruptHandler<D>>
            + Copy
            + 'd,
    {
        let Self { pin, timer, dma} = self;

        let mut delay = Am2302StartDelay; 

        Self::begin_signal(pin, &mut delay);

        let mut capture = Self::make_capture(pin, timer, irq);

        let edges = Self::capture_edges(dma, &mut capture, irq).await?;

        let first_bit_rise = Self::find_first_bit_edge(&edges)?;

        let bytes = Self::decode_bytes(&edges, first_bit_rise);

        if let Err(error) = Self::validate_checksum(&bytes) {
            info!(
                "[DBG-CAP] first_bit_rise={}, bytes=[{:#04x}, {:#04x}, {:#04x}, {:#04x}, {:#04x}]",
                first_bit_rise,
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4]
            );
            info!(
                "[DBG-CAP] edges0=[{}, {}, {}, {}, {}, {}, {}, {}]",
                edges[0], edges[1], edges[2], edges[3], edges[4], edges[5], edges[6], edges[7]
            );
            return Err(error);
        }

        Ok(Self::decode_measurements(&bytes))
    }
}

struct Am2302StartDelay;

impl DelayNs for Am2302StartDelay {
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
        // Implementa milissegundos reaproveitando delay_us.
        for _ in 0..ms {
            self.delay_us(1000);
        }
    }
}
