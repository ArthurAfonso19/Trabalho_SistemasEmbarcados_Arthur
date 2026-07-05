# Plano De Trabalho

Este documento e o plano de execucao do trabalho, refinado apos analise do codigo fonte, dos datasheets (STM32G474RE, RM0440, AM2302, HC-SR04), do driver ADXL345 de referencia e da documentacao oficial do ecossistema Embassy.

O plano esta organizado em fases sequenciais. Cada fase tem objetivo, passos concretos, entregaveis esperados, observacoes tecnicas e um ciclo de desenvolvimento e teste.

Estado atual do plano:

- Fase 1 concluida
- Proxima fase sugerida: Fase 2

---

## 1. Objetivo Geral

Desenvolver, em Rust, um firmware embarcado para STM32G474RE que atenda aos topicos obrigatorios do professor e implemente uma arquitetura de drivers genericos e agnosticos de RTOS.

O escopo total inclui:

- shell/console serial com comandos de introspeccao
- tarefa de LED como referencia temporal
- tarefa de botao acionada por interrupcao
- metricas primarias do sistema: tarefas instaladas, runtime, heap
- comando de tarefas de tempo real
- drivers primarios genericos
- drivers secundarios para sensores
- sensores AM2302 (temp/umidade) e HC-SR04 (distancia)
- aproveitamento do ADXL345 ja existente como referencia arquitetural

---

## 2. Estado Atual Do Projeto

### Situacao da Fase 1

A Fase 1 pode ser considerada concluida porque:

- `cargo check` ja passa
- `cargo run` ja foi validado no hardware
- o firmware atual continua funcional na placa
- o `src/main.rs` foi reorganizado por papel:
  - infraestrutura
  - setup
  - runtime
  - `main`
- a `main()` ficou mais enxuta e orientada a orquestracao

### Ja implementado (baseline do professor)

| Componente | Localizacao | Tecnologia |
|---|---|---|
| Blink LED `PA5` | `src/main.rs` | GPIO com `Timer::after_millis` |
| Botao EXTI `PC13` | `src/main.rs` | `ExtiInput` com `wait_for_rising_edge/wait_for_falling_edge` |
| Eco serial `LPUART1` | `src/main.rs` | `Uart` async com buffer de 1 byte |
| Leitura ADC `PA7` | `src/main.rs` | `Adc::blocking_read` |
| Sensor temperatura interna | `src/main.rs` | `enable_temperature` no ADC1 |
| PWM `TIM1` em `PC0` | `src/main.rs` | `SimplePwm` com duty cycle variavel |
| I2C1 com ADXL345 | `src/main.rs` | `I2c` async + `adxl345-async` driver |
| `bind_interrupts!` | `src/main.rs` | LPUART, DMA, I2C, EXTI |
| Clock 170 MHz | `src/main.rs` | HSE 24 MHz + PLL |
| `#[pre_init]` | `src/main.rs` | Copia `.ccmdata` e `.data2` |
| `irqs` | `src/main.rs` | Estrutura de interrupcoes |

### Ainda nao implementado

- shell interativo com parser de comandos
- comandos `help`, `tasks`, `rt`, `mem`, `uptime`, `sensors`, `led`, `status`
- metricas de tarefas (contador, timestamp, jitter)
- heap estatico ou monitoramento de memoria
- drivers primarios genericos desacoplados do Embassy
- drivers secundarios AM2302 e HC-SR04
- arquitetura agnostica de RTOS

### Fluxo de trabalho adotado

Para as proximas fases, o fluxo padrao sera:

1. Implementar uma parte pequena da fase
2. Rodar `cargo check`
3. Se compilar, rodar `cargo run`
4. Testar o comportamento real na placa
5. Registrar o que ficou validado antes de seguir

Isso reduz risco e evita acumular muitas mudancas sem validacao intermediaria.

---

## 3. Materiais Disponiveis

Apos a etapa de levantamento, estes materiais estao acessiveis e serao usados ativamente no desenvolvimento:

### Documentacao oficial STM32G4

| Documento | Conteudo principal |
|---|---|
| `stm32g474re.pdf` | Pinagem, caracteristicas eletricas, limites de tensao, encapsulamento |
| `rm0440...pdf` | Reference manual: perifericos, registradores, EXTI, timers, GPIO, I2C, UART, ADC |

### Datasheets dos sensores

| Documento | Sensor | Protocolo |
|---|---|---|
| `AM2302_copia.pdf` | Temperatura e umidade | One-wire proprietario (GPIO + timing) |
| `HC-SR04 (1).PDF` | Ultrassonico | Trigger + Echo (GPIO + medicao de pulso) |

### Driver de referencia

