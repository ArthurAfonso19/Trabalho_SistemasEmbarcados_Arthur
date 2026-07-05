## Fase 6 - Passo 4: guia de implementacao de `PulseInput` com `timer input capture`

Este documento cobre o passo 4 da Fase 6.

Objetivo do passo:

- sair da abstracao definida no passo 3 e implementar a captura real do `ECHO`
- usar o hardware do STM32G4 como caminho principal de medicao temporal
- fechar uma primeira implementacao funcional sobre `PC7 / TIM3_CH2`

Base ja fechada antes deste passo:

- `TRIG = PA6 = D12`
- `ECHO = PC7 = D9`
- `ECHO` protegido por divisor resistivo
- `ECHO` medido por `TIM3_CH2`
- `src/drivers/hcsr04.rs` ja contem `PulseInput`, `PulseMeasureError` e `TimPulseInput<'d, T, P>`

---

## 1. O que este passo deve entregar

Ao final deste passo, o projeto ainda nao precisa medir distancia em centimetros.

Ele precisa entregar a primeira implementacao concreta desta interface:

```rust
// Contrato definido no passo 3 e consumido depois pelo driver do sensor.
pub trait PulseInput {
    // Tipo de erro concreto da implementacao.
    type Error;

    // Mede a largura do pulso HIGH observado no ECHO.
    async fn measure_high_pulse(&mut self, timeout: Duration) -> Result<Duration, Self::Error>;
}
```

Traduzindo para a bancada atual:

- gerar uma captura no `TIM3_CH2`
- esperar uma borda de subida no `PC7`
- esperar a borda de descida correspondente
- devolver a largura desse pulso em `Duration`

---

## 2. O que muda do passo 3 para o passo 4

No passo 3, o foco era o contrato.

No passo 4, o foco passa a ser o adaptador concreto para STM32G4.

Entao a responsabilidade deste passo e:

- transformar `pin + timer` em uma entrada de captura valida
- configurar uma base temporal previsivel
- esperar eventos de subida e descida sem polling apertado
- converter ticks capturados em tempo util para o driver do `HC-SR04`

O driver final `HcSr04` ainda nao e o foco aqui.

---

## 3. Por que `timer input capture` e o caminho principal

O `HC-SR04` depende de uma largura de pulso relativamente simples, mas ainda temporalmente importante.

Usar `timer input capture` aqui traz as mesmas vantagens arquiteturais que apareceram no trabalho com o `AM2302`:

- a medicao fica centrada no hardware, nao em polling de CPU
- a resolucao fica facil de interpretar com timer em `1 MHz`
- o caminho fica desacoplado do driver final de distancia
- a task que usa o sensor continua mais previsivel

Neste passo, a recomendacao continua sendo:

- comecar **sem DMA**
- medir apenas um pulso `HIGH`
- deixar `DMA` como evolucao futura se a implementacao simples ficar ruim ou se a arquitetura precisar convergir ainda mais com o `AM2302`

---

## 4. Estrutura recomendada de arquivo neste passo

O caminho mais simples, a partir do estado atual do projeto, e continuar em `src/drivers/hcsr04.rs`.

Neste passo, esse arquivo deve passar a concentrar:

- `PulseMeasureError`
- trait `PulseInput`
- struct `TimPulseInput<'d, T, P>`
- construtor `new(...)`
- helpers internos de captura
- implementacao de `PulseInput` para `TimPulseInput`

Ou seja: ainda nao precisa espalhar a implementacao em varios arquivos.

---

## 5. Comportamento funcional esperado da medicao

Quando `measure_high_pulse(timeout)` for chamada, a implementacao deve obedecer a esta sequencia:

1. preparar o canal de captura do timer
2. esperar o inicio do pulso `HIGH` no `ECHO`
3. registrar o timestamp da subida
4. esperar o final do mesmo pulso `HIGH`
5. registrar o timestamp da descida
6. calcular `fall - rise`
7. devolver a largura como `Duration`

Se a subida nao vier a tempo:

- retornar `PulseMeasureError::TimeoutWaitingForRise`

Se a descida nao vier a tempo:

- retornar `PulseMeasureError::TimeoutWaitingForFall`

---

## 6. Base temporal recomendada

Para esta fase, a base mais simples e recomendada continua sendo:

- `TIM3` em `1 MHz`

Com isso:

- `1 tick = 1 us`

Essa escolha simplifica tudo:

- leitura mental dos timestamps
- logica de `wrapping_sub`
- conversao para `Duration`
- futura conversao para distancia

Exemplo conceitual:

