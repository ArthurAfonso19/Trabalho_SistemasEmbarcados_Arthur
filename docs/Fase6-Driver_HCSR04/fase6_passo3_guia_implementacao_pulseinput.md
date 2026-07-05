## Fase 6 - Passo 3: guia de implementacao da abstracao `PulseInput`

Este documento cobre o passo 3 da Fase 6.

Objetivo do passo:

- definir a abstracao primaria de medicao de pulso para o `HC-SR04`
- preparar o terreno para a implementacao baseada em `timer input capture`
- manter o driver do sensor desacoplado dos detalhes do `TIM3`

Base de hardware ja fechada nos passos 1 e 2:

- `TRIG = PA6 = D12`
- `ECHO = PC7 = D9`
- `ECHO` protegido por divisor resistivo
- `ECHO` medido por `TIM3_CH2`

---

## 1. O que este passo deve entregar

Ao final deste passo, o codigo ainda nao precisa medir distancia.

Ele precisa deixar pronto o contrato central que o driver usara depois:

```rust
// Contrato minimo para qualquer implementacao que meca a largura
// de um pulso HIGH no pino ECHO.
pub trait PulseInput {
    // Tipo de erro concreto da implementacao.
    type Error;

    // Mede quanto tempo o sinal ficou em nivel alto.
    async fn measure_high_pulse(&mut self, timeout: Duration) -> Result<Duration, Self::Error>;
}
```

Esse contrato responde a pergunta principal do `HC-SR04`:

- "quanto tempo o `ECHO` ficou em nivel alto?"

---

## 2. Por que criar essa abstracao antes do driver final

O `HC-SR04` depende de uma medicao temporal, nao apenas de ler um GPIO.

Entao separar essa responsabilidade traz 3 vantagens:

- o driver `HcSr04` fica mais limpo
- a logica do sensor nao fica acoplada diretamente ao `TIM3`
- a implementacao concreta pode evoluir sem reescrever a API do driver

Em outras palavras:

- `HcSr04` deve saber disparar `TRIG` e converter tempo em distancia
- `PulseInput` deve saber medir o pulso `HIGH` do `ECHO`

---

## 3. Estrutura recomendada de arquivos

Para esta fase, a organizacao mais simples e:

- criar `src/drivers/hcsr04.rs`
- colocar neste arquivo:
- a trait `PulseInput`
- os tipos de erro do caminho de medicao
  - a struct generica que depois medira o `ECHO` via `timer input capture`

Neste passo 3, ainda nao e necessario criar a task periodica nem integrar a shell.

---

## 4. Forma minima recomendada para a trait

Sugestao direta:

```rust
use embassy_time::Duration;

// Interface basica que o driver do HC-SR04 vai consumir.
pub trait PulseInput {
    // Tipo de erro deixado em aberto para a implementacao concreta.
    type Error;

    // Mede a largura do pulso HIGH observado no ECHO.
    async fn measure_high_pulse(
        &mut self,
        // Timeout para nao travar a task quando nao houver eco.
        timeout: Duration,
    ) -> Result<Duration, Self::Error>;
}
```

Pontos importantes:

- `&mut self`: a medicao vai consumir estado interno do periferico
- `timeout`: evita travar a task se nao houver eco
- retorno em `Duration`: simplifica a etapa seguinte de conversao para centimetros

---

## 5. Tipo de erro recomendado para o caminho de pulso

Mesmo antes da implementacao completa, vale definir o esqueleto do erro.

Exemplo enxuto:

```rust
use defmt::Format;

// Erros iniciais pensados para a medicao de pulso do ECHO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Format)]
pub enum PulseMeasureError {
    // Nao apareceu borda de subida dentro do timeout.
    TimeoutWaitingForRise,
    // O pulso subiu, mas nao caiu dentro do timeout.
    TimeoutWaitingForFall,
    // Captura retornou dados incoerentes.
    InvalidCapture,
    // O contador do timer deixou a conta invalida para a logica atual.
    Overflow,
}
```

Interpretacao:

- `TimeoutWaitingForRise`: o sensor nao iniciou o eco
- `TimeoutWaitingForFall`: o pulso subiu, mas nao terminou a tempo
- `InvalidCapture`: timestamps incoerentes
- `Overflow`: contador girou de um jeito nao tratado pela implementacao

Neste passo, o mais importante e fechar nomes claros e uma forma estavel de propagacao de erro.

---

## 6. Struct generica recomendada para o proximo passo

Ainda sem implementar toda a captura, ja vale desenhar a struct que vai carregar o hardware.

Como a decisao agora e seguir o estilo do `AM2302`, a recomendacao deste guia passa a ser uma struct generica desde o inicio.