| Recurso | Uso |
|---|---|
| `adxl345-async` crate | Modelo arquitetural para drivers genericos |
| `AsyncBus` trait | Exemplo de abstracao de barramento |
| `I2cBus` / `SpiBus` | Exemplo de adaptadores concretos |
| `Adxl345Async<XBUS>` | Exemplo de driver secundario generico |

### Links de documentacao

Organizados no arquivo `Documents/Comandos para Documentacoes.txt`:
- Rust: Book, Embedded Book, Async Book, Reference, Nomicon
- Embassy: site, book, API docs (executor, stm32, time, sync, futures)
- Traits: embedded-hal, embedded-hal-async, embedded-io, embedded-io-async
- Debug: defmt, defmt-rtt, panic-probe, probe-rs

---

## 4. Decisoes Tecnicas Preliminares

### 4.1. Manter Embassy como framework base

O projeto atual ja usa Embassy e `cargo check` confirma que compila. Nao faz sentido migrar para FreeRTOS.

### 4.2. Sem heap dinamico

Embassy nao exige heap. Para atender o requisito de "heap livre", podemos:

- expor o valor estatico disponivel
- justificar a ausencia de alocacao dinamica como escolha arquitetural
- se necessario, medir uso de memoria estatica via simbolos do linker

### 4.3. Shell sobre UART existente

A UART ja esta funcionando com eco simples. Vamos mante-la e adicionar:

- buffer de linha
- parser de comandos
- resposta sem bloqueio

### 4.4. Arquitetura de drivers inspirada no `adxl345-async`

O driver do professor usa este padrao:

```
trait AsyncBus { read_reg, write_reg, read_multiple, write_multiple }
struct I2cBus<I2C: I2c>  (impl AsyncBus)
struct SpiBus<SPI: SpiDevice> (impl AsyncBus)
struct Adxl345Async<XBUS: AsyncBus> { bus: XBUS }
```

Vamos replicar essa arquitetura para os novos sensores.

### 4.5. Necessidade de GPIO + timing como "drivers primarios"

Diferente do ADXL345 (puramente I2C/SPI), AM2302 e HC-SR04 dependem de:

- GPIO digital com temporizacao precisa
- medicao de largura de pulso

Isso significa que nossa camada de drivers primarios precisa incluir:
- `OutputPin` / `InputPin` (ja coberto por `embedded-hal`)
- `Delay` de microssegundos
- `PulseInput` (trait propria para medir pulso)

### 4.6. Cuidado eletrico com HC-SR04

Segundo o datasheet do STM32G474RE:

- GPIO sao tolerantes a 5 V apenas em pinos especificos
- pino `ECHO` do HC-SR04 normalmente opera em 5 V
- sera necessario:
  - consultar `stm32g474re.pdf` para pinos 5V-tolerant
  - ou usar divisor resistivo
  - ou usar level shifter

Isso sera resolvido na fase de mapeamento de pinos.

---

## 5. Arquitetura Proposta

### 5.1. Estrutura de modulos

```
src/
  main.rs               # Inicializacao, config, spawn de tarefas
  app/
    mod.rs               # Orquestracao da aplicacao
    shell.rs             # Console UART: parser, comandos, dispatch
    monitor.rs           # Coleta de metricas: tarefas, tempo, memoria
    led_task.rs          # Tarefa de LED refatorada
    button_task.rs       # Tarefa de botao refatorada
  drivers/
    mod.rs               # Re-export e traits publicas
    bus/                 # Traits de barramento (ex: AsyncBus)
      mod.rs
    gpio/                # Traits para GPIO + timing (se necessario)
      mod.rs
    adxl345/             # Envoltorio ou reimplementacao
      mod.rs
    am2302/              # Driver do sensor de temperatura/umidade
      mod.rs
    hcsr04/              # Driver do sensor ultrassonico
      mod.rs
```

Se a quantidade de codigo por driver for pequena, podemos simplificar:

```
src/
  main.rs
  app/
    mod.rs
    shell.rs
    monitor.rs
  drivers/
    mod.rs
    adxl345.rs
    am2302.rs
    hcsr04.rs
```

### 5.2. Camadas

```
APLICACAO (app/)
  |
  v
DRIVERS SECUNDARIOS (drivers/)
  - Adxl345Async<XBUS: AsyncBus>
  - Am2302<PIN: OutputPin + InputPin, D: DelayUs>
  - HcSr04<TRIG: OutputPin, ECHO: InputPin + PulseMeasure>
  |
  v
DRIVERS PRIMARIOS (trait definitions + adapters)
  - AsyncBus (trait) -> I2cBus / SpiBus
  - PulseInput (trait) -> impl com timer do STM32
  - DelayUs (trait, de embedded-hal)
  - OutputPin / InputPin (de embedded-hal)
  |
  v
HAL CONCRETO (embassy-stm32, cortex-m)
```

### 5.3. Trait `PulseInput`

