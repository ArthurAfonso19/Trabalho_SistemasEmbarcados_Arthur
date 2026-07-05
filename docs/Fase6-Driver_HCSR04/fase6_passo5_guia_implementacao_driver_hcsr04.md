## Fase 6 - Passo 5: guia de implementacao do driver secundario `HcSr04`

Este documento cobre o passo 5 da Fase 6.

Objetivo do passo:

- construir o driver secundario do `HC-SR04` em cima da base temporal do passo 4
- separar claramente a logica do sensor da logica de captura do `ECHO`
- deixar pronta a API que depois sera usada pela task periodica e pelo comando `sensors`

Base ja fechada antes deste passo:

- `TRIG = PA6 = D12`
- `ECHO = PC7 = D9`
- `ECHO` protegido por divisor resistivo
- `TIM3_CH2` escolhido como caminho de captura
- `src/drivers/hcsr04.rs` ja contem `PulseMeasureError`
- `src/drivers/hcsr04.rs` ja contem `TimPulseInput<'d, T, P>` com `measure_high_pulse_with_irq(...)`

---

## 1. O que este passo deve entregar

Ao final deste passo, o projeto ainda nao precisa ter task periodica nem shell integrada.

Ele precisa entregar o driver do sensor propriamente dito:

```rust
// Driver secundario do HC-SR04, desacoplado do timer concreto.
pub struct HcSr04<TRIG, ECHO> {
    // GPIO de saida usado para disparar o sensor.
    trig: TRIG,
    // Adaptador responsavel por medir a largura do pulso HIGH do ECHO.
    echo: ECHO,
}
```

E a operacao central esperada desta fase:

```rust
// Fluxo de alto nivel da medicao do sensor.
pub async fn measure_distance_cm(...) -> Result<f32, HcSr04Error>;
```

Traduzindo:

- o driver sobe `TRIG` por `10 us`
- o driver pede a largura do pulso `HIGH` para a camada de `echo`
- o driver converte esse tempo em distancia
- o driver entrega `cm` como `f32`

---

## 2. Por que este passo e um driver secundario

O `HC-SR04` nao e apenas um pino ou um timer.

Ele e a combinacao de duas responsabilidades diferentes:

- gerar o trigger do modulo
- interpretar a largura do eco como distancia

A medicao do `ECHO` por si so nao e a logica completa do sensor.

Por isso a divisao correta continua sendo:

- camada primaria: medicao temporal do `ECHO`
- camada secundaria: logica do `HC-SR04`

Em termos de arquitetura:

- `TimPulseInput<'d, T, P>` responde: "quanto tempo o `ECHO` ficou em HIGH?"
- `HcSr04<TRIG, ECHO>` responde: "dispare o sensor e converta esse pulso em distancia"

Essa e a mesma filosofia geral usada no `AM2302` e inspirada no desenho do `ADXL345`.

---

## 3. O que muda do passo 4 para o passo 5

No passo 4, o foco era provar a medicao temporal do `ECHO`.

No passo 5, o foco passa a ser:

- encapsular o `TRIG`
- definir erro proprio do `HC-SR04`
- converter tempo para `cm`
- expor uma API limpa de leitura do sensor

Neste passo, o driver deixa de ser apenas "captura de pulso" e passa a representar o sensor completo.

---

## 4. Estado atual e como seguir a partir dele

Hoje, `src/drivers/hcsr04.rs` esta em um ponto intermediario importante:

- a trait `PulseInput` existe, mas ainda nao esta ligada ao caminho concreto do Embassy
- a implementacao concreta funcional neste momento e `measure_high_pulse_with_irq(...)`

Entao, para este passo 5, a recomendacao mais pratica e:

- usar o metodo concreto atual como base da validacao
- construir o `HcSr04` de forma que a logica do sensor fique pronta
- deixar o refinamento final da trait como ajuste posterior, se necessario

Ou seja: o passo 5 pode avancar, desde que voce aceite este estado intermediario de integracao.

---

## 5. Erro proprio recomendado para o driver do sensor

