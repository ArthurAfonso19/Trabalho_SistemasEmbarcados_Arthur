## Fase 5 - Passo 6: Guia completo para reimplementar o `AM2302` com `timer input capture`

Este arquivo e um guia completo para reimplementar, do zero, a versao funcional atual do driver `AM2302` baseada em:

- `timer input capture`
- `DMA`
- captura de uma janela unica do frame
- decodificacao posterior por software a partir dos timestamps

O objetivo aqui nao e apenas mostrar a ideia geral.

O objetivo e mostrar exatamente:

- o que criar
- em qual arquivo
- o que alterar
- quais imports usar
- quais tipos escolher
- quais constantes usar
- como integrar no `main.rs`
- por que essa abordagem funcionou

Este guia foi reconstruido a partir da implementacao funcional atual do projeto.

---

## 1. Objetivo da abordagem

O `AM2302` envia um frame com temporizacao curta.

Cada bit tem, em termos praticos:

- um `LOW` fixo de cerca de `50 us`
- um `HIGH` de cerca de `26 us` para bit `0`
- um `HIGH` de cerca de `70 us` para bit `1`

O problema do driver antigo era que ele media isso por polling fino e bloqueio de interrupcoes.

O problema das primeiras tentativas com `EXTI` ou `await` em bordas sequenciais era outro:

- perda de bordas no handshake
- rearme tardio entre uma borda e outra
- latencia de software interferindo na parte critica

A implementacao que funcionou faz diferente:

1. usa `GPIO` apenas para o start do sensor
2. entrega ao timer a captura das bordas
3. usa `DMA` para copiar os timestamps de varias bordas sem rearme manual
4. decodifica o frame depois, olhando o buffer capturado

---

## 2. Arquivos que voce precisa criar ou alterar

### Criar

- `src/drivers/am2302_capture.rs`

### Alterar

- `src/drivers/mod.rs`
- `src/main.rs`

### Manter intacto

- `src/drivers/am2302.rs`

Esse arquivo antigo continua util como referencia historica, mas nao deve ser a base da nova solucao.

---

## 3. Hardware usado na implementacao funcional

Esta implementacao funcional foi validada com:

- pino do sensor: `PB6`
- timer: `TIM4`
- canal de captura: `TIM4_CH1`
- DMA: `DMA1_CH5`

Portanto, a integracao no firmware deve usar exatamente esse conjunto.

### Esquema eletrico

Para esta solucao, o esquema eletrico nao precisou ser mudado.

O barramento do sensor continuou no mesmo pino `PB6`.

---

## 4. Primeiro passo: exportar o novo modulo

Abra `src/drivers/mod.rs`.

Se ele estiver assim:

```rust
pub mod am2302;
```

deixe assim:

```rust
// Mantem o driver antigo disponivel como referencia historica.
pub mod am2302;
// Exporta o novo driver baseado em timer input capture.
pub mod am2302_capture;
```

Esse e o menor passo inicial para permitir a integracao do novo driver.

---

## 5. Criar o novo arquivo do driver

Crie `src/drivers/am2302_capture.rs`.

O conteudo funcional completo e este:

