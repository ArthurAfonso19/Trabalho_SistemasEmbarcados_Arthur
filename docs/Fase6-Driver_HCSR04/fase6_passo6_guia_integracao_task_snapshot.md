## Fase 6 - Passo 6: guia de integracao do `HC-SR04` com task periodica e snapshot

Este documento cobre o passo 6 da Fase 6.

Objetivo do passo:

- integrar o `HC-SR04` ao firmware real
- criar uma task periodica propria do sensor
- publicar o ultimo estado conhecido em um snapshot compartilhado
- manter a shell apenas como leitora desse estado

Base ja fechada antes deste passo:

- `TRIG = PA6 = D12`
- `ECHO = PC7 = D9`
- `ECHO` protegido por divisor resistivo
- `TIM3_CH2` definido como caminho de captura
- `TimPulseInput<'d, T, P>` existe com `measure_high_pulse_with_irq(...)`
- `HcSr04<TRIG, ECHO>` existe e compila

---

## 1. O que este passo deve entregar

Ao final deste passo, o `HC-SR04` ja deve participar do firmware da mesma forma geral que o `AM2302` participa hoje.

Isso significa entregar:

- uma task periodica do `HC-SR04`
- uma instancia real do sensor no `main`
- um snapshot compartilhado com ultima distancia valida e ultimo erro
- logs de bancada para observar sucesso e falha

Ainda nao e obrigatorio neste passo:

- integrar a exibicao final no comando `sensors`
- refinar a forma final da trait `PulseInput`
- reorganizar todos os modulos do driver

---

## 2. Filosofia desta integracao

O plano da Fase 6 ja fechou um principio importante:

- a shell nao deve medir diretamente o sensor
- a shell deve apenas ler o ultimo estado conhecido

Entao a arquitetura deste passo fica assim:

1. task do `HC-SR04` roda em loop
2. task mede distancia periodicamente
3. task atualiza `MONITOR`
4. shell apenas le `MONITOR`

Isso segue o mesmo padrao real que o projeto ja usa com o `AM2302`.

---

## 3. O que mudar no firmware

Neste passo, os pontos naturais de alteracao sao:

- `src/main.rs`
- `src/app/monitor.rs`
- opcionalmente `src/app/shell.rs` so no passo seguinte

O trabalho principal agora e:

- ligar `PA6`, `PC7` e `TIM3` ao driver
- criar a task do sensor
- guardar o resultado no monitor compartilhado

---

## 4. Primeiro ajuste: IRQ do `TIM3`

O `TimPulseInput` concreto depende da IRQ de capture/compare do `TIM3`.

Hoje o projeto ja tem binding para `TIM4`, por causa do `AM2302`.

Entao o primeiro passo da integracao e adicionar `TIM3` no `bind_interrupts!` de `src/main.rs`.

Exemplo:

```rust
bind_interrupts!(struct Irqs {
    LPUART1 => embassy_stm32::usart::InterruptHandler<peripherals::LPUART1>;
    DMA1_CHANNEL1 => embassy_stm32::dma::InterruptHandler<peripherals::DMA1_CH1>;
    DMA1_CHANNEL2 => embassy_stm32::dma::InterruptHandler<peripherals::DMA1_CH2>;

    I2C1_EV => embassy_stm32::i2c::EventInterruptHandler<peripherals::I2C1>;
    I2C1_ER => embassy_stm32::i2c::ErrorInterruptHandler<peripherals::I2C1>;
    DMA1_CHANNEL3 => embassy_stm32::dma::InterruptHandler<peripherals::DMA1_CH3>;
    DMA1_CHANNEL4 => embassy_stm32::dma::InterruptHandler<peripherals::DMA1_CH4>;

    // Canal DMA usado pelo TIM4_CH1 na implementacao validada do AM2302.
    DMA1_CHANNEL5 => embassy_stm32::dma::InterruptHandler<peripherals::DMA1_CH5>;
    EXTI15_10 => exti::InterruptHandler<interrupt::typelevel::EXTI15_10>;

    // IRQ de capture/compare do TIM3 usada pelo HC-SR04.
    TIM3 => embassy_stm32::timer::CaptureCompareInterruptHandler<peripherals::TIM3>;
    // IRQ de capture/compare do TIM4 usada pelo AM2302.
    TIM4 => embassy_stm32::timer::CaptureCompareInterruptHandler<peripherals::TIM4>;
});
```