O `HC-SR04` deve ter um erro proprio, separado do erro da camada de captura.

Motivo:

- `PulseMeasureError` descreve problemas de tempo e bordas
- `HcSr04Error` descreve problemas do sensor do ponto de vista do chamador

Forma recomendada:

```rust
use defmt::Format;

// Erros de alto nivel do driver do HC-SR04.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum HcSr04Error<E> {
    // Falha ao dirigir o pino TRIG.
    TriggerPin,
    // Falha propagada pela camada que mede o ECHO.
    Echo(E),
    // O tempo medido nao fez sentido para a conversao atual.
    InvalidPulse,
}
```

Observacao:

- `E` representa o erro concreto da camada que mede o `ECHO`
- isso mantem o driver reutilizavel

---

## 6. Struct recomendada para o driver `HcSr04`

O esqueleto ideal para esta fase e simples:

```rust
use embedded_hal::digital::OutputPin;

// Driver secundario do sensor ultrassonico.
pub struct HcSr04<TRIG, ECHO> {
    // Saida digital que dispara o burst ultrassonico.
    trig: TRIG,
    // Camada que mede a largura do pulso HIGH em ECHO.
    echo: ECHO,
}
```

Na forma mais generica:

```rust
impl<TRIG, ECHO> HcSr04<TRIG, ECHO>
where
    TRIG: OutputPin,
    ECHO: PulseInput,
{
    // Construtor minimo do driver do sensor.
    pub fn new(trig: TRIG, echo: ECHO) -> Self {
        Self { trig, echo }
    }
}
```

Essa continua sendo a melhor API de alto nivel para o driver.

---

## 7. Como lidar com o estado atual da trait `PulseInput`

Aqui existe o principal detalhe pratico desta fase.

No desenho arquitetural ideal, o driver seria algo assim:

```rust
impl<TRIG, ECHO> HcSr04<TRIG, ECHO>
where
    TRIG: OutputPin,
    ECHO: PulseInput<Error = PulseMeasureError>,
{
    // Forma ideal depois que a trait estiver plenamente integrada.
    pub async fn measure_distance_cm(&mut self) -> Result<f32, HcSr04Error<PulseMeasureError>> {
        // usa self.echo.measure_high_pulse(...)
        todo!()
    }
}
```

Mas, no estado atual do projeto, a implementacao concreta validada ainda e:

- `TimPulseInput::measure_high_pulse_with_irq(...)`

Entao, para este passo 5, existem duas formas corretas de seguir.

### Opcao recomendada agora

- documentar o `HcSr04` na forma ideal generica
- implementar a primeira versao concreta com um metodo adicional que receba `irq`

Exemplo conceitual:

```rust
impl<'d, TRIG, T, P> HcSr04<TRIG, TimPulseInput<'d, T, P>>
where
    TRIG: OutputPin,
    T: GeneralInstance4Channel<Word = u16>,
    P: TimerPin<T, Ch2>,
{
    // Primeira versao concreta enquanto a trait PulseInput ainda nao fecha o irq.
    pub async fn measure_distance_cm_with_irq<I>(
        &mut self,
        timeout: Duration,
        irq: I,
    ) -> Result<f32, HcSr04Error<PulseMeasureError>>
    where
        I: Binding<T::CaptureCompareInterrupt, CaptureCompareInterruptHandler<T>> + Copy + 'd,
    {
        todo!()
    }
}
```

Essa abordagem respeita o estado real do projeto sem mentir sobre a abstracao.

---

## 8. Sequencia funcional que o driver deve obedecer

O fluxo da leitura do `HC-SR04` deve ser este:

1. garantir `TRIG` em `LOW`
2. aguardar alguns microssegundos para estabilizar
3. colocar `TRIG` em `HIGH`
4. manter `HIGH` por `10 us`
5. voltar `TRIG` para `LOW`
6. medir o pulso `HIGH` em `ECHO`
7. converter esse tempo para distancia
8. devolver `cm`

Separando responsabilidades:

