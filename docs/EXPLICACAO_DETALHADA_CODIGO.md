# Explicação Detalhada da Implementação

## Índice

1. [Organização do `main.rs`](#1-organização-do-mainrs)
2. [Console Shell](#2-console-shell)
3. [Driver AM2302 (Temperatura e Umidade)](#3-driver-am2302)
4. [Driver HC-SR04 (Ultrassônico)](#4-driver-hc-sr04)

---

## 1. Organização do `main.rs`

O `main.rs` é o ponto de entrada do firmware e está dividido em **6 seções lógicas**, executadas em ordem sequencial dentro da função `main`.

### Estrutura geral

```
main()
├── 1. Configuração do chip (clock + periféricos)
├── 2. Criação dos drivers
├── 3. Logs de startup + self-checks
├── 4. Inicialização dos periféricos
├── 5. Spawn das tasks concorrentes
└── 6. Loop principal (mantém main viva)
```

### 1.1. Configuração do chip (linhas 555-561)

```rust
let config = stm32_config();
let p = embassy_stm32::init(config);
```

A função `stm32_config()` (linha 167) configura o clock do sistema:

| Parâmetro | Valor |
|---|---|
| Fonte HSE | 24 MHz (cristal externo da placa) |
| PLL pré-divisor | ÷6 → 4 MHz |
| PLL multiplicador | ×85 → 340 MHz |
| PLL pós-divisor | ÷2 → **170 MHz** |
| Barramentos AHB/APB1/APB2 | SEM divisão (DIV1) |

Isso significa que todo o sistema roda a 170 MHz, incluindo os timers usados pelos sensores. Esse valor é usado nos cálculos de delay dos drivers (ex: `170 * us` nos delays com `cortex_m::asm::delay`).

**Por que usar HSE + PLL em vez do oscilador interno?**

O HSE (cristal externo de 24 MHz) tem precisão muito maior que o oscilador RC interno. Isso é essencial para os sensores AM2302 e HC-SR04, que dependem de temporização na faixa de microssegundos.

### 1.2. Ativação do DWT (linhas 563-566)

```rust
let mut core = cortex_m::Peripherals::take().unwrap();
core.DCB.enable_trace();
core.DWT.enable_cycle_counter();
```

O **DWT (Data Watchpoint and Trace)** é um periférico do core Cortex-M que conta ciclos de CPU. Ativamos o contador de ciclos (`DWT_CYCCNT`) para debug e calibração — especialmente útil durante o desenvolvimento do driver AM2302 para verificar a temporização dos pulsos.

### 1.3. Criação dos drivers de sensores (linhas 568-576)

Dois drivers são instanciados antes dos self-checks:

**AM2302** — usa o trio **pino PB6 + TIM4 + DMA1_CH5**:
```rust
let am2302 = Am2302Capture::new(p.PB6, p.TIM4, p.DMA1_CH5);
```

**HC-SR04** — separa trigger (GPIO) e echo (timer input capture):
```rust
let trig = Output::new(p.PA6, Level::Low, Speed::Low);
let echo = TimPulseInput::new(p.PC7, p.TIM3);
let hcsr04 = HcSr04::new(trig, echo);
```

**Por que criar drivers antes dos self-checks?** Porque a inicialização consome os periféricos do PAC (Peripheral Access Crate) — depois de movidos para dentro do driver, não podem ser usados em outro lugar. É o padrão de ownership do Rust.

### 1.4. Self-checks de hardware (linhas 578-587)

```rust
log_startup();                    // Hello World + verificação de RAM
let (mut adc, mut adc_channel) = init_adc(p.ADC2, p.PA7);
log_initial_adc_sample(&mut adc, &mut adc_channel);  // leitura PA7
log_temperature_sample(p.ADC1);   // temperatura interna do chip
```

Essas funções validam que o hardware básico responde antes de entrar no runtime concorrente:
- `log_startup()` — exibe "Hello World!" e os valores das variáveis em `.ccmram` e `.data2`, confirmando que a rotina `#[pre_init]` copiou essas seções corretamente.
- `log_initial_adc_sample()` — faz uma leitura do pino PA7. Se o valor variar, o ADC está funcional.
- `log_temperature_sample()` — lê o sensor de temperatura interno do STM32G4. É opcional, mas confirma que o ADC1 também está operacional.

### 1.5. Inicialização dos periféricos (linhas 589-607)

Cada periférico ganha sua própria função de setup:

| Periférico | Função | Pinos | Detalhes |
|---|---|---|---|
| Botão | `init_button()` | PC13 + EXTI13 | Pull-Down interno, borda de subida = pressionado |
| UART | `init_uart()` | PA3 (RX), PA2 (TX) | LPUART1 a 115200 baud com DMA (CH1/CH2) |
| PWM | `init_pwm()` | PC0 + TIM1 | 10 kHz, push-pull, duty cycle variável |
| ADXL345 | `init_accelerometer()` | PB8 (SCL), PB9 (SDA), PC9 (CS) | I2C1 a 100 kHz com DMA, verifica WHOAMI |

**Destaque:** a `init_accelerometer()` precisa ser `.await` porque faz uma transação I2C durante a inicialização (leitura do registrador WHOAMI para confirmar que o ADXL345 responde).

### 1.6. Runtime concorrente — spawn das tasks (linhas 609-619)

```rust
spawner.spawn(unwrap!(adc_task(adc, adc_channel)));
spawner.spawn(unwrap!(button_task(button)));
spawner.spawn(unwrap!(shell_taks(lpuart1)));
spawner.spawn(unwrap!(pwm_task(pwm)));
spawner.spawn(unwrap!(accel_task(accel)));
spawner.spawn(unwrap!(led_task(p.PA5)));
spawner.spawn(unwrap!(am2302_task(am2302)));
spawner.spawn(unwrap!(hcsr04_task(hcsr04)));
```

Cada `spawner.spawn()` entrega a ownership do periférico para uma task assíncrona que roda **concorrentemente** com as demais. O executor do Embassy (um agendador cooperativo) gerencia a vez de cada task baseado em `.await` points.

**8 tasks no total**, cada uma com sua responsabilidade:

| Task | Periférico | O que faz | Intervalo |
|---|---|---|---|
| `adc_task` | ADC2 + PA7 | Lê valor analógico | 500 ms |
| `button_task` | PC13 + EXTI | Espera borda de subida/descida | Evento (interrupção) |
| `shell_taks` | LPUART1 | Console serial com comandos | Contínuo (byte a byte) |
| `pwm_task` | TIM1 + PC0 | Varre duty cycle do PWM | 300 ms cada passo |
| `accel_task` | I2C1 + ADXL345 | Lê aceleração | 1000 ms |
| `led_task` | PA5 | Pisca LED | Configurável (padrão 1000 ms) |
| `am2302_task` | PB6 + TIM4 + DMA | Lê temperatura/umidade | 2000 ms |
| `hcsr04_task` | PA6 + PC7 + TIM3 | Lê distância | 500 ms |

### 1.7. Loop principal (linhas 621-625)

```rust
loop {
    Timer::after_secs(1).await;
}
```

A `main` precisa ficar viva para que o executor continue rodando. Como todas as tasks já foram spawnadas e rodam independentemente, a `main` só precisa dormir para não terminar.

### 1.8. Memória e seções especiais

O `memory.x` define duas seções extras além das padrão:
- **`.data2`** em SRAM2 — variáveis que precisam de acesso rápido em DMA
- **`.ccmdata`** em CCMRAM — variáveis críticas com acesso single-cycle

A rotina `#[pre_init]` (linha 104) copia ambas da flash para a RAM antes mesmo da `main` executar, usando instruções `ldm`/`stm` em assembly inline. Isso é necessário porque o startup padrão do Cortex-M só copia `.data`.

---

## 2. Console Shell

### 2.1. Arquitetura geral

O shell está em `src/app/shell.rs` e roda como uma task assíncrona independente (`shell_taks`). Ela substituiu o eco simples da UART (o antigo `uart_task` que apenas devolvia o que recebia) por um interpretador de comandos completo.

O fluxo de processamento de cada caractere é:

```
UART → byte → echo_input_byte() → ShellBuffer::push_byte()
                                          ↓
                                  Quando '\r' → LineReady
                                          ↓
                                  take_line() → String<64>
                                          ↓
                                  parse_command() → ParsedCommand
                                          ↓
                                  execute_command() → ShellResponse<512>
                                          ↓
                                  uart.write() → UART de volta
```

### 2.2. Buffer circular de entrada (`ShellBuffer`)

```rust
pub struct ShellBuffer {
    buf: [u8; RX_BUF_SIZE],  // 64 bytes
    head: usize,              // posição de leitura
    tail: usize,              // posição de escrita
    len: usize,               // bytes ocupados
    state: RxState,           // Receiving | LineReady | Overflow
}
```

**Por que circular?** Porque o buffer precisa aceitar bytes assíncronos da UART enquanto o comando anterior ainda está sendo processado. Um buffer linear exigiria shiftar todos os bytes a cada caractere — ineficiente.

Métodos importantes:
- `push_byte(byte)` — insere um byte no buffer. Quando chega `\r` (CR), muda o estado para `LineReady`. Se encher, vai para `Overflow`.
- `pop_last()` — usado pelo **backspace**: remove o último byte sem precisar esvaziar o buffer inteiro.
- `take_line()` — extrai todos os bytes do buffer em uma `heapless::String<64>` quando uma linha está pronta.

### 2.3. Parser de comandos (`parse_command`)

Separa a linha em comando + argumentos usando `split_ascii_whitespace()`:

```
"led period 500" → ParsedCommand { command: "led", args: ["period", "500"] }
```

Limite de `MAX_ARGS = 8` argumentos. Se exceder, retorna `ShellError::TooManyArgs`.

**Por que `heapless::Vec`?** Porque o firmware é `#![no_std]` — não há allocador de heap. `heapless` fornece containers com tamanho fixo em tempo de compilação.

### 2.4. Tabela de comandos (`COMMANDS`)

```rust
pub const COMMANDS: &[CommandEntry] = &[
    CommandEntry { name: "help",    help: "Lista os comandos disponíveis" },
    CommandEntry { name: "status",  help: "Exibe um breve estado do sistema" },
    CommandEntry { name: "tasks",   help: "Lista de tarefas com métricas" },
    CommandEntry { name: "uptime",  help: "Mostra o tempo desde o boot" },
    CommandEntry { name: "mem",     help: "Mostra o tamanho das seções de memória" },
    CommandEntry { name: "led",     help: "Controla o LED: on, off, period <ms>" },
    CommandEntry { name: "sensors", help: "Mostra a ultima leitura dos sensores" },
    CommandEntry { name: "clean",   help: "Limpa o terminal" },
];
```

É um **array estático** em `rodata` (flash). Não há alocação dinâmica.

### 2.5. Executor de comandos (`execute_command`)

Cada comando é tratado por um braço do `match`. Padrão comum a todos:

```rust
"tasks" => {
    // 1. Copia os dados para fora do lock (rápido)
    let (adc, button, led) = {
        let monitor = MONITOR.lock().await;
        (monitor.adc, monitor.button, monitor.led)
    };
    // 2. Formata a resposta FORA do lock
    let _ = out.push_str("Tasks monitoradas: \r\n");
    write_task_metrics_line(&mut out, adc);
    // ...
}
```

**Isolamento do lock** — o mutex (`MONITOR`) é liberado antes de começar a formatação. Isso evita segurar outras tasks (como a `hcsr04_task` ou `am2302_task`) que precisam escrever no mesmo monitor.

### 2.6. Tratamento de backspace

O shell suporta edição de linha via backspace (`0x08` ou `0x7F`):

```rust
0x08 | 0x7F => {
    if shell.pop_last().is_some() {
        echo_input_byte(&mut uart, byte).await;  // ecoa "\x08 \x08"
    }
    continue;
}
```

O eco `\x08 \x08` (backspace + espaço + backspace) apaga visualmente o caractere no terminal.

### 2.7. Escrita não intrusiva

A resposta é construída em uma `ShellResponse<512>` e enviada de uma vez com `uart.write(response.as_bytes()).await`. A escrita é **assíncrona** — enquanto a UART transmite byte a byte via DMA, o executor pode rodar outras tasks.

**Limitação atual:** respostas muito longas (>512 bytes) são truncadas silenciosamente. O comando `help` ocupa ~300 bytes hoje — com folga, mas novos comandos podem exigir aumentar o buffer ou migrar para escrita progressiva (linha a linha).

---

## 3. Driver AM2302

### 3.1. O sensor AM2302

O **AM2302** (também conhecido como AOSONG DHT22 ou clone) é um sensor digital de temperatura e umidade relativa. Ele usa um **protocolo one-wire proprietário** (não confundir com o padrão Dallas/Maxim one-wire).

#### Características físicas

| Parâmetro | Valor |
|---|---|
| Tensão de operação | 3.3 V a 5.5 V |
| Faixa de temperatura | -40 °C a +80 °C |
| Faixa de umidade | 0% a 100% RH |
| Resolução | 0.1 °C / 0.1% RH |
| Precisão típica | ±0.5 °C / ±2% RH |
| Período mínimo entre leituras | 2 segundos |

#### Pinagem

| Pino | Função |
|---|---|
| 1 | VDD (alimentação) |
| 2 | DATA (bidirecional, open-drain) |
| 3 | NC (não conectado) |
| 4 | GND |

O pino DATA precisa de um **resistor de pull-up externo** de 4.7 kΩ a 10 kΩ para 3.3 V.

### 3.2. Protocolo de comunicação

O protocolo é mestre-escravo: o microcontrolador (mestre) inicia a comunicação, e o sensor (escravo) responde com 40 bits de dados.

```
                  INÍCIO (mestre)              RESPOSTA (sensor)                  DADOS (40 bits)
linha DATA:   ‾‾‾‾‾‾\____________________/‾‾‾‾‾‾\_______/‾‾‾‾‾\___/‾‾‾\___/‾‾‾\___/‾‾‾\__
                     ^                    ^        ^       ^   ^   ^   ^   ^   ^   ^
                     |                    |        |       |   |   |   |   |   |   |
                   HIGH (5 µs)         LOW (2 ms) 80 µs LOW 80 µs HIGH  bit 0  bit 1 ...
```

**Passo a passo:**

1. **Mestre puxa linha LOW por ≥1 ms** (nós usamos 2 ms para margem)
2. **Mestre libera linha** (coloca em alta impedância, pull-up leva para HIGH)
3. **Sensor aguarda 20-40 µs** e então:
   - Puxa LOW por ~80 µs (acknowledge)
   - Libera por ~80 µs (preparação)
4. **Sensor envia 40 bits** (5 bytes):
   - Cada bit começa com LOW de ~50 µs
   - Se o HIGH seguinte dura **26-28 µs** → bit **0**
   - Se o HIGH seguinte dura **~70 µs** → bit **1**

### 3.3. Formato dos dados (40 bits = 5 bytes)

```
Byte 0 (MSB)    Byte 1          Byte 2 (MSB)    Byte 3          Byte 4
[hhhh hhhh]    [llll llll]     [tttt tttt]     [tttt tttt]     [cccc cccc]
 ↑umidade int.  ↑umidade dec.  ↑temp. int.     ↑temp. dec.     ↑checksum
```

- **Umidade** = (Byte0 << 8 | Byte1) / 10 → %RH (ex: 525 = 52.5%)
- **Temperatura** = (Byte2 << 8 | Byte3) / 10 → °C
  - Bit 15 do valor de 16 bits indica sinal negativo: `0x8000` = temperatura negativa
- **Checksum** = (Byte0 + Byte1 + Byte2 + Byte3) & 0xFF

### 3.4. Implementação do driver

Arquivo: `src/drivers/am2302_capture.rs`

#### Abordagem: Timer Input Capture + DMA

Diferente de drivers ingênuos que fazem **polling** do pino GPIO em loop apertado (consumindo 100% da CPU durante a leitura), nossa implementação delega a captura dos pulsos ao **hardware do timer STM32**:

1. O **TIM4** é configurado em **input capture mode** a 1 MHz (1 tick = 1 µs)
2. O canal 1 do TIM4 é ligado ao pino PB6 (DATA do sensor)
3. O **DMA** copia automaticamente cada timestamp de borda para um buffer em RAM

```
                        TIM4 (1 MHz)
                            │
PB6 ──────┬──── TIM4_CH1 (input capture)
           │
    ┌──────┴──────┐
    │  InputCapture │  detecta cada borda (subida/descida)
    └──────┬──────┘
           │ (valor do contador a cada borda)
           ▼
    ┌──────────────┐
    │   DMA_CH5     │  copia para o buffer RAM
    └──────┬──────┘
           ▼
    ┌──────────────┐
    │  edges[83]    │  array de timestamps
    └──────────────┘
```

**Por que 83 bordas?** O frame completo do AM2302 tem:
- 2 bordas do sinal de resposta (LOW→HIGH e HIGH→LOW)
- 80 bordas dos 40 bits (cada bit tem 2 bordas: rising e falling)
- 1 borda extra para detecção de alinhamento

Total: 2 + 80 + 1 = **83 bordas**.

#### Estrutura do driver

```rust
pub struct Am2302Capture<'d, T, P, D>
where
    T: GeneralInstance4Channel,     // TIM4
    P: TimerPin<T, Ch1>,            // PB6 como canal 1
    D: TimerDma<T, Ch1>,            // DMA1_CH5
{
    pin: Peri<'d, P>,
    timer: Peri<'d, T>,
    dma: Peri<'d, D>,
}
```

**Por que usar generics em vez de tipos concretos?** Para manter a arquitetura agnóstica de hardware — se no futuro for necessário usar outro timer ou pino, basta mudar o tipo no momento da instanciação.

#### Método `read()` — o ciclo completo

```rust
pub async fn read<I>(&mut self, irq: I) -> Result<(f32, f32), Am2302CaptureError>
```

1. **`begin_signal(pin, &mut delay)`** — gera o start do protocolo:
   - Configura PB6 como `Flex` (GPIO bidirecional open-drain)
   - Põe HIGH por 5 µs (estabilização)
   - Põe LOW por 2 ms (start condition)
   - Libera (coloca HIGH novamente)

2. **`make_capture(pin, timer, irq)`** — reconfigura PB6 do GPIO para a função alternativa de timer (input capture), ligando-o ao canal 1 do TIM4.

3. **`capture_edges(dma, capture, irq)`** — dispara a captura:
   - Chama `capture.receive_waveform::<Ch1, D>(...)` — função do Embassy que arma o timer + DMA para capturar todas as bordas do canal 1
   - Aguarda com timeout de 6 ms (via `with_timeout`)
   - Se o timeout estourar, retorna `Am2302CaptureError::Timeout`

4. **`find_first_bit_edge(&edges)`** — encontra onde os dados reais começam:
   - O buffer pode conter as bordas da resposta do sensor (2 pulsos de ~80 µs cada) antes dos bits
   - A função busca sequências de 3 bordas consecutivas que correspondam ao padrão conhecido (HIGH de resposta ~80 µs, LOW de resposta ~80 µs, LOW do primeiro bit ~50 µs)
   - Se não encontrar, retorna `Am2302CaptureError::Protocol` com os primeiros deltas para debug

5. **`decode_bytes(&edges, first_bit_rise)`** — percorre as 40 bordas a partir do ponto encontrado:
   ```rust
   for bit_index in 0..40 {
       let rise = edges[first_bit_rise + bit_index * 2];
       let fall = edges[first_bit_rise + bit_index * 2 + 1];
       let high_width_us = fall.wrapping_sub(rise);
       let bit = if high_width_us > 50 { 1 } else { 0 };  // threshold = 50 µs
       bytes[byte_index] = (bytes[byte_index] << 1) | bit;
   }
   ```
   O threshold de 50 µs separa bits 0 (~26 µs) de bits 1 (~70 µs).

6. **`validate_checksum(&bytes)`** — verifica se `bytes[4] == bytes[0] + bytes[1] + bytes[2] + bytes[3]`. Se falhar, prepara um log de debug com os bytes e bordas para análise em bancada.

7. **`decode_measurements(&bytes)`** — converte os 4 bytes de dados em `(f32, f32)`:
   - Umidade = (byte0 << 8 | byte1) / 10.0
   - Temperatura: extrai magnitude (bits 0-14) e sinal (bit 15), divide por 10.0

#### Por que usar DMA em vez de interrupção simples?

O AM2302 envia 40 bits em sequência — isso significa 83 bordas em ~5 ms. Se cada borda gerasse uma interrupção, a CPU seria interrompida 83 vezes, cada uma exigindo salvamento/restauração de contexto (~20 ciclos). Com DMA, a CPU não é interrompida nenhuma vez durante a captura: o DMA copia os timestamps silenciosamente enquanto a CPU pode executar outras tasks.

#### Task de leitura periódica (`am2302_task`)

```rust
#[embassy_executor::task]
async fn am2302_task(mut sensor: Am2302Capture<'static, peripherals::TIM4, peripherals::PB6, peripherals::DMA1_CH5>) {
    loop {
        match sensor.read(Irqs).await {
            Ok((temperature_c, humidity_rh)) => {
                let now_ms = Instant::now().as_millis() as u64;
                let mut monitor = MONITOR.lock().await;
                monitor.am2302.update_success(temperature_c, humidity_rh, now_ms);
            }
            Err(error) => {
                let mut monitor = MONITOR.lock().await;
                monitor.am2302.update_error(error);
            }
        }
        Timer::after_secs(2).await;  // período mínimo entre leituras
    }
}
```

A task:
1. Tenta ler o sensor (bloqueante em microssegundos, mas ~5 ms apenas)
2. Atualiza o **snapshot compartilhado** (`monitor.am2302`) — outras tasks (shell) podem ler sem interferir
3. Aguarda 2 segundos (período mínimo recomendado pelo datasheet)

---

## 4. Driver HC-SR04

### 4.1. O sensor HC-SR04

O **HC-SR04** é um sensor ultrassônico de distância que mede de 2 cm a 400 cm com precisão de ~3 mm.

#### Características físicas

| Parâmetro | Valor |
|---|---|
| Tensão de operação | 5 V (VCC) |
| Tensão do ECHO | 5 V (lógica) |
| Corrente típica | 15 mA (ativo), <2 mA (standby) |
| Faixa de medição | 2 cm a 400 cm |
| Resolução | ~3 mm |
| Ângulo de detecção | ~15° |
| Frequência ultrassônica | 40 kHz |

#### Atenção elétrica

O pino **ECHO** do HC-SR04 emite 5 V. O STM32G474RE opera a 3.3 V e **nem todos os pinos são 5V-tolerant**. Por isso:
- **PC7** foi escolhido para o ECHO — é 5V-tolerant conforme o datasheet
- Caso contrário, seria necessário um divisor resistivo (ex: 10 kΩ + 5.6 kΩ para ~3.3 V)

### 4.2. Protocolo de funcionamento

```
TRIG:  ‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾\___________________/‾‾‾‾‾‾‾‾‾‾‾‾‾‾
                                ↑ 10 µs
                                
ECHO:  ‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾\_____/‾‾‾‾‾\_________________
                                    ↑ pulse width
                                      (150 µs a 25 ms)
```

**Passo a passo:**

1. **Mestre envia trigger**: pino TRIG HIGH por **10 µs**
2. **Sensor emite 8 pulsos** ultrassônicos a 40 kHz
3. **Sensor coloca ECHO em HIGH** — início da medição
4. **Sensor mantém ECHO HIGH** pelo tempo proporcional à distância:
   - 150 µs → ~2.5 cm (mínimo)
   - 25 ms → ~430 cm (máximo)
5. **Mestre mede a largura do pulso HIGH** no ECHO
6. **Converte para distância**:
   - `distância_cm = (tempo_µs * 0.034) / 2`
   - Simplificado: `distância_cm = tempo_µs / 58`

### 4.3. Arquitetura em duas camadas

O driver do HC-SR04 segue a mesma filosofia do AM2302: **delegar o timing crítico ao hardware**.

```
                    HC-SR04
                       │
          ┌────────────┴────────────┐
          │                         │
       TRIG (PA6)               ECHO (PC7)
     (GPIO Output)         (Timer Input Capture)
          │                         │
          │                    TIM3_CH2
          │                    (1 MHz, input capture)
          │                         │
          ▼                         ▼
    ┌──────────┐           ┌──────────────────┐
    │ HcSr04   │───────▶   │ TimPulseInput    │
    │ (driver  │           │ (medição de pulso)│
    │  secund.)│           └──────────────────┘
    └──────────┘
```

**Camada primária: `PulseInput` trait e `TimPulseInput`**

Em `src/drivers/hcsr04.rs`:

```rust
pub trait PulseInput {
    type Error;
    async fn measure_high_pulse(&mut self, timeout: Duration) -> Result<Duration, Self::Error>;
}
```

A implementação concreta `TimPulseInput` usa o **TIM3 em input capture**:

```rust
pub struct TimPulseInput<'d, T, P>
where
    T: GeneralInstance4Channel<Word = u16>,  // TIM3
    P: TimerPin<T, Ch2>,                      // PC7 como canal 2
{
    pin: Peri<'d, P>,
    timer: Peri<'d, T>,
}
```

O método `measure_high_pulse_with_irq`:
1. **Configura o timer** a 1 MHz (1 tick = 1 µs) via `make_capture()`
2. **Aguarda a borda de subida** (rising edge) com timeout de 40 ms
3. **Aguarda a borda de descida** (falling edge) com timeout de 40 ms
4. **Calcula a diferença** — `fall_tick - rise_tick` = largura do pulso em µs
5. Retorna `Duration::from_micros(width_us)`

**Camada secundária: `HcSr04`**

Em `src/drivers/Hc_Sr_04.rs`:

```rust
pub struct HcSr04<TRIG, ECHO> {
    trig: TRIG,   // OutputPin
    echo: ECHO,   // PulseInput
}
```

Método `measure_distance_cm_with_irq`:
1. **`trigger_sensor()`**: coloca TRIG HIGH por 10 µs usando `cortex_m::asm::delay(170 * 10)` — delay bloqueante mas muito curto (10 µs)
2. **`echo.measure_high_pulse_with_irq(timeout, irq)`**: mede o pulso HIGH do ECHO;
   a task *cede a CPU* enquanto o timer espera as bordas — zero consumo de CPU durante a espera
3. **`pulse_to_distance_cm(pulse)`**: converte a largura medida para centímetros: `pulse_us / 58.0`

**Por que o trigger usa delay bloqueante?**

O pulso de trigger precisa de exatos 10 µs. Não há suporte nativo do Embassy para `Timer::after_micros()` em ambiente async (o timer do Embassy tem resolução de 1 tick de 1 ms). A alternativa seria configurar um timer de hardware dedicado só para o trigger, o que é desnecessário — 10 µs de busy-wait a 170 MHz são apenas 1700 ciclos de CPU.

### 4.4. Task periódica (`hcsr04_task`)

```rust
#[embassy_executor::task]
async fn hcsr04_task(
    mut sensor: HcSr04<Output<'static>, TimPulseInput<'static, peripherals::TIM3, peripherals::PC7>>,
) {
    loop {
        match sensor.measure_distance_cm_with_irq(Irqs).await {
            Ok(distance_cm) => {
                let now_ms = Instant::now().as_millis() as u64;
                let mut monitor = MONITOR.lock().await;
                monitor.hcsr04.update_success(distance_cm, now_ms);
                info!("HC-SR04: {} cm", distance_cm);
            }
            Err(error) => {
                let mut monitor = MONITOR.lock().await;
                monitor.hcsr04.update_error(error);
                warn!("HC-SR04 error: {:?}", error);
            }
        }
        Timer::after_millis(500).await;  // evita congestionamento acústico
    }
}
```

**Intervalo entre medições de 500 ms** — o datasheet recomenda pelo menos 60 ms entre triggers para evitar eco de medições anteriores. 500 ms dá ampla margem e não satura o barramento acústico.

### 4.5. Comparação: AM2302 vs HC-SR04

| Aspecto | AM2302 | HC-SR04 |
|---|---|---|
| Protocolo | One-wire bidirecional | Trigger + Echo |
| Timing crítico | ~50 µs por bit | Medição única de pulso |
| Abordagem | DMA captura 83 bordas | Input capture, 2 bordas |
| Task bloqueante? | ~5 ms (leitura completa) | ~10 µs (só o trigger) |
| Intervalo entre leituras | 2 s | 500 ms |

Ambos seguem o mesmo princípio: **o caminho crítico de tempo fica no hardware do timer**, não na CPU. A CPU só é acordada quando os dados já estão capturados.

---

## Snapshot e sincronização entre tasks

Tanto o AM2302 quanto o HC-SR04 usam o mesmo padrão de **snapshot compartilhado**:

```
sensor_task (escritor)              shell (leitor)
       │                                │
       │ MONITOR.lock().await           │ MONITOR.lock().await
       │   ↓                            │   ↓
       │ monitor.am2302/hcsr04          │ monitor.am2302/hcsr04
       │   .update_success(...)         │   (copia por valor)
       │ MONITOR.drop()                 │ MONITOR.drop()
       │                                │
       │                                │ formata resposta
       │                                │ uart.write()
```

**Vantagens:**
- A shell nunca dispara uma leitura de sensor — só lê o último valor conhecido
- O lock é retido pelo tempo mínimo necessário (cópia de structs `Copy`)
- Leituras e comandos não interferem entre si
- Erros são preservados sem apagar a última leitura válida

---