```rust
use defmt::{info, Format};
// API de DMA do embassy-stm32.
use embassy_stm32::dma;
// GPIO flexivel para alternar entre start em GPIO e captura por AF.
use embassy_stm32::gpio::{Flex, Pull, Speed};
// Trait usada para provar em compilacao que a IRQ correta foi ligada.
use embassy_stm32::interrupt::typelevel::Binding;
// Registrador bruto do timer para limpar o flag de captura antes de iniciar o DMA.
use embassy_stm32::pac::timer::TimGp16;
// Tipo de frequencia usado na configuracao do timer.
use embassy_stm32::time::Hertz;
// Driver de captura e marcadores de canal.
use embassy_stm32::timer::input_capture::{CapturePin, Ch1, InputCapture};
// Modo de contagem do timer.
use embassy_stm32::timer::low_level::CountingMode;
use embassy_stm32::timer::{
    // Handler da IRQ do timer e traits de compatibilidade do periferico.
    CaptureCompareInterruptHandler, Dma as TimerDma, GeneralInstance4Channel, TimerPin,
};
// Wrapper de posse exclusiva de perifericos do Embassy.
use embassy_stm32::Peri;
// Timeout para evitar travar a task em caso de falha do sensor.
use embassy_time::{with_timeout, Duration};
// Delay curto usado so no start do protocolo.
use embedded_hal::delay::DelayNs;

// Host segura a linha em LOW por 2 ms para iniciar o protocolo.
const START_LOW_MS: u32 = 2;
// Janela total de captura do frame inteiro.
const FRAME_TIMEOUT_US: u64 = 6_000;
// Limiar entre HIGH de bit 0 (~26 us) e HIGH de bit 1 (~70 us).
const BIT_ONE_THRESHOLD_US: u16 = 50;
// Faixa valida para os pulsos de resposta de ~80 us do sensor.
const RESPONSE_PULSE_MIN_US: u16 = 60;
const RESPONSE_PULSE_MAX_US: u16 = 100;
// Faixa valida para o LOW fixo de ~50 us que antecede cada bit.
const BIT_START_LOW_MIN_US: u16 = 35;
const BIT_START_LOW_MAX_US: u16 = 65;
// Numero de bordas capturadas na janela unica que fechou corretamente em hardware.
const RAW_EDGE_COUNT: usize = 83;
// Mantido igual ao total para simplificar a busca do alinhamento do frame.
const VALID_FRAME_EDGE_COUNT: usize = 83;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum Am2302CaptureError {
    // O frame nao chegou completo no tempo esperado.
    Timeout,
    // Os primeiros deltas capturados nao bateram com um inicio valido do protocolo.
    Protocol { dt01_us: u16, dt12_us: u16 },
    // O frame foi lido, mas o quinto byte nao bateu com o checksum calculado.
    Checksum,
}

// Driver generico sobre timer, pino e canal DMA.
pub struct Am2302Capture<'d, T, P, D>
where
    T: GeneralInstance4Channel,
    P: TimerPin<T, Ch1>,
    D: TimerDma<T, Ch1>,
{
    // Mesmo pino fisico do AM2302.
    pin: Peri<'d, P>,
    // Timer responsavel pelos timestamps das bordas.
    timer: Peri<'d, T>,
    // DMA que copia os valores do CCR para RAM.
    dma: Peri<'d, D>,
}

impl<'d, T, P, D> Am2302Capture<'d, T, P, D>
where
    T: GeneralInstance4Channel<Word = u16>,
    P: TimerPin<T, Ch1>,
    D: TimerDma<T, Ch1>,
{
    // Construtor simples do driver.
    pub fn new(pin: Peri<'d, P>, timer: Peri<'d, T>, dma: Peri<'d, D>) -> Self {
        Self { pin, timer, dma }
    }

    // Faz o start do protocolo em GPIO open-drain.
    fn begin_signal(pin: &mut Peri<'d, P>, delay: &mut impl DelayNs) {
        // Reempacota o pino como GPIO flexivel temporario.
        let mut line = Flex::new(pin.reborrow());

        // Open-drain: low dirige a linha; high apenas libera para o pull-up externo.
        line.set_as_input_output_pull(Speed::Low, Pull::None);
        // Garante barramento ocioso em high.
        line.set_high();
        delay.delay_us(5);

        // Pulso de start do host.
        line.set_low();
        delay.delay_ms(START_LOW_MS);

        // Solta a linha para o sensor responder.
        line.set_high();
    }

    // Reconfigura o pino como entrada de captura do timer.
    fn make_capture<'a>(
        pin: &'a mut Peri<'d, P>,
        timer: &'a mut Peri<'d, T>,
        irq: impl Binding<T::CaptureCompareInterrupt, CaptureCompareInterruptHandler<T>> + 'a,
    ) -> InputCapture<'a, T> {
        // Liga o pino ao canal 1 do timer em funcao alternativa.
        let ch1 = CapturePin::new(pin.reborrow(), Pull::None);

        InputCapture::new(
            timer.reborrow(),
            // Somente o canal 1 e usado, porque o sensor esta no TIM4_CH1.
            Some(ch1),
            None,
            None,
            None,
            irq,
            // 1 tick = 1 us.
            Hertz(1_000_000),
            CountingMode::EdgeAlignedUp,
        )
    }

    // Helper para comparar deltas de tempo com faixas validas.
    fn in_range(value: u16, min: u16, max: u16) -> bool {
        value >= min && value <= max
    }

    // Limpa CC1IF antes de armar uma nova captura para nao reaproveitar evento antigo.
    fn clear_ccif() {
        let regs = unsafe { TimGp16::from_ptr(T::regs()) };
        regs.sr().modify(|r| r.set_ccif(0, false));
    }

    // Captura o frame inteiro como uma janela unica de timestamps em RAM.
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

    // Decide se o HIGH observado representa bit 0 ou bit 1.
    fn classify_high_width(high_width_us: u16) -> u8 {
        if high_width_us > BIT_ONE_THRESHOLD_US { 1 } else { 0 }
    }

    // Procura dentro do buffer onde esta o primeiro rising edge do primeiro bit real.
    fn find_first_bit_rise(edges: &[u16; RAW_EDGE_COUNT]) -> Result<usize, Am2302CaptureError> {
        for start in 0..=(RAW_EDGE_COUNT - VALID_FRAME_EDGE_COUNT) {
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

        // Se nenhum caso bater, devolve os primeiros deltas para facilitar o debug.
        let dt01 = edges[1].wrapping_sub(edges[0]);
        let dt12 = edges[2].wrapping_sub(edges[1]);
        Err(Am2302CaptureError::Protocol { dt01_us: dt01, dt12_us: dt12 })
    }

    // Converte os timestamps capturados em 5 bytes do frame do sensor.
    fn decode_bytes(edges: &[u16; RAW_EDGE_COUNT], first_bit_rise: usize) -> [u8; 5] {
        let mut bytes = [0u8; 5];

        for bit_index in 0..40 {
            // Cada bit consome um par rise/fall correspondente ao HIGH daquele bit.
            let rise = edges[first_bit_rise + bit_index * 2];
            let fall = edges[first_bit_rise + bit_index * 2 + 1];
            let high_width_us = fall.wrapping_sub(rise);
            let bit = Self::classify_high_width(high_width_us);

            // Empacota os bits do sensor nos 5 bytes do frame.
            let byte_index = bit_index / 8;
            bytes[byte_index] = (bytes[byte_index] << 1) | bit;
        }

        bytes
    }

    // Valida o checksum do frame.
    fn validate_checksum(bytes: &[u8; 5]) -> Result<(), Am2302CaptureError> {
        let expected = bytes[0]
            .wrapping_add(bytes[1])
            .wrapping_add(bytes[2])
            .wrapping_add(bytes[3]);

        if expected != bytes[4] {
            return Err(Am2302CaptureError::Checksum);
        }

        Ok(())
    }

    // Converte os bytes crus para temperatura e umidade em unidades fisicas.
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

    // Fluxo completo da leitura do sensor.
    pub async fn read<I>(&mut self, irq: I) -> Result<(f32, f32), Am2302CaptureError>
    where
        I: Binding<T::CaptureCompareInterrupt, CaptureCompareInterruptHandler<T>>
            + Binding<D::Interrupt, dma::InterruptHandler<D>>
            + Copy
            + 'd,
    {
        // Separa os recursos do driver para coordenar pino, timer e DMA.
        let Self { pin, timer, dma } = self;

        // Delay local usado apenas na fase de start.
        let mut delay = Am2302StartDelay;

        // 1. Faz o start do protocolo em GPIO.
        Self::begin_signal(pin, &mut delay);

        // 2. Entrega o mesmo pino ao timer para captura do frame.
        let mut capture = Self::make_capture(pin, timer, irq);

        // 3. Captura todas as bordas relevantes numa unica janela DMA.
        let edges = Self::capture_edges(dma, &mut capture, irq).await?;

        // 4. Procura onde realmente comeca o primeiro HIGH do primeiro bit.
        let first_bit_rise = Self::find_first_bit_rise(&edges)?;

        // 5. Decodifica os 40 bits para 5 bytes.
        let bytes = Self::decode_bytes(&edges, first_bit_rise);

        // 6. Se o checksum falhar, loga contexto util de debug.
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

        // 7. Se tudo estiver correto, converte o frame para grandezas fisicas.
        Ok(Self::decode_measurements(&bytes))
    }
}

// Delay bloqueante curto usado so no start do protocolo.
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
```