- `TRIG`: responsabilidade do `HcSr04`
- `ECHO`: responsabilidade da camada de captura

---

## 9. Temporizacao do `TRIG`

O datasheet do `HC-SR04` pede um pulso de trigger de `10 us`.

Para este projeto, a forma mais simples e coerente e usar um delay curto bloqueante, assim como o repositorio ja aceitou no `AM2302` para a parte realmente fina de temporizacao.

Exemplo de helper recomendado:

```rust
struct HcSr04TriggerDelay;

impl embedded_hal::delay::DelayNs for HcSr04TriggerDelay {
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
        // Reaproveita delay_us para manter a implementacao simples.
        for _ in 0..ms {
            self.delay_us(1000);
        }
    }
}
```

Para este passo, o foco e usar `delay_us(10)` de forma clara e previsivel.

---

## 10. Conversao recomendada de tempo para distancia

O calculo pratico mais comum para o `HC-SR04` e:

- `distancia_cm = tempo_us / 58.0`

Porque o pulso `ECHO` representa ida e volta do som.

Entao, depois de medir um `Duration`, a conversao pode ser assim:

```rust
// Converte a largura do pulso HIGH em distancia aproximada em centimetros.
fn pulse_to_distance_cm(pulse: Duration) -> Result<f32, HcSr04Error<PulseMeasureError>> {
    let pulse_us = pulse.as_micros();

    // Protege a API contra largura nula.
    if pulse_us == 0 {
        return Err(HcSr04Error::InvalidPulse);
    }

    Ok(pulse_us as f32 / 58.0)
}
```

Esse helper deixa a regra explicita e facilita debug.

---

## 11. Implementacao recomendada do disparo de `TRIG`

O disparo deve ficar pequeno e isolado.

Exemplo:

```rust
impl<TRIG, ECHO> HcSr04<TRIG, ECHO>
where
    TRIG: OutputPin,
{
    // Gera o pulso de 10 us exigido pelo HC-SR04.
    fn trigger_sensor(&mut self) -> Result<(), HcSr04Error<ECHO>> {
        let mut delay = HcSr04TriggerDelay;

        // Garante partida em LOW antes de emitir o trigger.
        self.trig.set_low().map_err(|_| HcSr04Error::TriggerPin)?;
        delay.delay_us(2);

        // Pulso de trigger propriamente dito.
        self.trig.set_high().map_err(|_| HcSr04Error::TriggerPin)?;
        delay.delay_us(10);
        self.trig.set_low().map_err(|_| HcSr04Error::TriggerPin)?;

        Ok(())
    }
}
```

Observacao:

- o tipo exato do erro do `OutputPin` pode pedir pequenos ajustes na assinatura final
- o principal aqui e isolar a logica de trigger

---

## 12. Forma recomendada da medicao completa nesta fase

No estado atual do projeto, o caminho mais honesto e fazer primeiro uma versao concreta sobre `TimPulseInput`.

Exemplo de direcao:

```rust
use embassy_time::Duration;

// Timeout inicial pratico para ausencia de eco ou alvo longe demais.
const ECHO_TIMEOUT: Duration = Duration::from_millis(40);

impl<'d, TRIG, T, P> HcSr04<TRIG, TimPulseInput<'d, T, P>>
where
    TRIG: OutputPin,
    T: GeneralInstance4Channel<Word = u16>,
    P: TimerPin<T, Ch2>,
{
    // Primeira medicao funcional apoiada no metodo concreto do passo 4.
    pub async fn measure_distance_cm_with_irq<I>(
        &mut self,
        irq: I,
    ) -> Result<f32, HcSr04Error<PulseMeasureError>>
    where
        I: Binding<T::CaptureCompareInterrupt, CaptureCompareInterruptHandler<T>> + Copy + 'd,
    {
        // Dispara o burst do sensor.
        self.trigger_sensor()?;

        // Mede quanto tempo o ECHO ficou em HIGH.
        let pulse = self
            .echo
            .measure_high_pulse_with_irq(ECHO_TIMEOUT, irq)
            .await
            .map_err(HcSr04Error::Echo)?;

        // Converte o tempo capturado em centimetros.
        Self::pulse_to_distance_cm(pulse)
    }
}
```