Para o HC-SR04, precisamos medir a duracao de um pulso no pino `ECHO`. A experiencia com o `AM2302` mostrou que, quando o protocolo depende de janelas curtas e previsibilidade temporal, a melhor arquitetura e tirar a CPU do caminho critico e delegar a medicao ao hardware.

Para o STM32G4, isso leva a esta ordem de preferencia:

- Opcao A: usar timer em modo input capture
- Opcao B: usar timer em input capture com apoio de DMA, se a janela de captura justificar
- Opcao C: usar EXTI + timestamp do `embassy-time` apenas como prototipo ou fallback
- Opcao D: usar `InputPin` + loop de polling com `RMW` (nao recomendado)

Trait sugerida:

```rust
trait PulseInput {
    type Error;
    async fn measure_high_pulse(&mut self, timeout: Duration) -> Result<Duration, Self::Error>;
}
```

Adapter concreto: usar um timer qualquer (ex: TIM2 ou TIM3) em modo de captura.

Para o HC-SR04, como normalmente ha um unico pulso `HIGH` por medicao, pode nao ser necessario um buffer DMA grande como no `AM2302`. Ainda assim, a linha de arquitetura deve continuar a mesma:

- `GPIO` para o trigger
- timer para capturar as bordas do `ECHO`
- CPU apenas para converter a largura do pulso em distancia

Isso precisa ser validado com o `rm0440.pdf` para ver qual timer esta livre.

---

## 6. Shell / Console

### 6.1. Funcionamento

Baseado na UART async existente:

- buffer circular de entrada (ex: 64 bytes)
- detecta `\n` ou `\r\n`
- interpreta comando + argumentos
- executa callback correspondente
- resposta via `Uart::write`

### 6.2. Estrutura de dados

```rust
struct Command {
    name: &'static str,
    help: &'static str,
    handler: fn(args: &[&str], uart: &mut Uart),
}
```

Ou, para manter agnosticismo sem depender do tipo Uart concreto:

```rust
trait CommandHandler {
    fn name(&self) -> &'static str;
    fn help(&self) -> &'static str;
    fn execute(&self, args: &[&str], out: &mut dyn Write);
}
```

### 6.3. Tabela de comandos

| Comando | Funcao |
|---|---|
| `help` | Lista todos os comandos |
| `status` | Estado geral do sistema |
| `tasks` | Lista tarefas e metricas |
| `rt` | Informacoes de tempo real |
| `mem` | Uso de memoria |
| `uptime` | Tempo desde o boot |
| `sensors` | Leitura de sensores |
| `led on/off` | Controle manual do LED |
| `led period <ms>` | Configura periodo do blink |
| `mode` | Exibe modo atual do sistema |

### 6.4. Nao intrusivo

- saida via `Uart::write` async, sem bloqueio
- escrita limitada a N bytes por ativacao para evitar monopolizar CPU
- envio maior pode ser chunked

---

## 7. Monitoramento De Tarefas

### 7.1. O que Embassy ja oferece

O `embassy-executor` nao expoe nativamente as metricas que o professor pede.

Temos que construir nossa propria camada.

### 7.2. Estrutura de monitoramento

```rust
struct TaskMetrics {
    name: &'static str,
    run_count: AtomicU32,
    last_run: AtomicU64,
    min_interval: AtomicU64,
    max_interval: AtomicU64,
}
```

Cada tarefa periodica registra:

- incrementa contador
- armazena timestamp de execucao
- calcula intervalo desde a ultima execucao

### 7.3. Uptime do sistema

Usar `embassy_time::Instant` no boot como referencia.

Comando `uptime`:

```rust
let elapsed = Instant::now() - BOOT_TIME;
```

### 7.4. Memoria

- nao ha heap no projeto atual
- podemos expor:
  - tamanho da secao `.bss` + `.data` via simbolos do linker
  - espaco livre em `SRAM2` e `CCMRAM`, se relevante
- a documentacao do `memory.x` permite isso

---

## 8. Tarefa De LED

### 8.1. Situacao atual

O LED `PA5` pisca a cada 500 ms dentro de um `loop {}` no final da `main`.

Isso e funcional, mas nao e uma tarefa nomeada nem monitorada.

### 8.2. Melhorias previstas

- extrair para uma funcao separada com `#[embassy_executor::task]`
- receber periodo como parametro externo
- registrar metricas de execucao
- aceitar comando `led on/off` para pausar/retomar
- aceitar comando `led period <ms>` para alterar periodo

### 8.3. Interface proposta

```rust
#[embassy_executor::task]
async fn led_task(control: &'static LedControl) {
    loop {
        if control.enabled() {
            led.set_high();
            Timer::after_millis(control.period_ms() as u64 / 2).await;
            led.set_low();
            Timer::after_millis(control.period_ms() as u64 / 2).await;
        } else {
            led.set_low();
            Timer::after_millis(100).await;
        }
    }
}
```