---

## 6. Como esse driver funciona

### 6.1. `begin_signal()`

Essa funcao faz apenas o start do protocolo.

Ela usa `GPIO` em open-drain para:

- deixar a linha liberada
- puxar para `LOW` por `2 ms`
- soltar a linha novamente

Trecho:

```rust
// Start do host em barramento de um fio.
fn begin_signal(pin: &mut Peri<'d, P>, delay: &mut impl DelayNs) {
    // Reempacota o pino fisico como GPIO flexivel temporario.
    let mut line = Flex::new(pin.reborrow());

    // Open-drain evita que o microcontrolador force HIGH contra o sensor.
    line.set_as_input_output_pull(Speed::Low, Pull::None);
    // Garante idle high antes de comecar.
    line.set_high();
    delay.delay_us(5);

    // Pulso de start do host.
    line.set_low();
    delay.delay_ms(START_LOW_MS);

    // Solta a linha para o sensor responder.
    line.set_high();
}
```

### 6.2. `make_capture()`

Essa funcao reconfigura o mesmo pino como entrada de captura do timer.

O timer e configurado em `1 MHz`, ou seja:

```text
1 tick = 1 us
```

Trecho:

```rust
// Configura o timer para capturar timestamps no canal 1.
InputCapture::new(
    timer.reborrow(),
    // O sensor esta no canal 1.
    Some(ch1),
    None,
    None,
    None,
    irq,
    // Cada tick do timer corresponde a 1 us.
    Hertz(1_000_000),
    CountingMode::EdgeAlignedUp,
)
```