Sem isso, a medicao concreta do `HC-SR04` nao fecha.

---

## 5. Segundo ajuste: snapshot do `HC-SR04` no monitor

O melhor modelo aqui e copiar a forma que o `AM2302` ja usa em `src/app/monitor.rs`.

A ideia e criar um snapshot proprio do ultrassonico.

Forma recomendada:

```rust
use crate::drivers::Hc_Sr_04::HcSr04Error;
use crate::drivers::hcsr04::PulseMeasureError;

// Snapshot mais recente do HC-SR04.
#[derive(Debug, Clone, Copy)]
pub struct HcSr04Snapshot {
    pub distance_cm: Option<f32>,
    pub last_update_ms: Option<u64>,
    pub last_error: Option<HcSr04Error<PulseMeasureError>>,
}

impl HcSr04Snapshot {
    // Estado inicial: ainda nao existe leitura valida.
    pub const fn new() -> Self {
        Self {
            distance_cm: None,
            last_update_ms: None,
            last_error: None,
        }
    }

    // Atualiza o snapshot quando uma medicao termina com sucesso.
    pub fn update_success(&mut self, distance_cm: f32, now_ms: u64) {
        self.distance_cm = Some(distance_cm);
        self.last_update_ms = Some(now_ms);
        self.last_error = None;
    }

    // Registra apenas o ultimo erro sem apagar a ultima distancia valida.
    pub fn update_error(&mut self, error: HcSr04Error<PulseMeasureError>) {
        self.last_error = Some(error);
    }
}
```

Depois, esse snapshot deve entrar em `SystemMonitor`:

```rust
pub struct SystemMonitor {
    // Snapshot mais recente do AM2302.
    pub am2302: Am2302Snapshot,
    // Snapshot mais recente do HC-SR04.
    pub hcsr04: HcSr04Snapshot,
}
```

E na inicializacao:

```rust
pub const fn new() -> Self {
    Self {
        // Estado inicial do AM2302 antes da primeira leitura.
        am2302: Am2302Snapshot::new(),
        // Estado inicial do HC-SR04 antes da primeira leitura.
        hcsr04: HcSr04Snapshot::new(),
    }
}
```

---

## 6. Terceiro ajuste: task periodica do `HC-SR04`

O padrao mais seguro e repetir o estilo da task do `AM2302` em `src/main.rs`.

Ou seja:

- loop infinito
- mede o sensor
- atualiza snapshot no `MONITOR`
- faz log de sucesso ou erro
- espera um intervalo entre medicoes

Forma recomendada:

```rust
#[embassy_executor::task]
async fn hcsr04_task(
    mut sensor: HcSr04<
        Output<'static>,
        TimPulseInput<'static, peripherals::TIM3, peripherals::PC7>,
    >,
) {
    loop {
        match sensor.measure_distance_cm_with_irq(Irqs).await {
            Ok(distance_cm) => {
                // Captura o instante em que a medicao terminou com sucesso.
                let now_ms = Instant::now().as_millis() as u64;

                // Atualiza o snapshot compartilhado do HC-SR04.
                {
                    let mut monitor = MONITOR.lock().await;
                    monitor.hcsr04.update_success(distance_cm, now_ms);
                }

                // Mantem o log RTT para observacao em bancada.
                info!("HC-SR04: {} cm", distance_cm);
            }

            Err(error) => {
                // Registra o erro mais recente sem apagar a ultima leitura valida.
                {
                    let mut monitor = MONITOR.lock().await;
                    monitor.hcsr04.update_error(error);
                }

                warn!("HC-SR04 error: {:?}", error);
            }
        }

        // Evita congestionamento acustico e da CPU.
        Timer::after_millis(500).await;
    }
}
```