---

## 9. Tarefa De Botao

### 9.1. Situacao atual

O botao `PC13` usa EXTI e detecta borda de subida/descida.

Ja atende ao requisito de "sem polling".

### 9.2. Melhorias previstas

- associar acao ao clique:
  - alternar LED on/off
  - alternar modo de operacao entre "normal" e "diagnostico"
- registrar evento para o shell logar

### 9.3. Decisao

Inicialmente: alternar o estado do LED entre ligado e desligado.

Em uma segunda versao: alternar modo de operacao.

---

## 10. Drivers Genericos

### 10.1. Inspiracao no `adxl345-async`

O driver de referencia tem esta estrutura:

```rust
pub trait AsyncBus {
    type Error;
    async fn read_reg(&mut self, reg: u8) -> Result<u8, Self::Error>;
    async fn write_reg(&mut self, reg: u8, val: u8) -> Result<(), Self::Error>;
    async fn read_multiple(&mut self, reg: u8, buf: &mut [u8]) -> Result<(), Self::Error>;
    async fn write_multiple(&mut self, reg: u8, bytes: &[u8]) -> Result<(), Self::Error>;
}

pub struct Adxl345Async<XBUS: AsyncBus> { bus: XBUS }

impl<XBUS: AsyncBus> Adxl345Async<XBUS> {
    pub fn new(bus: XBUS) -> Self;
    pub async fn setup(&mut self) -> Result<(), XBUS::Error>;
    pub async fn get_accel(&mut self) -> Result<(f32, f32, f32), XBUS::Error>;
    // etc
}
```

Vamos usar exatamente esse padrao para AM2302 e HC-SR04.

### 10.2. Driver do AM2302

#### Protocolo (do datasheet)

1. MCU envia start: puxa linha baixo por >= 1 ms
2. MCU libera linha e espera resposta
3. Sensor responde com:
   - 80 us low
   - 80 us high
4. Sensor envia 40 bits:
   - cada bit começa com 50 us low
   - bit `0`: 26-28 us high
   - bit `1`: 70 us high
5. Paridade: soma dos primeiros 4 bytes deve ser igual ao 5o byte

#### Dependencias

- `OutputPin` (para start)
- `InputPin` (para leitura de resposta e bits)
- `DelayUs` (para temporizacao de microssegundos)

#### Interface proposta

```rust
pub struct Am2302<PIN: OutputPin + InputPin, D: DelayUs> {
    pin: PIN,
    delay: D,
}

impl<PIN: OutputPin + InputPin, D: DelayUs> Am2302<PIN, D> {
    pub fn new(pin: PIN, delay: D) -> Self;
    pub async fn read(&mut self) -> Result<(f32, f32), Am2302Error>;
}
```

Onde `Am2302Error` cobre:
- timeout sem resposta
- erro de paridade
- erro de temporizacao

#### Observacao tecnica importante

O AM2302 exige temporizacao da ordem de microssegundos:

- `DelayUs` de `embedded-hal` funciona, mas em ambiente async com Embassy, `Timer::after_micros` nao existe
- precisaremos de:
  - `blocking_delay_us` via `cortex_m::asm::delay` ou
  - loop com `asm::nop` calibrado

Isso significa que a leitura do AM2302 **precisa ser bloqueante no momento da leitura** e executada em sua propria tarefa, para nao travar as demais.

Isso e aceitavel, desde que a leitura seja rapida (< 5 ms por ciclo completo).

#### Formato de saida

- Temperatura: graus Celsius
- Umidade: porcentagem

Ambas como `f32`.

### 10.3. Driver do HC-SR04

#### Funcionamento (do datasheet)

1. MCU envia trigger: pino `TRIG` alto por 10 us
2. Sensor emite 8 pulsos ultrasonicos a 40 kHz
3. Sensor coloca `ECHO` em nivel alto
4. Sensor mantem `ECHO` alto por tempo proporcional a distancia
5. MCU mede largura do pulso em `ECHO`
6. Distancia = (tempo_us * 0.034) / 2

Ou de forma simplificada: distancia_cm = tempo_us / 58.

#### Dependencias

- `OutputPin` (para TRIG)
- `InputPin` + `PulseInput` (para medir pulso em ECHO)

#### Interface proposta

```rust
pub struct HcSr04<TRIG: OutputPin, ECHO: PulseInput> {
    trig: TRIG,
    echo: ECHO,
}

impl<TRIG: OutputPin, ECHO: PulseInput> HcSr04<TRIG, ECHO> {
    pub fn new(trig: TRIG, echo: ECHO) -> Self;
    pub async fn measure_distance_cm(&mut self) -> Result<f32, HcSr04Error>;
}
```