```rust
// Base de tempo simples para que cada captura do timer represente 1 us.
let capture = InputCapture::new(
    timer.reborrow(),
    None,
    Some(ch2),
    None,
    None,
    irq,
    Hertz(1_000_000),
    CountingMode::EdgeAlignedUp,
);
```

---

## 7. Como a struct do passo 3 deve evoluir agora

O esqueleto atual em `src/drivers/hcsr04.rs` esta assim, em essencia:

```rust
// Implementacao generica alinhada com o estilo do AM2302.
pub struct TimPulseInput<'d, T, P>
where
    T: GeneralInstance4Channel,
    P: TimerPin<T, Ch2>,
{
    // Pino ligado ao ECHO.
    pin: Peri<'d, P>,
    // Timer usado para medir o pulso HIGH.
    timer: Peri<'d, T>,
}
```

No passo 4, essa struct ainda pode continuar exatamente com esses campos.

O que muda e que agora ela passa a ter comportamento real:

- helper para criar `InputCapture`
- helper para esperar uma borda
- implementacao de `measure_high_pulse(...)`

Isso mantem a evolucao pequena e coerente com o principio de menor mudanca correta.

---

## 8. Imports e recursos que entram neste passo

Para sair do esqueleto e chegar na captura real, o guia recomenda estudar e usar estes elementos do `embassy-stm32`:

```rust
use embassy_stm32::gpio::Pull;
use embassy_stm32::interrupt::typelevel::Binding;
use embassy_stm32::time::Hertz;
use embassy_stm32::timer::input_capture::{CapturePin, Ch2, InputCapture};
use embassy_stm32::timer::low_level::CountingMode;
use embassy_stm32::timer::{
    CaptureCompareInterruptHandler,
    GeneralInstance4Channel,
    TimerPin,
};
use embassy_stm32::Peri;
use embassy_time::{with_timeout, Duration};
```

Esses itens sao suficientes para a primeira implementacao sem `DMA`.

---

## 9. Helper recomendado para montar o `InputCapture`

Assim como no `AM2302`, vale encapsular a criacao do periferico de captura em um helper interno.

Exemplo de formato recomendado:

```rust
impl<'d, T, P> TimPulseInput<'d, T, P>
where
    // Restricao mais pratica para comecar em base de 16 bits.
    T: GeneralInstance4Channel<Word = u16>,
    P: TimerPin<T, Ch2>,
{
    // Reconfigura o pino como entrada de captura no canal 2.
    fn make_capture<'a>(
        pin: &'a mut Peri<'d, P>,
        timer: &'a mut Peri<'d, T>,
        irq: impl Binding<T::CaptureCompareInterrupt, CaptureCompareInterruptHandler<T>> + 'a,
    ) -> InputCapture<'a, T> {
        // Liga o ECHO ao canal 2 do timer sem pull interno.
        let ch2 = CapturePin::new(pin.reborrow(), Pull::None);

        // Timer sobe em 1 MHz para que cada tick represente 1 us.
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
}
```

Pontos importantes desse helper:

- `Pull::None`: o `ECHO` ja chega dirigido pelo modulo, e ainda passa pelo divisor resistivo
- `Some(ch2)`: o canal fechado da bancada e o `CH2`
- `Word = u16`: ajuda a manter a primeira conta simples e coerente com o estilo do `AM2302`

---

## 10. Estrategia recomendada para capturar o pulso sem DMA

Como neste caso so queremos um pulso `HIGH`, a primeira estrategia boa e simples e:

1. esperar uma captura de subida
2. guardar `rise_tick`
3. esperar uma captura de descida
4. guardar `fall_tick`
5. calcular `fall_tick.wrapping_sub(rise_tick)`

Conceitualmente, o helper pode ficar assim:

```rust
impl<'d, T, P> TimPulseInput<'d, T, P>
where
    T: GeneralInstance4Channel<Word = u16>,
    P: TimerPin<T, Ch2>,
{
    // Espera uma borda de subida no canal 2 e devolve o timestamp capturado.
    async fn wait_for_rise(
        capture: &mut InputCapture<'_, T>,
        timeout: Duration,
    ) -> Result<u16, PulseMeasureError> {
        with_timeout(timeout, capture.wait_for_rising_edge::<Ch2>())
            .await
            .map_err(|_| PulseMeasureError::TimeoutWaitingForRise)?;

        // Le o valor capturado depois que a interrupcao indicar evento valido.
        capture
            .get_capture_value::<Ch2>()
            .ok_or(PulseMeasureError::InvalidCapture)
    }

    // Espera a borda de descida do mesmo pulso HIGH e devolve o timestamp.
    async fn wait_for_fall(
        capture: &mut InputCapture<'_, T>,
        timeout: Duration,
    ) -> Result<u16, PulseMeasureError> {
        with_timeout(timeout, capture.wait_for_falling_edge::<Ch2>())
            .await
            .map_err(|_| PulseMeasureError::TimeoutWaitingForFall)?;

        capture
            .get_capture_value::<Ch2>()
            .ok_or(PulseMeasureError::InvalidCapture)
    }
}
```