Observacoes importantes:

- `500 ms` e um bom ponto de partida simples
- `1 s` tambem seria aceitavel
- o mais importante e nao medir em loop apertado

---

## 7. Quarto ajuste: instanciar o sensor no `main`

Depois da task, o `main` precisa criar os recursos concretos da bancada.

A ideia e seguir exatamente os pinos ja decididos:

- `TRIG = PA6`
- `ECHO = PC7`
- `TIM3`

Exemplo de direcao:

```rust
use crate::drivers::Hc_Sr_04::HcSr04;
use crate::drivers::hcsr04::TimPulseInput;
use embassy_stm32::gpio::{Level, Output, Speed};

// TRIG como GPIO de saida em nivel baixo inicial.
let trig = Output::new(p.PA6, Level::Low, Speed::Low);

// ECHO medido pelo caminho concreto definido na bancada.
let echo = TimPulseInput::new(p.PC7, p.TIM3);

// Driver do sensor completo.
let hcsr04 = HcSr04::new(trig, echo);
```

O ponto importante aqui e que o `HC-SR04` nasce no `main`, mas quem mede de verdade e a task dedicada.

---

## 8. Quinto ajuste: spawn da task

Com a task pronta e a instancia criada, basta adicionar o `spawn` junto do restante do runtime.

Exemplo:

```rust
// Runtime concorrente.
spawner.spawn(unwrap!(adc_task(adc, adc_channel)));
spawner.spawn(unwrap!(button_task(button)));
spawner.spawn(unwrap!(shell_taks(lpuart1)));
spawner.spawn(unwrap!(pwm_task(pwm)));
spawner.spawn(unwrap!(accel_task(accel)));
spawner.spawn(unwrap!(led_task(p.PA5)));
spawner.spawn(unwrap!(am2302_task(am2302)));
spawner.spawn(unwrap!(hcsr04_task(hcsr04)));
```

Com isso, o sensor passa a rodar junto com o resto do firmware.

---

## 9. O que validar neste passo

Este passo ja e a primeira validacao real em firmware do `HC-SR04`.

Checklist minimo:

- `TIM3` foi adicionado ao `bind_interrupts!`
- `HcSr04` foi instanciado com `PA6`, `PC7` e `TIM3`
- `hcsr04_task(...)` foi spawnada
- medicoes validas aparecem no RTT
- falhas aparecem como erro coerente no RTT
- `MONITOR` recebe o ultimo valor e o ultimo erro

Se isso fechar, o sensor ja esta integrado ao runtime.

---

## 10. O que ainda nao pertence a este passo

Mesmo com a integracao da task, ainda nao e objetivo do passo 6:

- formatar o `HC-SR04` no comando `sensors`
- reorganizar os dois modulos `hcsr04.rs` e `Hc_Sr_04.rs`
- eliminar todos os warnings de itens ainda nao usados
- refinar a trait `PulseInput` para a forma ideal final

Esses ajustes podem vir depois que a integracao basal estiver funcionando.

---

## 11. Como integrar ao comando `sensors` depois

O passo seguinte natural, depois desta integracao, e repetir o padrao ja existente em `src/app/shell.rs` para o `AM2302`.

Ou seja:

- copiar o snapshot do `HC-SR04` dentro do lock
- liberar o lock
- formatar a resposta com:
  - distancia atual
  - idade da amostra
  - ultimo erro, se existir

Mas isso ja e uma etapa posterior a este documento.

---

## 12. Esqueleto de codigo sugerido para este passo