#### Sobre `PulseInput`

Trait que ainda nao existe e precisamos definir:

```rust
pub trait PulseInput {
    type Error;
    /// Mede a duracao de um pulso em nivel alto no pino.
    /// Retorna timeout se o pulso exceder `timeout`.
    async fn measure_high_pulse(&mut self, timeout: Duration) -> Result<Duration, Self::Error>;
}
```

Implementacao concreta para STM32G4:

- **Opcao principal**: usar um timer em input capture
  - vantagem: medicao por hardware, menor jitter e melhor convivencia com outras tasks
  - vantagem: preserva a escalabilidade da arquitetura para varios sensores sensiveis a tempo
  - desvantagem: consome um timer e precisa de mapeamento correto de canal/pino
  - precisamos verificar no RM0440 qual timer esta livre (TIM2, TIM3, TIM15, TIM16, etc.)

- **Opcao secundaria**: usar timer em input capture com DMA
  - vantagem: util se quisermos capturar mais de uma borda sem rearme manual ou montar uma interface primaria mais geral
  - desvantagem: aumenta a complexidade e o consumo de recursos

- **Opcao de prototipo apenas**: usar EXTI + `embassy_time::Instant`
  - vantagem: implementacao inicial mais simples
  - desvantagem: depende mais da CPU e da resolucao temporal do sistema
  - desvantagem: pior previsibilidade em ambiente concorrente

Depois do aprendizado com o `AM2302`, a recomendacao do plano deixa de ser `EXTI + Instant` e passa a ser **timer input capture como estrategia padrao** para qualquer sensor com timing relevante.

#### Resultado

```rust
struct EchoInput<'a, P: InputPin> {
    pin: P,
    exti: ExtiInput<'a, P::Interrupt>,
}
```

#### Atencao eletrica

- Verificar no `stm32g474re.pdf` quais pinos sao 5V-tolerant
- `ECHO` do HC-SR04 normalmente emite 5 V
- Se nao houver pino 5V-tolerant disponivel, usar divisor resistivo

### 10.4. Driver do ADXL345 como referencia

O driver atual ja esta funcional com `I2cBus`. Se houver tempo, podemos:

- mante-lo como esta
- ou reimplementa-lo usando nossa propria trait de barramento
- ou simplesmente referencia-lo como exemplo de arquitetura

Para o trabalho, manter como esta e suficiente.

---

## 11. Pinos Planejados

### 11.1. Pinos ja em uso

Com base em `src/main.rs`:

| Pino | Funcao | Periferico |
|---|---|---|
| `PA5` | LED | GPIO Output |
| `PA7` | Entrada analogica | ADC2 |
| `PA3` | RX | LPUART1 |
| `PA2` | TX | LPUART1 |
| `PC13` | Botaozinho | EXTI (PullDown) |
| `PC0` | PWM | TIM1 CH1 |
| `PB8` | SCL | I2C1 |
| `PB9` | SDA | I2C1 |
| `PC9` | CS ADXL345 | GPIO Output (nao usado no driver atual) |

Total: 9 pinos ocupados.

### 11.2. Pinos disponiveis (estimativa)

Depende do encapsulamento (LQFP64, LQFP48, etc.), mas a placa STM32G474RE Nucleo tem:

- varios pinos livres nos headers CN5, CN6, CN7, CN8, CN9, CN10
- pelo menos 2-3 GPIO livres em cada porta (A, B, C, D)

### 11.3. Sugestao para AM2302

- Um pino GPIO digital bidirecional
- Sugestao: qualquer pino livre, por exemplo `PA0`, `PA1`, `PB0`, `PB10`, `PC1`
- alimentacao: 3.3 V (o modulo AM2302 normalmente aceita 3.3 V a 5.5 V)
- conectar DATA ao pino escolhido + pull-up de 4.7 k-10 k

### 11.4. Sugestao para HC-SR04

- `TRIG`: qualquer pino GPIO de saida, ex: `PA6`, `PB1`, `PC2`
- `ECHO`: pino GPIO de entrada com EXTI, e preferencialmente 5V-tolerant
- alimentacao: 5 V (externo ou pino 5 V da placa)
- `ECHO` precisa de atencao eletrica: verificar 5V-tolerant ou usar divisor

### 11.5. Confirmacao necessaria

Esta secao precisa ser revisada com a placa real em maos e o `stm32g474re.pdf` para confirmar:
- encapsulamento exato
- pinos 5V-tolerant
- conflitos com perifericos ja em uso

---

## 12. Fases De Execucao

### Regra geral para todas as fases

Cada fase deve seguir o mesmo ciclo curto:

1. Implementar apenas o escopo da fase atual
2. Validar compilacao com `cargo check`
3. Validar gravacao e execucao com `cargo run`
4. Testar no hardware o comportamento esperado da fase
5. So depois avancar para a fase seguinte