Esse desenho e suficiente para fechar a primeira versao do driver do sensor.

---

## 13. Sobre a trait `PulseInput` neste passo

Uma pergunta natural aqui e:

- "eu preciso fechar a trait `PulseInput` agora?"

Resposta pratica:

- nao obrigatoriamente para avancar no passo 5

O que voce precisa para o passo 5 e:

- uma forma compilavel de medir o pulso do `ECHO`
- uma forma compilavel de disparar `TRIG`
- uma API do sensor que devolva `cm`

Se isso estiver fechado por um metodo concreto com `irq`, o passo 5 ja avanca.

Depois, se quiser refinar a arquitetura, voce pode:

1. adaptar `PulseInput` para o caso concreto do Embassy
2. criar um wrapper que esconda o `irq`
3. voltar o `HcSr04` para a forma 100% generica planejada

---

## 14. O que testar antes de avancar para task periodica

Checklist minimo deste passo:

- `HcSr04::new(...)` compila
- o helper de `TRIG` sobe e desce o pino sem erro
- `measure_distance_cm_with_irq(...)` compila
- um erro da camada `echo` e propagado como `HcSr04Error::Echo(...)`
- um pulso valido e convertido para `f32` em `cm`
- largura nula gera `InvalidPulse`

Se esse checklist fechar, o passo 5 cumpriu o papel dele.

---

## 15. O que ainda nao pertence a este passo

Ainda nao e objetivo do passo 5:

- criar a task periodica do `HC-SR04`
- armazenar ultimo valor em snapshot compartilhado
- integrar ao comando `sensors`
- otimizar politica de amostragem
- introduzir `DMA`
- fechar a forma final da trait `PulseInput` se a versao concreta ja estiver funcional

Tudo isso entra melhor no passo 6 em diante da integracao local da Fase 6.

---

## 16. Esqueleto de codigo sugerido para este passo

Este bloco resume a direcao de implementacao a partir do estado real atual do projeto:

```rust
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
    fn trigger_sensor(&mut self) -> Result<(), HcSr04Error<ECHO>> {
        let mut delay = HcSr04TriggerDelay;

        self.trig.set_low().map_err(|_| HcSr04Error::TriggerPin)?;
        delay.delay_us(2);

        self.trig.set_high().map_err(|_| HcSr04Error::TriggerPin)?;
        delay.delay_us(10);
        self.trig.set_low().map_err(|_| HcSr04Error::TriggerPin)?;

        Ok(())
    }

    // Converte largura de pulso em distancia aproximada.
    fn pulse_to_distance_cm(pulse: Duration) -> Result<f32, HcSr04Error<ECHO>> {
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
```

Esse esqueleto e o menor caminho correto para o passo 5 a partir do estado atual do projeto.

---

## 17. Criterio de conclusao do passo 5

Considere o passo 5 concluido quando existir:

- `HcSr04Error`
- `HcSr04<TRIG, ECHO>`
- construtor `new(trig, echo)`
- helper de trigger com `10 us`
- conversao de `Duration` para `cm`
- uma primeira medicao concreta usando `measure_high_pulse_with_irq(...)`

Ainda nao precisa neste passo:

- task periodica
- snapshot compartilhado
- shell
- refinamento final da trait `PulseInput`

---

## 18. Decisao final deste passo

Para o estado atual do projeto, a recomendacao mais limpa e:

- criar o `HcSr04` em `src/drivers/hcsr04.rs`
- tratar `TRIG` e `ECHO` como responsabilidades separadas
- usar `measure_high_pulse_with_irq(...)` como ponte concreta temporaria
- devolver distancia em `cm` como `f32`
- deixar a task periodica e a shell para a etapa seguinte

Esse e o menor passo correto para transformar a captura de pulso em um driver real de sensor.