Observacao importante:

- os nomes exatos dos metodos podem variar conforme a API concreta da versao de `embassy-stm32` do projeto
- a intencao deste guia e fechar a estrategia, nao prometer nome de metodo sem checagem local

Entao, ao implementar, compare com a API real de `InputCapture` disponivel no projeto.

---

## 11. Implementacao recomendada da trait `PulseInput`

Depois dos helpers, a implementacao principal deve ficar curta e legivel.

Formato recomendado:

```rust
impl<'d, T, P> PulseInput for TimPulseInput<'d, T, P>
where
    T: GeneralInstance4Channel<Word = u16>,
    P: TimerPin<T, Ch2>,
{
    type Error = PulseMeasureError;

    // Mede a largura do pulso HIGH usando duas capturas consecutivas.
    async fn measure_high_pulse(&mut self, timeout: Duration) -> Result<Duration, Self::Error> {
        let Self { pin, timer } = self;

        // Mesmo estilo do AM2302: cria a interface concreta de captura so quando vai medir.
        let mut capture = Self::make_capture(pin, timer, /* irq aqui */);

        // Primeira borda: inicio do pulso HIGH.
        let rise_tick = Self::wait_for_rise(&mut capture, timeout).await?;

        // Segunda borda: fim do mesmo pulso HIGH.
        let fall_tick = Self::wait_for_fall(&mut capture, timeout).await?;

        // O timer esta em 1 MHz, entao ticks e microssegundos coincidem.
        let width_us = fall_tick.wrapping_sub(rise_tick);

        // Protege contra largura nula ou claramente invalida para a logica atual.
        if width_us == 0 {
            return Err(PulseMeasureError::InvalidCapture);
        }

        Ok(Duration::from_micros(width_us as u64))
    }
}
```

Essa e a espinha dorsal esperada do passo 4.

---

## 12. Como tratar o `irq` nesta fase

Aqui existe uma decisao pratica importante.

O `InputCapture::new(...)` precisa de um binding de interrupcao, assim como aconteceu no `AM2302`.

Entao, para esta fase, a forma mais coerente com o estado atual do projeto e uma destas duas:

1. receber o `irq` no proprio metodo de medicao
2. receber o `irq` em um metodo concreto da struct, deixando a trait mais limpa para depois

A opcao mais proxima do que ja existe no `AM2302` e esta:

```rust
impl<'d, T, P> TimPulseInput<'d, T, P>
where
    T: GeneralInstance4Channel<Word = u16>,
    P: TimerPin<T, Ch2>,
{
    // Versao concreta para esta placa: a trait pode continuar enxuta,
    // mas a implementacao real precisa do binding de interrupcao.
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
```

Depois, no passo 5 ou na integracao, voce decide se:

- a trait `PulseInput` tambem recebe o `irq`
- ou se um wrapper guarda esse binding de outra forma

Para o passo 4, o importante e provar a captura real do pulso.

---

## 13. Sobre timeout neste passo

Uma duvida comum aqui e: usar um timeout unico ou dois timeouts separados?

Para o passo 4, a forma mais simples e aceitavel e:

- usar o mesmo `timeout` para esperar a subida
- usar o mesmo `timeout` para esperar a descida

Isso gera comportamento claro e facilita debug.

Depois, se voce quiser refinar, pode separar:

- `rise_timeout`
- `fall_timeout`

Mas ainda nao e necessario agora.

---

## 14. Sobre overflow do contador

Como a base sugerida e `1 MHz` com `u16`, o contador gira em cerca de `65,536 ms`.

O `HC-SR04` trabalha com janelas muito menores que isso.

Entao, para a primeira implementacao:

- `wrapping_sub` ja cobre bem a conta de largura
- `PulseMeasureError::Overflow` pode continuar existindo no enum
- mas ele talvez nem precise ser emitido na primeira versao concreta

Ou seja: manter o variant e valido, mesmo que ele ainda nao seja usado.

---

## 15. O que testar antes de avancar para o passo 5

O passo 4 deve ser validado antes de construir o driver completo `HcSr04`.

Checklist recomendado:

- `TimPulseInput::new(...)` compila com `PC7` e `TIM3`
- a criacao de `InputCapture` no `CH2` compila
- a medicao devolve `Duration` em vez de ticks crus
- ausencia de subida gera `TimeoutWaitingForRise`
- subida sem descida gera `TimeoutWaitingForFall`
- uma medicao valida devolve largura nao nula