Quando uma fase for grande, quebrar em subetapas pequenas e repetir esse mesmo ciclo.

### Fase 1 - Baseline e Modularizacao

**Status**: concluida

**Objetivo**: compilar, executar e organizar o codigo existente.

**Passos**:
1. Executar `cargo check` e confirmar compilacao
2. Executar `cargo run` e validar o firmware no hardware
3. Reorganizar o `src/main.rs` sem alterar o comportamento existente
4. Separar o arquivo por infraestrutura, setup, runtime e `main`
5. Validar novamente com `cargo check` e teste na placa

**Entregaveis**:
- Projeto compilando
- Firmware validado no hardware
- `main.rs` reorganizado e mais legivel
- `main()` enxuta, focada em orquestracao

**Observacao**: a fase foi concluida sem modularizacao em varios arquivos. Neste momento, a reorganizacao interna do `main.rs` ja atende bem ao objetivo de baseline e organizacao.

---

### Fase 2 - Shell UART

**Objetivo**: substituir o eco serial por um console com comandos.

**Ciclo de validacao da fase**:

1. Implementar primeiro apenas `help`
2. Rodar `cargo check`
3. Rodar `cargo run`
4. Testar digitacao real pela UART
5. So depois adicionar `status`

**Passos**:
1. Em `app/shell.rs`, criar:
   - buffer de entrada circular de 64 bytes
   - estado de recepcao (aguardando, linha completa)
   - parser que separa comando e argumentos
2. Tabela de comandos:
   - `help` -> lista comandos
   - `status` -> breve status do sistema
3. Integrar com `main.rs`:
   - spawnar `shell_task` recebendo a UART
   - conectar leitura/escrita
4. Garantir que a escrita nao seja intrusiva:
   - usar `uart.write()` async
   - evitar prints excessivos

**Entregaveis**:
- `help` funcional
- `status` funcional
- shell minimo operacional

---

### Fase 3 - Monitoramento De Tarefas

**Objetivo**: criar estrutura para coletar metricas e expor via shell.

**Ciclo de validacao da fase**:

1. Implementar a estrutura de metricas
2. Validar compilacao
3. Instrumentar uma tarefa de cada vez
4. Testar o comando `tasks` antes de adicionar `mem` e `uptime`

**Passos**:
1. Em `app/monitor.rs`, criar `TaskMetrics`:
   - nome
   - contador
   - timestamp da ultima execucao
   - intervalo minimo/maximo
   - `mark_execution()` para registrar
2. Criar `SystemMonitor` como container de metricas
3. Adicionar marcacao em `led_task`, `button_task`, `adc_task`
4. Comandos novos:
   - `tasks` -> lista tarefas com metricas
   - `uptime` -> tempo desde o boot
5. Comando `mem`:
   - expor tamanho das secoes `.bss`, `.data`, `.text`
   - via simbolos do linker (consultar `memory.x`)

**Entregaveis**:
- `tasks`, `uptime`, `mem` funcionando

---

### Fase 4 - Tarefa De LED Refatorada

**Objetivo**: transformar LED em tarefa de sistema monitoravel e controlavel.

**Ciclo de validacao da fase**:

1. Extrair o LED para task propria
2. Confirmar que o blink continua funcionando
3. Adicionar controle on/off
4. Testar via shell
5. Adicionar ajuste de periodo por ultimo

**Passos**:
1. Extrair loop do LED para `app/led_task.rs`
2. Criar `LedControl`:
   - `enabled: bool`
   - `period_ms: u32`
   - metodos `toggle`, `set_period`, `is_enabled`
3. Spawnar `led_task` com `LedControl`
4. Comandos:
   - `led on` / `led off`
   - `led period <ms>`

**Entregaveis**:
- LED como tarefa formal
- Controle via shell
- Metricas registradas

---

### Fase 5 - Driver AM2302

**Objetivo**: implementar driver generico para o sensor de temperatura e umidade.

**Ciclo de validacao da fase**:

1. Implementar o driver isoladamente
2. Validar leitura minima em uma tarefa simples
3. Testar checksum e timeout no hardware
4. So depois integrar ao comando `sensors`

**Passos**:
1. Estudar datasheet para confirmar:
   - sequencia de start
   - formato da resposta
   - checksum
2. Em `src/drivers/am2302.rs`:
   - `Am2302Error` enum
   - `Am2302<PIN: OutputPin + InputPin, D: DelayUs>` struct
   - `new(pin, delay)`
   - `read() -> Result<(f32, f32), Am2302Error>`
3. Implementar:
   - start: pino baixo por 1-2 ms
   - esperar resposta do sensor
   - ler 40 bits com temporizacao
   - validar checksum
   - converter para temperatura e umidade