```rust
// Variante generica alinhada com a filosofia usada no AM2302.
pub struct TimPulseInput<'d, T, P> {
    // Pino que recebe o ECHO em funcao alternativa do timer.
    pin: Peri<'d, P>,
    // Timer responsavel pela captura temporal.
    timer: Peri<'d, T>,
}
```

Forma recomendada para os bounds:

```rust
// T = timer de 4 canais, como o TIM3.
// P = pino compativel com o canal 2 desse timer.
pub struct TimPulseInput<'d, T, P>
where
    T: GeneralInstance4Channel,
    P: TimerPin<T, Ch2>,
{
    pin: Peri<'d, P>,
    timer: Peri<'d, T>,
}
```

Por que fixar `Ch2` agora:

- a bancada atual esta fechada em `PC7 / TIM3_CH2`
- isso ainda deixa o timer e o pino genericos
- evita generalizar tambem o canal cedo demais
- segue a linha do `AM2302`, mas com complexidade menor

---

## 7. API recomendada para o construtor

Como o hardware ja foi definido, o passo seguinte deve nascer com um construtor claro.

Exemplo de forma esperada:

```rust
impl<'d, T, P> TimPulseInput<'d, T, P>
where
    T: GeneralInstance4Channel,
    P: TimerPin<T, Ch2>,
{
    // O construtor recebe os recursos concretos da bancada, mas a struct
    // continua reutilizavel para outro timer/pino compativel no futuro.
    pub fn new(pin: Peri<'d, P>, timer: Peri<'d, T>) -> Self {
        Self {
            pin,
            timer,
        }
    }
}
```

Neste passo 3, o construtor ainda pode ficar apenas planejado no documento.

---

## 8. Contrato funcional que a implementacao futura deve obedecer

Quando `measure_high_pulse(timeout)` for chamada, a implementacao deve fazer isso:

1. esperar a borda de subida do `ECHO`
2. registrar o timestamp da subida
3. esperar a borda de descida do `ECHO`
4. registrar o timestamp da descida
5. calcular `descida - subida`
6. devolver esse tempo como `Duration`

Se qualquer etapa falhar dentro do `timeout`, devolver erro.

---

## 9. Como esse passo conversa com o passo 4

O passo 4 sera a implementacao da trait para STM32G4 usando `timer input capture` sobre a struct generica.

Com a decisao atual de hardware, isso significa:

- usar `TIM3`
- usar `CH2`
- usar o pino `PC7` (`D9`) como entrada de captura
- configurar uma base temporal simples, preferencialmente `1 MHz`

Com isso, a regra fica facil:

- `1 tick = 1 us`

---

## 10. Esqueleto de codigo sugerido para este passo

Este e um bom ponto de partida para o arquivo `src/drivers/hcsr04.rs`:

```rust
use embassy_time::Duration;
use defmt::Format;

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

use embassy_stm32::Peri;
use embassy_stm32::timer::input_capture::Ch2;
use embassy_stm32::timer::{GeneralInstance4Channel, TimerPin};

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

impl<'d, T, P> TimPulseInput<'d, T, P>
where
    T: GeneralInstance4Channel,
    P: TimerPin<T, Ch2>,
{
    // Construtor minimo; a configuracao real entra no passo 4.
    pub fn new(pin: Peri<'d, P>, timer: Peri<'d, T>) -> Self {
        Self { pin, timer }
    }
}
```

Esse esqueleto e pequeno, mas ja fecha a interface que o resto da fase vai consumir.

---

## 11. Criterio de conclusao do passo 3

Considere o passo 3 concluido quando existir:

- a trait `PulseInput`
- um enum de erro inicial para medicao de pulso
- a decisao explicita de que a primeira implementacao usara uma struct generica no estilo do `AM2302`
- a decisao explicita de que a primeira combinacao validada sera `PC7 / TIM3_CH2`
- o arquivo preparado para receber a implementacao do passo 4

Ainda nao e necessario neste passo:

- gerar `TRIG`
- capturar bordas de verdade
- calcular distancia
- integrar ao `main`
- integrar ao comando `sensors`

---

## 12. Decisao final deste passo

Para a Fase 6 seguir de forma limpa, a recomendacao e:

- criar `src/drivers/hcsr04.rs`
- definir nele a trait `PulseInput`
- usar `Duration` como tipo de retorno da largura do pulso
- preparar uma struct generica `TimPulseInput<'d, T, P>`
- fixar por enquanto o canal em `Ch2`
- validar primeiro a combinacao `PC7 / D9` com `TIM3_CH2`

Esse e o menor passo correto antes de partir para a captura real do `ECHO`.
