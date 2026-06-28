use core::convert::Infallible;

use defmt::info;
use cortex_m::{interrupt, peripheral::DWT};
use embassy_stm32::gpio::{Flex, Pull, Speed};
use embedded_hal::delay::DelayNs;

const CYCLES_PER_US: u32 = 170;
const SIGNAL_TIMEOUT_CYCLES: u32 = 1_000 * CYCLES_PER_US;
const BIT_ONE_THRESHOLD_CYCLES: u32 = 47 * CYCLES_PER_US;

#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum Am2302Error
{
    //O sensor nao respondeu no tempo esperado
    Timeout,

    //Os 40 bits foram lidos, mas o checksum não bateu 
    Checksum,

    //O protocolo não bateu com o esperado
    Protocol, 

    //Falha ao dirigir a linha para output ou ler o input
    Pin,
}

pub trait Am2302Io {
    type Error;

    // MCU assume a linha e força nivel baixo.
    fn drive_low(&mut self) -> Result<(), Self::Error>;

    // MCU libera a linha. O pull-up externo leva o barramento para high.
    fn release(&mut self) -> Result<(), Self::Error>;

    // MCU observa o nivel atual do barramento.
    fn is_high(&mut self) -> Result<bool, Self::Error>;
}

impl Am2302Io for Flex<'_> {
    type Error = Infallible;

    fn drive_low(&mut self) -> Result<(), Self::Error> {
        // Configura como saida push-pull e puxa para GND.
        self.set_as_output(Speed::Low);
        self.set_low();
        Ok(())
    }

    fn release(&mut self) -> Result<(), Self::Error> {
        // Configura como entrada: realmente solta a linha para o pull-up externo.
        self.set_as_input(Pull::None);
        Ok(())
    }

    fn is_high(&mut self) -> Result<bool, Self::Error> {
        // Le o nivel atual da linha.
        Ok(Flex::is_high(self))
    }
}

pub struct Am2302<IO, D>
{
    io: IO,
    delay: D,
}