4. Integrar no `main.rs`:
   - escolher pino
   - instanciar driver
   - spawnar tarefa de leitura periodica
5. Comando `sensors` no shell exibe valores

**Observacoes tecnicas**:
- a leitura e bloqueante no microssegundo, entao `Timer::after_micros` nao funciona
- usar `cortex_m::asm::delay()` ou loop calibrado com `DWT` para temporizacao fina
- manter a leitura em uma tarefa separada, com sleep de 2-5 s entre medicoes
- tratar timeout de resposta (sensor ausente, fio solto)

**Entregaveis**:
- `Am2302` struct com trait generica
- Tarefa de leitura periodica
- Exibicao por `sensors`

---

### Fase 6 - Driver HC-SR04

**Objetivo**: implementar o driver do sensor ultrassonico de forma coerente com as licoes aprendidas no `AM2302`, priorizando escalabilidade, sincronismo e isolamento das tarefas de tempo real.

**Principio arquitetural da fase**:

- o `HC-SR04` nao deve nascer como um driver baseado em polling fino nem em medicao dominada pela CPU
- o caminho critico de tempo deve ficar preferencialmente no hardware do microcontrolador
- a shell nao deve disparar medicoes diretamente; deve ler apenas um snapshot compartilhado
- a integracao precisa preservar a convivencia com as demais tasks do sistema

**Ciclo de validacao da fase**:

1. Confirmar primeiro a compatibilidade eletrica do `ECHO`
2. Validar o mapeamento de pino, canal de timer e recurso de captura
3. Testar trigger + captura do `ECHO` sem shell e sem outras abstracoes extras
4. So depois integrar a tarefa periodica e o estado compartilhado
5. Por ultimo, expor a distancia no comando `sensors`

**Passos**:
1. Confirmar a parte eletrica:
   - escolher um pino seguro para `ECHO`
   - verificar se e `5V-tolerant`
   - se nao for, prever divisor resistivo ou level shifter
2. Escolher os recursos de hardware:
   - pino `TRIG` como GPIO output comum
   - pino `ECHO` ligado a um canal de timer com input capture
   - validar qual timer e canal estao livres no firmware atual
3. Definir a abstracao primaria para medicao de pulso:
   ```rust
    pub trait PulseInput {
        type Error;
        async fn measure_high_pulse(&mut self, timeout: Duration) -> Result<Duration, Self::Error>;
    }
    ```
   - seguir a mesma filosofia do `AM2302`: implementacao generica sobre timer e pino
   - primeira versao validada em bancada pode fixar o canal em `TIM3_CH2`
4. Implementar `PulseInput` para STM32G4 com **timer input capture como caminho principal**:
   - configurar o timer em base temporal simples, por exemplo `1 MHz`
   - capturar pelo menos a borda de subida e a borda de descida do `ECHO`
   - calcular a largura do pulso a partir da diferenca entre timestamps
   - usar `DMA` apenas se isso simplificar a captura ou aumentar a robustez da interface primaria
5. Em `src/drivers/hcsr04.rs`, implementar o driver secundario:
   - `HcSr04Error` enum
   - `HcSr04<TRIG: OutputPin, ECHO: PulseInput>` struct
   - `new(trig, echo)`
   - `measure_distance_cm() -> Result<f32, HcSr04Error>`
6. Implementar a logica de medicao:
   - gerar trigger: pino alto por `10 us`
   - medir o pulso `HIGH` do `ECHO`
   - converter tempo para distancia
   - tratar timeout e ausencia de eco
7. Integrar ao firmware com a mesma filosofia usada no `AM2302`:
   - criar uma task periodica propria do `HC-SR04`
   - atualizar um snapshot compartilhado com ultima distancia valida e ultimo erro
   - manter a shell apenas como leitora desse estado
8. So depois integrar a exibicao no comando `sensors`

**Observacoes tecnicas**:
- timeout padrao: `30-40 ms` para cobrir distancia maxima e falha sem eco
- evitar `polling` em loop apertado no pino `ECHO`
- evitar tratar `EXTI + Instant` como solucao final de arquitetura; se for usado, deve ser apenas para experimento inicial ou prova rapida de conceito
- preferir resolucao temporal simples e interpretavel, como timer em `1 MHz`, para que `1 tick = 1 us`
- manter a medicao em task separada, com intervalo entre amostras, para nao congestionar o barramento acustico nem a CPU
- a etapa de decodificacao/conversao deve acontecer depois da captura, fora da janela temporal critica

**Entregaveis**:
- `PulseInput` trait definida com implementacao baseada em timer input capture
- `HcSr04` struct com trait generica
- tarefa periodica de medicao desacoplada da shell
- snapshot compartilhado com ultima distancia e ultimo erro
- exibicao por `sensors`

---