### 6.3. `capture_edges()`

Essa e a parte critica da implementacao.

Ela usa `DMA` para capturar uma janela unica de `83` bordas.

Trecho:

```rust
// Inicia uma captura unica do frame inteiro com DMA.
with_timeout(
    Duration::from_micros(FRAME_TIMEOUT_US),
    // O DMA copia automaticamente cada timestamp do canal para o buffer.
    capture.receive_waveform::<Ch1, D>(dma.reborrow(), irq, &mut edges),
)
.await
.map_err(|_| Am2302CaptureError::Timeout)?;
```

Isso foi decisivo porque elimina o problema de rearme entre varias bordas consecutivas.

### 6.4. `find_first_bit_rise()`

Esta funcao procura dentro do buffer um inicio valido do protocolo.

Ela aceita dois casos:

1. handshake completo ainda presente
2. buffer ja comecando em `HIGH` de resposta seguido do `LOW` do primeiro bit

Trecho principal:

```rust
// Caso em que o buffer ainda contem o handshake completo.
if Self::in_range(dt01, RESPONSE_PULSE_MIN_US, RESPONSE_PULSE_MAX_US)
    && Self::in_range(dt12, RESPONSE_PULSE_MIN_US, RESPONSE_PULSE_MAX_US)
    && Self::in_range(dt23, BIT_START_LOW_MIN_US, BIT_START_LOW_MAX_US)
{
    // O primeiro HIGH do primeiro bit esta 3 bordas depois.
    return Ok(start + 3);
}

// Caso em que o buffer ja comecou no HIGH de resposta.
if Self::in_range(dt01, RESPONSE_PULSE_MIN_US, RESPONSE_PULSE_MAX_US)
    && Self::in_range(dt12, BIT_START_LOW_MIN_US, BIT_START_LOW_MAX_US)
{
    // O primeiro HIGH do primeiro bit esta 2 bordas depois.
    return Ok(start + 2);
}
```