impl<IO, D> Am2302<IO, D>
where 
    IO: Am2302Io,
    D: DelayNs,
{
    pub fn new(io: IO, delay: D) -> Self
    {
        Self {io, delay}
    }

    fn cycle_count() -> u32
    {
        DWT::cycle_count()
    }

    fn elapsed_cycles(start: u32) -> u32
    {
        Self::cycle_count().wrapping_sub(start)
    }

    fn wait_for_level(&mut self, expected_high: bool, timeout_cycles: u32) -> Result<(), Am2302Error>
    {
        let start = Self::cycle_count();

        loop
        {
            let is_high = self.io.is_high().map_err(|_| Am2302Error::Pin)?;
            if is_high == expected_high
            {
                return Ok(());
            }

            if Self::elapsed_cycles(start) >= timeout_cycles
            {
                return Err(Am2302Error::Timeout);
            }
        }
    }

    //Executa o star do protocolo e espera a resposta inicial do sensor
    fn begin_signal(&mut self) -> Result<(), Am2302Error>
    {
        //coloca a linha em nível baixo para iniciar o protocolo
        self.io.drive_low().map_err(|_| Am2302Error::Pin)?;

        // Mantém low por um tempo dentro da janela do datasheet
        self.delay.delay_ms(2);

        //Libera a linha antes de esperar a resposta do sensor
        self.io.release().map_err(|_| Am2302Error::Pin)?;

        //Libera a janela curta de liberação antes da resposta
        self.delay.delay_us(30);

        //Espera a linha cair:: resposta low do sensor
        self.wait_for_level(false, SIGNAL_TIMEOUT_CYCLES)?;

        //Espera linha subir: resposta high do sensor
        self.wait_for_level(true, SIGNAL_TIMEOUT_CYCLES)?;

        //Espera terminar a fase high de resposta para inicia os bits
        self.wait_for_level(false, SIGNAL_TIMEOUT_CYCLES)?;

        Ok(())
    }

    fn measure_high_pulse_cycles(&mut self, timeout_cycles: u32) -> Result<u32, Am2302Error>
    {
        let start = Self::cycle_count();

        loop
        {
            let is_high = self.io.is_high().map_err(|_| Am2302Error::Pin)?;
            if !is_high
            {
                return Ok(Self::elapsed_cycles(start));
            }

            if Self::elapsed_cycles(start) >= timeout_cycles
            {
                return Err(Am2302Error::Timeout);
            }
        }
    }

    //Le um unico bit do barramento do AM3202
    fn read_bit(&mut self) ->Result<u8, Am2302Error>
    {
        //Espera a fase low obrigatoria do bit.
        self.wait_for_level(true, SIGNAL_TIMEOUT_CYCLES)?;

        //Mede quanto tempo a linha permanece em high
        let high_time_cycles = self.measure_high_pulse_cycles(SIGNAL_TIMEOUT_CYCLES)?;

        // DHT22/AM2302 usa ~26us para '0' e ~70us para '1'.
        let bit = if high_time_cycles > BIT_ONE_THRESHOLD_CYCLES { 1 } else { 0 };

        Ok(bit)
    }

    //Le os 40 bits completos e devolve 5 bytes
    fn read_frame(&mut self) -> Result<[u8; 5], Am2302Error>
    {
        let mut bytes = [0u8; 5];

        for bit_index in 0..40
        {
            let bit = match self.read_bit() {
                Ok(result) => result,
                Err(e) => {
                    info!("[DBG] read_frame: ERRO no bit {} = {:?}", bit_index, e);
                    return Err(e);
                }
            };

            //Descobre em qual byte esse bit entra
            let byte_index = bit_index / 8;

            //Desloca o byte atual para a esquerda e injeta novo bit
            bytes[byte_index] = (bytes[byte_index] << 1) | bit;
        }

        // Loga os 5 bytes crus lidos do sensor.
        info!("[DBG] bytes crus: [{:#04x}, {:#04x}, {:#04x}, {:#04x}, {:#04x}]",
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4]);

        Ok(bytes)
    }

    //Valida o checksum do frame recebido 
    fn validate_checksum(bytes: &[u8; 5]) -> Result<(), Am2302Error>
    {
        //Recalcula o checksum esperado a partir dos 4 primeiros bytres 
        let expected = bytes[0]
                .wrapping_add(bytes[1])
                .wrapping_add(bytes[2])
                .wrapping_add(bytes[3]);
        
        //Se não bater, a leitura deve falhar 
        if expected != bytes[4]
        {
            return Err(Am2302Error::Checksum);
        }

        Ok(())
    }

    //Converte os bytes em temperatura umidade 
    fn decode_measurements(bytes: &[u8; 5]) -> Result<(f32, f32), Am2302Error> {
        // Converte a umidade em decimos para %RH.
        let raw_humidity = (u16::from(bytes[0]) << 8) | u16::from(bytes[1]);
        let humidity_rh = raw_humidity as f32 / 10.0;

        // Converte a temperatura, tratando o bit de sinal.
        let raw_temperature = (u16::from(bytes[2]) << 8) | u16::from(bytes[3]);
        let negative = (raw_temperature & 0x8000) != 0;
        let magnitude = raw_temperature & 0x7FFF;

        let mut temperature_c = magnitude as f32 / 10.0;
        if negative {
            temperature_c = -temperature_c;
        }

        Ok((temperature_c, humidity_rh))
    }

    pub fn read(&mut self) -> Result<(f32, f32), Am2302Error>
    {
        let bytes = interrupt::free(|_| -> Result<[u8; 5], Am2302Error> {
            // DHT22 e sensivel a jitter; bloquear IRQs evita perder pulsos curtos.
            self.begin_signal()?;
            self.read_frame()
        })?;

        //Garante que o frame nao veio corrompido
        Self::validate_checksum(&bytes)?;

        // Converte os bytes crus em temperatura e umidade.
        let result = Self::decode_measurements(&bytes)?;

        info!("[DBG] read final: temp={} C, humidity={} %RH", result.0, result.1);

        Ok(result)
    }
}