Este bloco resume o menor caminho correto de integracao do `HC-SR04` ao firmware:

```rust
use crate::drivers::Hc_Sr_04::{HcSr04, HcSr04Error};
use crate::drivers::hcsr04::{PulseMeasureError, TimPulseInput};
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::peripherals;
use embassy_time::{Instant, Timer};

// Snapshot mais recente do HC-SR04.
#[derive(Debug, Clone, Copy)]
pub struct HcSr04Snapshot {
    pub distance_cm: Option<f32>,
    pub last_update_ms: Option<u64>,
    pub last_error: Option<HcSr04Error<PulseMeasureError>>,
}

impl HcSr04Snapshot {
    // Estado inicial: ainda nao houve leitura valida.
    pub const fn new() -> Self {
        Self {
            distance_cm: None,
            last_update_ms: None,
            last_error: None,
        }
    }

    // Atualiza o snapshot quando uma medicao termina com sucesso.
    pub fn update_success(&mut self, distance_cm: f32, now_ms: u64) {
        self.distance_cm = Some(distance_cm);
        self.last_update_ms = Some(now_ms);
        self.last_error = None;
    }

    // Registra o erro mais recente sem apagar a ultima leitura valida.
    pub fn update_error(&mut self, error: HcSr04Error<PulseMeasureError>) {
        self.last_error = Some(error);
    }
}

#[embassy_executor::task]
async fn hcsr04_task(
    mut sensor: HcSr04<
        Output<'static>,
        TimPulseInput<'static, peripherals::TIM3, peripherals::PC7>,
    >,
) {
    loop {
        match sensor.measure_distance_cm_with_irq(Irqs).await {
            Ok(distance_cm) => {
                // Captura o instante de uma medicao valida.
                let now_ms = Instant::now().as_millis() as u64;

                // Atualiza o snapshot compartilhado do HC-SR04.
                {
                    let mut monitor = MONITOR.lock().await;
                    monitor.hcsr04.update_success(distance_cm, now_ms);
                }

                info!("HC-SR04: {} cm", distance_cm);
            }

            Err(error) => {
                // Registra o erro sem apagar a ultima medicao valida.
                {
                    let mut monitor = MONITOR.lock().await;
                    monitor.hcsr04.update_error(error);
                }

                warn!("HC-SR04 error: {:?}", error);
            }
        }

        // Evita medir rapido demais e congestionar o sensor.
        Timer::after_millis(500).await;
    }
}

// No main, criar os recursos concretos da bancada.
let trig = Output::new(p.PA6, Level::Low, Speed::Low);
let echo = TimPulseInput::new(p.PC7, p.TIM3);
let hcsr04 = HcSr04::new(trig, echo);

// Depois, incluir a task no runtime.
spawner.spawn(unwrap!(hcsr04_task(hcsr04)));
```

Esse esqueleto e suficiente para tirar o `HC-SR04` do papel e coloca-lo dentro do runtime real do firmware.

---

## 13. Criterio de conclusao do passo 6

Considere o passo 6 concluido quando existir:

- binding de `TIM3` no `bind_interrupts!`
- task propria do `HC-SR04`
- snapshot `HcSr04Snapshot` em `SystemMonitor`
- instancia do sensor criada no `main`
- `spawn` da task no runtime
- logs RTT mostrando leitura valida ou erro coerente

Ainda nao precisa neste passo:

- shell exibindo o `HC-SR04`
- consolidacao final dos modulos do driver
- limpeza completa dos warnings

---

## 14. Decisao final deste passo

Para o estado atual do projeto, a recomendacao mais limpa e:

- integrar o `HC-SR04` exatamente como o `AM2302` ja foi integrado
- usar uma task dedicada para a medicao
- guardar apenas um snapshot compartilhado
- manter a shell fora do caminho critico

Esse e o menor passo correto para validar o driver em firmware real antes da exibicao final em `sensors`.