O valor retornado e o indice do `rising edge` do primeiro bit real.

### 6.5. `decode_bytes()`

Depois de achar o `rising edge` correto, a decodificacao fica direta.

Para cada bit:

1. pega `rise`
2. pega `fall`
3. calcula `fall - rise`
4. compara com `50 us`

Trecho:

```rust
// Cada bit e decidido pela largura do HIGH correspondente.
let rise = edges[first_bit_rise + bit_index * 2];
let fall = edges[first_bit_rise + bit_index * 2 + 1];
let high_width_us = fall.wrapping_sub(rise);
let bit = Self::classify_high_width(high_width_us);
```

---

## 7. Integracao no `main.rs`

Agora voce precisa integrar o driver novo no `main.rs`.

### 7.1. Importar o novo driver

No topo do arquivo, troque ou adicione:

```rust
// Importa o driver novo baseado em timer input capture + DMA.
use crate::drivers::am2302_capture::Am2302Capture;
```

### 7.2. Adicionar IRQ do timer e do DMA

No `bind_interrupts!`, deixe assim:

```rust
// Liga todas as IRQs ja usadas no projeto e adiciona as do AM2302.
bind_interrupts!(struct Irqs {
    LPUART1 => embassy_stm32::usart::InterruptHandler<peripherals::LPUART1>;
    DMA1_CHANNEL1 => embassy_stm32::dma::InterruptHandler<peripherals::DMA1_CH1>;
    DMA1_CHANNEL2 => embassy_stm32::dma::InterruptHandler<peripherals::DMA1_CH2>;

    I2C1_EV => embassy_stm32::i2c::EventInterruptHandler<peripherals::I2C1>;
    I2C1_ER => embassy_stm32::i2c::ErrorInterruptHandler<peripherals::I2C1>;
    DMA1_CHANNEL3 => embassy_stm32::dma::InterruptHandler<peripherals::DMA1_CH3>;
    DMA1_CHANNEL4 => embassy_stm32::dma::InterruptHandler<peripherals::DMA1_CH4>;
    // Canal DMA usado pelo TIM4_CH1 na implementacao validada.
    DMA1_CHANNEL5 => embassy_stm32::dma::InterruptHandler<peripherals::DMA1_CH5>;
    EXTI15_10 => exti::InterruptHandler<interrupt::typelevel::EXTI15_10>;
    // IRQ de capture/compare do TIM4.
    TIM4 => embassy_stm32::timer::CaptureCompareInterruptHandler<peripherals::TIM4>;
});
```

### 7.3. Criar a task do sensor

Substitua a task antiga do AM2302 pela seguinte:

```rust
#[embassy_executor::task]
async fn am2302_task(
    // Tipo concreto validado: TIM4 no PB6 com DMA1_CH5.
    mut sensor: Am2302Capture<'static, peripherals::TIM4, peripherals::PB6, peripherals::DMA1_CH5>,
) {
    loop {
        // Leitura assincrona do sensor usando timer + DMA.
        match sensor.read(Irqs).await {
            Ok((temperature_c, humidity_rh)) => {
                // Loga as medicoes no RTT.
                info!("AM2302: temp={} C, humidity={} %RH", temperature_c, humidity_rh);
            }

            Err(error) => {
                // Mantem os erros visiveis para diagnostico.
                error!("AM2302 error: {:?}", error);
            }
        }

        // Respeita o intervalo minimo entre leituras do AM2302.
        Timer::after_secs(2).await;
    }
}
```

### 7.4. Instanciar o driver no `main()`

Na `main()`, depois de `embassy_stm32::init(config)`, crie:

```rust
// Cria o driver com o trio validado em hardware: pino, timer e DMA.
let am2302 = Am2302Capture::new(p.PB6, p.TIM4, p.DMA1_CH5);
```