Se esse checklist fechar, o passo 4 ja cumpriu o papel dele.

---

## 16. O que ainda nao pertence a este passo

Ainda nao e objetivo do passo 4:

- gerar o pulso de `TRIG` do `HC-SR04`
- converter tempo para centimetros
- criar `HcSr04Error`
- criar `HcSr04<TRIG, ECHO>`
- spawnar task periodica
- expor leitura pela shell
- introduzir `DMA` sem necessidade comprovada

Tudo isso entra melhor no passo 5 em diante.

---

## 17. Esqueleto de codigo sugerido para este passo

Este bloco junta a direcao de implementacao em uma forma unica e proxima do estado atual do projeto:

```rust
use defmt::Format;
use embassy_stm32::gpio::Pull;
use embassy_stm32::interrupt::typelevel::Binding;
use embassy_stm32::time::Hertz;
use embassy_stm32::timer::input_capture::{CapturePin, Ch2, InputCapture};
use embassy_stm32::timer::low_level::CountingMode;
use embassy_stm32::timer::{
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
    // Overflow de contador ou caso ainda nao tratado.
    Overflow,
}

// Trait consumida pelo driver do HC-SR04.
pub trait PulseInput {
    // Tipo de erro especifico da implementacao concreta.
    type Error;

    // Mede a largura do pulso HIGH do ECHO.
    async fn measure_high_pulse(
        &mut self,
        // Tempo maximo de espera pela medicao.
        timeout: Duration,
    ) -> Result<Duration, Self::Error>;
}

// Implementacao generica alinhada com o estilo do AM2302.
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
    // Construtor minimo; o hardware real continua sendo injetado de fora.
    pub fn new(pin: Peri<'d, P>, timer: Peri<'d, T>) -> Self {
        Self { pin, timer }
    }

    // Reconfigura o pino como entrada de captura no canal 2 do timer.
    fn make_capture<'a>(
        pin: &'a mut Peri<'d, P>,
        timer: &'a mut Peri<'d, T>,
        irq: impl Binding<T::CaptureCompareInterrupt, CaptureCompareInterruptHandler<T>> + 'a,
    ) -> InputCapture<'a, T> {
        let ch2 = CapturePin::new(pin.reborrow(), Pull::None);

        // 1 tick = 1 us.
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

    // Espera a borda de subida que marca o inicio do pulso HIGH.
    async fn wait_for_rise(
        capture: &mut InputCapture<'_, T>,
        timeout: Duration,
    ) -> Result<u16, PulseMeasureError> {
        with_timeout(timeout, capture.wait_for_rising_edge::<Ch2>())
            .await
            .map_err(|_| PulseMeasureError::TimeoutWaitingForRise)?;

        capture
            .get_capture_value::<Ch2>()
            .ok_or(PulseMeasureError::InvalidCapture)
    }

    // Espera a borda de descida que fecha a largura do pulso HIGH.
    async fn wait_for_fall(
        capture: &mut InputCapture<'_, T>,
        timeout: Duration,
    ) -> Result<u16, PulseMeasureError> {
        with_timeout(timeout, capture.wait_for_falling_edge::<Ch2>())
            .await
            .map_err(|_| PulseMeasureError::TimeoutWaitingForFall)?;

        capture
            .get_capture_value::<Ch2>()
            .ok_or(PulseMeasureError::InvalidCapture)
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
```

Esse esqueleto nao tenta resolver a fase inteira, mas deixa a implementacao concreta do passo 4 bem encaminhada.

---

## 18. Criterio de conclusao do passo 4

Considere o passo 4 concluido quando existir:

- uma implementacao concreta de captura sobre `TIM3_CH2`
- base temporal em `1 MHz`
- espera de subida e descida com timeout
- retorno em `Duration`
- uso de `PulseMeasureError` coerente com as falhas reais da medicao
- uma primeira validacao de compilacao do adaptador concreto no projeto

Nao precisa ainda neste passo:

- distancia em `cm`
- integracao com shell
- task periodica
- `DMA`

---

## 19. Decisao final deste passo

Para o estado atual do projeto, a recomendacao mais limpa e:

- manter `src/drivers/hcsr04.rs` como ponto central
- implementar `TimPulseInput<'d, T, P>` sem aumentar a generificacao desnecessariamente
- usar `TIM3_CH2` em `1 MHz`
- capturar um par `rise/fall` sem `DMA`
- devolver `Duration`
- deixar `HcSr04` e integracao para o passo seguinte

Esse e o menor passo correto para sair da abstracao e entrar na medicao real do `ECHO`.