### Fase 7 - Comando RT (Tempo Real)

**Objetivo**: criar comando `rt` com informacoes de comportamento temporal.

**Ciclo de validacao da fase**:

1. Calcular metrica em uma unica tarefa de referencia
2. Validar saida do comando `rt`
3. Estender para outras tarefas somente depois da primeira validacao

**Passos**:
1. Em `app/monitor.rs`, estender `TaskMetrics`:
   - `intervalo_atual` vs `intervalo_esperado`
   - `jitter_estimado`
2. Calcular no `mark_execution()`:
   - diferenca entre `Instant::now()` e ultimo timestamp
   - comparar com intervalo esperado (se configurado)
   - acumular jitter medio
3. Comando `rt`:
   - periodicidade esperada do blink
   - ultima ativacao
   - jitter aproximado
   - contagem de execucoes por janela

**Entregaveis**:
- `rt` com metricas temporais
- LED blink usado como referencia de estabilidade

---

### Fase 8 - Integracao, Testes E Documentacao

**Objetivo**: finalizar o trabalho com verificacao e documentacao.

**Ciclo de validacao da fase**:

1. Testar cada subsistema separadamente
2. Testar o conjunto completo no hardware
3. Registrar limitacoes reais observadas
4. So entao consolidar a documentacao final

**Passos**:
1. Verificar que nenhuma tarefa faz polling:
   - LED: `Timer::after_millis`
   - Botao: `wait_for_rising_edge`
   - UART: `read` async
   - Sensores: `measure` async ou bloqueante rapido em tarefa separada
2. Verificar comportamento temporal:
   - LED blink com periodo viavel
   - shell responsivo
   - sensores lendo e exibindo corretamente
3. Documentar:
   - arquitetura de drivers
   - comandos do shell
   - escolhas eletricas (pinos, level shifter se houver)
   - decisoes de implementacao
4. Revisar aderencia aos topicos do professor

**Entregaveis**:
- firmware funcional e integrado
- documentacao tecnica

---

## 13. Cronograma Estimado

| Fase | Descricao | Esforco estimado |
|---|---|---|
| 1 | Baseline e modularizacao | 1 sessao |
| 2 | Shell UART | 1-2 sessoes |
| 3 | Monitoramento de tarefas | 1 sessao |
| 4 | LED refatorado | 1 sessao |
| 5 | Driver AM2302 | 2-3 sessoes |
| 6 | Driver HC-SR04 | 2-3 sessoes |
| 7 | Comando RT | 1 sessao |
| 8 | Integracao e documentacao | 1-2 sessoes |

Total estimado: 10-14 sessoes de desenvolvimento.

---

## 14. Riscos E Mitigacoes

| Risco | Impacto | Mitigacao |
|---|---|---|
| AM2302 com temporizacao imprecisa | Driver nao funciona | Usar `cortex_m::asm::delay()` calibrado; testar com osciloscopio se possivel |
| HC-SR04 ECHO em 5 V queima pino | Hardware danificado | Verificar pinos 5V-tolerant no datasheet; usar divisor resistivo |
| Shell UART monopoliza CPU | Outras tarefas atrasam | Escrita chunked; limitar bytes por ciclo |
| Embassy nao expoe metricas de tarefa | RTOS stats nao disponiveis | Construir nossa propria camada de monitoramento |
| `cargo doc` nao funciona por target cruzado | Documentacao offline nao gera | Usar `cargo doc --target thumbv7em-none-eabihf` ou versao web |
| Pinos insuficientes para novos sensores | Nao e possivel conectar todos | Remapear perifericos; priorizar sensores |

---

## 15. Duvidas Em Aberto

Estas duvidas precisam ser resolvidas com o professor ou com a analise do hardware real:

1. O que exatamente o professor espera como "informacoes de tarefas de tempo real"?
2. A ausencia de heap dinamico e aceitavel ou precisa ser implementada?
3. Havera placa disponivel para testes frequentes ou apenas no laboratorio?
4. O ADXL345 pode ser mantido no firmware final ou precisa ser substituido por um dos novos sensores?
5. Qual encapsulamento exato do STM32G474RE na placa utilizada?
6. Havera componentes externos (resistores, jumpers, protoboard) para ligar os sensores?
7. O professor fornece o `adxl345-async` crate em um repositorio separado?

---

## 16. Proximos Passos Imediatos

A ordem concreta a partir do estado atual:

1. Iniciar a Fase 2 com um shell minimo sobre a UART ja existente
2. Implementar primeiro apenas leitura de linha + comando `help`
3. Rodar `cargo check`
4. Rodar `cargo run`
5. Testar o comando no terminal serial real
6. Depois adicionar `status`

Se voce concordar com este plano refinado, o proximo passo natural e comecar a Fase 2 em incrementos pequenos e testaveis.