### 7.5. Fazer o `spawn` da task

Na parte de `spawn`, mantenha:

```rust
// Coloca a task do AM2302 no runtime concorrente do Embassy.
spawner.spawn(unwrap!(am2302_task(am2302)));
```

---

## 8. Como testar

### Compilacao

Use:

```powershell
cargo check
```

### Teste em hardware

Use:

```powershell
cargo run
```

### Resultado esperado

No RTT, o esperado e algo deste tipo:

```text
AM2302: temp=17.4 C, humidity=73.6 %RH
```

Foi exatamente esse padrao que ficou estavel na implementacao funcional atual.

---

## 9. Por que a implementacao funcional precisou desse formato

As tentativas anteriores falharam por motivos diferentes.

### Driver antigo

O driver antigo funcionava, mas tinha custo ruim para tempo real porque:

- usava polling fino
- usava `DWT`
- bloqueava interrupcoes durante a leitura critica
- segurava CPU e timing no software

### Tentativas com borda a borda

As primeiras tentativas com `input capture` sem DMA, ou com a captura dividida em blocos menores, falharam porque:

- havia rearme entre bordas
- a latencia de software ainda entrava na janela critica
- o inicio do frame podia cair entre uma captura e outra

### O que resolveu

O que resolveu foi:

- capturar o frame inteiro de uma vez
- usar `DMA` para reduzir a influencia da latencia do software
- alinhar o decoder depois, a partir do padrao temporal real encontrado no buffer

---

## 10. Por que essa alternativa e melhor do que o driver antigo

Esta alternativa e melhor que o driver antigo por motivos tecnicos claros.

### 10.1. Melhor para latencia

No driver antigo, o software precisava estar presente em cada microjanela do protocolo.

Na versao nova:

- o timer registra as bordas em hardware
- o DMA copia os timestamps automaticamente
- o software entra depois para interpretar os dados

Isso reduz a sensibilidade a jitter do executor.

### 10.2. Melhor para sincronismo

No driver antigo, o sincronismo dependia de loops de leitura e bloqueio de interrupcoes.

Na versao nova:

- o sincronismo fino e delegado ao timer
- a classificacao dos bits usa diferencas entre timestamps reais do periferico

Ou seja, a medicao fica muito mais proxima do hardware e muito menos dependente do tempo de execucao do codigo Rust em cada iteracao.

### 10.3. Melhor para tempo real

O driver antigo comprometia o restante do sistema porque travava a janela critica dentro da CPU.

Na versao nova:

- a CPU nao precisa fazer polling fino de cada transicao
- nao ha necessidade de bloquear interrupcoes globalmente
- a parte critica fica concentrada no timer e no DMA

Isso e mais coerente com um firmware embarcado concorrente.

### 10.4. Melhor escalabilidade de engenharia

Se no futuro voce quiser:

- instrumentar melhor a captura
- guardar timestamps para analise
- estudar o protocolo com mais profundidade

esta abordagem e muito melhor, porque o dado bruto relevante ja existe como buffer de timestamps.

No driver antigo, o comportamento critico ficava “preso” ao fluxo do polling em tempo real.

---

## 11. Resumo final

Se voce quiser reimplementar do zero depois para aprender, siga esta ordem:

1. exporte `am2302_capture` em `src/drivers/mod.rs`
2. crie `src/drivers/am2302_capture.rs` com o codigo completo acima
3. adicione `TIM4` e `DMA1_CH5` no `bind_interrupts!`
4. troque a task do sensor para `Am2302Capture<'static, peripherals::TIM4, peripherals::PB6, peripherals::DMA1_CH5>`
5. instancie `Am2302Capture::new(p.PB6, p.TIM4, p.DMA1_CH5)`
6. rode `cargo check`
7. rode `cargo run`

O principal aprendizado desta versao funcional e este:

> nao basta “usar input capture”.
> o que realmente tornou o driver robusto foi usar `input capture + DMA + captura unica do frame + alinhamento por software do padrao temporal`.
