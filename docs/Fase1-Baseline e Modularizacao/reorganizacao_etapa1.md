# Reorganização da `main` - Plano Detalhado

Este documento mostra como reorganizar o `src/main.rs` extraindo blocos da `main` atual em funções auxiliares, sem mudar o comportamento do firmware.

## Imports e aliases necessarios

Se voce quiser implementar exatamente os exemplos abaixo, estes imports e aliases sao os mais importantes para evitar erros de tipo:

```rust
use defmt::{error, info, println, unwrap};
use embassy_executor::Spawner;
use embassy_stm32::adc::{Adc, AnyAdcChannel, SampleTime};
use embassy_stm32::exti::{self, ExtiInput};
use embassy_stm32::gpio::{Level, Output, OutputType, Pull, Speed};
use embassy_stm32::i2c::{self, I2c};
use embassy_stm32::peripherals::{self, ADC2};
use embassy_stm32::time::{khz, Hertz};
use embassy_stm32::timer::simple_pwm::{PwmPin, SimplePwm};
use embassy_stm32::usart::{self, Uart};
use embassy_stm32::{bind_interrupts, interrupt, Config, Peri};
use embassy_time::Timer;

use adxl345_async::{Address, Adxl345Async, DataRate, I2cBus, Range};
```

Para simplificar a assinatura do acelerometro, vale a pena declarar aliases de tipo:

```rust
type AsyncI2c =
    I2c<'static, embassy_stm32::mode::Async, embassy_stm32::i2c::mode::Master>;

type AccelDevice = Adxl345Async<I2cBus<AsyncI2c>>;
```

Observacao importante: nas funcoes auxiliares abaixo, os parametros dos perifericos usam `Peri<'static, ...>`.
Isso acontece porque o Embassy 0.6 entrega os recursos da HAL como tokens tipados, e os construtores como `ExtiInput::new`, `Uart::new`, `I2c::new`, `Output::new` e `Adc::new` esperam exatamente esse wrapper.

---

## Por que reorganizar

A `main` atual faz tudo em sequência: clock, logs, setup de periféricos, self-tests, spawn de tasks e loop do LED. Isso funciona, mas dificulta:

- entender o que cada parte faz
- localizar um problema específico
- adicionar ou remover periféricos depois
- reutilizar configuração entre partes do código

O objetivo é deixar a `main` como uma **orquestradora** que só coordena a inicialização, enquanto cada bloco vira uma função com nome e responsabilidade claros.

---

## 1. Função de configuração do chip

Extrai a configuração de clock e RCC da `main` atual (linhas 191-215).

```rust
/// Retorna a configuracao de clocks do STM32G474.
///
/// HSE externo de 24 MHz, PLL configurado para 170 MHz no sistema.
/// Os prescalers AHB/APB1/APB2 ficam em DIV1 (sem divisao).
fn stm32_config() -> Config {
    use embassy_stm32::rcc::*;

    // Use o Config do modulo embassy_stm32 inteiro.
    // Se voce usar apenas Config::default() depois do use acima,
    // o compilador pode tentar usar embassy_stm32::rcc::Config,
    // que e outro tipo e causara erro em config.rcc.*.
    let mut config = embassy_stm32::Config::default();

    // HSE: oscilador externo de 24 MHz (cristal da placa).
    config.rcc.hse = Some(Hse {
        freq: Hertz(24_000_000),
        mode: HseMode::Oscillator,
    });

    // PLL: entrada 24 MHz / 6 = 4 MHz, * 85 = 340 MHz, / 2 = 170 MHz.
    config.rcc.pll = Some(Pll {
        source: PllSource::HSE,
        prediv: PllPreDiv::DIV6,
        mul: PllMul::MUL85,
        divp: None,
        divq: None,
        divr: Some(PllRDiv::DIV2),
    });

    // Clock do sistema vindo do PLL1_R.
    config.rcc.sys = Sysclk::PLL1_R;

    // ADC12 usa o clock do sistema diretamente.
    config.rcc.mux.adc12sel = mux::Adcsel::SYS;

    // Nenhum prescaler nos barramentos principais.
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV1;
    config.rcc.apb2_pre = APBPrescaler::DIV1;

    // Retorno explicito da funcao.
    config
}
```

**Por que existe:** centraliza a configuração de clock em um lugar só, fora da `main`.

**Síncrona ou assíncrona?** `fn`, não `async`, porque só monta structs, sem `await`.

**Segura ou `unsafe`?** Segura, usa apenas APIs seguras do Embassy.

---

## 2. Função de logs iniciais

Extrai os logs de startup e verificação das seções especiais de memória.

```rust
/// Loga mensagens iniciais e verifica as variaveis em CCMRAM e SRAM2.
///
/// Se os valores aparecerem corretos no log, as secoes especiais
/// foram copiadas com sucesso pelo #[pre_init].
fn log_startup() {
    info!("Hello World!");

    // TESTE esta em .ccmram, TESTE2 esta em .data2 (SRAM2).
    // O #[pre_init] copia ambas de flash para RAM antes da main.
    println!("Teste de variavel na memoria CCMRAM {}", TESTE);
    println!("Teste de variavel na memoria SRAM2 {}", TESTE2);
}
```

**Por que existe:** separa a "apresentação inicial" do resto da inicialização.

**Síncrona ou assíncrona?** `fn`, uso de `info!` e `println!` não requer `await`.

**Segura ou `unsafe`?** Segura.

---

## 3. Função de setup do botão

Extrai a criação do `ExtiInput` para o botão `PC13`.

```rust
/// Configura o botao do usuario (PC13) com interrupcao EXTI.
///
/// Usa Pull::Down (pull-down interno) para que o estado padrao
/// seja LOW e a borda de subida indique o pressionamento.
fn init_button(
    pc13: Peri<'static, peripherals::PC13>,
    exti13: Peri<'static, peripherals::EXTI13>,
) -> ExtiInput<'static, embassy_stm32::mode::Async> {
    // Irqs contem os handlers de interrupcao definidos no bind_interrupts!.
    ExtiInput::new(pc13, exti13, Pull::Down, Irqs)
}
```

**Por que existe:** o setup do botão é uma linha só, mas extrair melhora a legibilidade e centraliza a configuração.

**Síncrona ou assíncrona?** `fn`, síncrona, sem `await`.

**Segura ou `unsafe`?** Segura.

---

## 4. Função de setup do ADC

Extrai a criação do ADC2 e do canal analógico em PA7.

```rust
/// Cria o ADC2 e o canal analogico a partir do pino PA7.
///
/// Retorna uma tupla (Adc, AnyAdcChannel) para ser usada tanto
/// no self-test quanto na task adc_task.
fn init_adc(
    adc2: Peri<'static, peripherals::ADC2>,
    pa7: Peri<'static, peripherals::PA7>,
) -> (Adc<'static, ADC2>, AnyAdcChannel<'static, ADC2>) {
    // Adc::new com Default::default() usa configuracao padrao do Embassy.
    let adc = Adc::new(adc2, Default::default());

    // degrade_adc() converte o pino PA7 em um canal analogico generico.
    let adc_channel = pa7.degrade_adc();

    (adc, adc_channel)
}
```

**Por que existe:** separa a criação do ADC da leitura inicial de teste e da task.

**Síncrona ou assíncrona?** `fn`, configuração de periférico é instantânea.

**Segura ou `unsafe`?** Segura.

---

## 5. Função de self-test do ADC

Extrai a leitura única de validação do ADC no startup.

```rust
/// Faz uma leitura unica do ADC para validar o hardware ja no boot.
///
/// O valor lido e logado via defmt. Se aparecer variando, o ADC
/// esta funcionando. Se ficar em zero ou maximo, algo pode estar errado.
fn log_initial_adc_sample(
    adc: &mut Adc<'static, ADC2>,
    adc_channel: &mut AnyAdcChannel<'static, ADC2>,
) {
    // blocking_read e síncrono, trava a task atual.
    let measured = adc.blocking_read(adc_channel, SampleTime::CYCLES247_5);
    info!("measured: {}", measured);
}
```

**Por que existe:** separa nitidamente o self-test do setup do periférico.

**Síncrona ou assíncrona?** `fn`, `blocking_read` é síncrono (bloqueante).

**Segura ou `unsafe`?** Segura.

---

## 6. Função de self-test do sensor de temperatura interno

Extrai a leitura do sensor interno do STM32G4.

```rust
/// Le o sensor de temperatura interno do chip uma vez no startup.
///
/// Loga o valor cru. Nao faz conversao para graus Celsius,
/// apenas confirma que o sensor responde.
fn log_temperature_sample(adc1: Peri<'static, peripherals::ADC1>) {
    // ADC1 e usado exclusivamente para o sensor interno.
    let mut adc_temp = Adc::new(adc1, Default::default());

    // Habilita o sensor de temperatura interno do chip.
    let mut temperature = adc_temp.enable_temperature();

    // Leitura unica, síncrona.
    let temp = adc_temp.blocking_read(&mut temperature, SampleTime::CYCLES247_5);
    info!("measured: {}", temp);
}
```

**Por que existe:** separa esse self-test do setup principal do ADC2/PA7.

**Síncrona ou assíncrona?** `fn`, síncrona.

**Segura ou `unsafe`?** Segura.

---

## 7. Função de setup da UART

Extrai a configuração da LPUART1.

```rust
/// Configura a LPUART1 como UART assincrona com DMA.
///
/// Bautrate de 115200, pinos PA3 (RX) e PA2 (TX).
/// Usa DMA1 CH1 e CH2 para RX e TX.
fn init_uart(
    lpuart1: Peri<'static, peripherals::LPUART1>,
    pa3: Peri<'static, peripherals::PA3>,
    pa2: Peri<'static, peripherals::PA2>,
    dma1_ch1: Peri<'static, peripherals::DMA1_CH1>,
    dma1_ch2: Peri<'static, peripherals::DMA1_CH2>,
) -> Uart<'static, embassy_stm32::mode::Async> {
    let mut config = usart::Config::default();
    config.baudrate = 115_200;

    // Uart::new com DMA. Irqs contem os handlers de interrupcao.
    Uart::new(lpuart1, pa3, pa2, dma1_ch1, dma1_ch2, Irqs, config).unwrap()
}
```

**Por que existe:** centraliza a configuração da UART, 7 parâmetros bem organizados.

**Síncrona ou assíncrona?** `fn`, síncrona.

**Segura ou `unsafe`?** Segura.

---

## 8. Função de setup do PWM

Extrai a configuração do TIM1 em modo PWM no pino PC0.

```rust
/// Configura o TIM1 como PWM simples no canal 1 (PC0).
///
/// Frequencia de 10 kHz, push-pull, duty cycle iniciado em 0.
fn init_pwm(
    tim1: Peri<'static, peripherals::TIM1>,
    pc0: Peri<'static, peripherals::PC0>,
) -> SimplePwm<'static, peripherals::TIM1> {
    let ch1_pin = PwmPin::new(pc0, OutputType::PushPull);

    // SimplePwm com somente o canal 1 ativo.
    SimplePwm::new(
        tim1,
        Some(ch1_pin),
        None, // ch2
        None, // ch3
        None, // ch4
        khz(10),
        Default::default(),
    )
}
```

**Por que existe:** extrai o setup do timer/PWM da `main`.

**Síncrona ou assíncrona?** `fn`, síncrona.

**Segura ou `unsafe`?** Segura.

---

## 9. Função de setup do acelerômetro ADXL345

Extrai a configuração do barramento I2C1, detecção do ADXL345, setup do driver e leitura inicial.

```rust
/// Inicializa o barramento I2C1, detecta o ADXL345, configura o
/// driver e faz uma leitura inicial de validacao.
///
/// Retorna o driver Adxl345Async pronto para a task accel_task.
async fn init_accelerometer(
    i2c1: Peri<'static, peripherals::I2C1>,
    pb8: Peri<'static, peripherals::PB8>,
    pb9: Peri<'static, peripherals::PB9>,
    dma1_ch3: Peri<'static, peripherals::DMA1_CH3>,
    dma1_ch4: Peri<'static, peripherals::DMA1_CH4>,
    pc9: Peri<'static, peripherals::PC9>,
) -> AccelDevice {
    // Configuracao do barramento I2C.
    let mut config: i2c::Config = Default::default();
    config.scl_pullup = true;   // pull-up interno no SCL
    config.sda_pullup = true;   // pull-up interno no SDA
    config.frequency = Hertz(100_000);  // 100 kHz (modo standard)
    config.timeout = embassy_time::Duration::from_millis(1000);

    // I2C1 com DMA (CH3 e CH4).
    let mut i2c = I2c::new(i2c1, pb8, pb9, dma1_ch3, dma1_ch4, Irqs, config);

    // A linha PC9 e usada como controle de habilitacao.
    let mut i2c_cs = Output::new(pc9, Level::Low, Speed::Low);
    Timer::after_millis(50).await;

    let mut data = [0u8; 1];
    i2c_cs.set_high();
    Timer::after_millis(5).await;

    // Le o registrador WHOAMI (0x00) para verificar se o ADXL345 responde.
    // O valor esperado e 0xE5.
    match i2c.write_read(ADDRESS, &[WHOAMI], &mut data).await {
        Ok(()) => {
            if data[0] == 0xE5 {
                info!("ADXL345 found!");
            } else {
                info!("Whoami: {}", data[0]);
            }
        }
        Err(e) => error!("I2c Error: {:?}", e),
    }

    // Envolve o I2c no wrapper I2cBus que o driver adxl345-async espera.
    let bus = I2cBus::new(i2c, Some(Address::SECONDARY));
    let mut accel = Adxl345Async::new(bus);

    // Setup padrao do ADXL345.
    let _ = accel.setup().await;

    // Leitura inicial para confirmar comunicacao.
    match accel.get_accel_raw().await {
        Ok((x, y, z)) => {
            info!("ADXL345 Accel Raw: x={}, y={}, z={}", x, y, z);
        }
        Err(e) => error!("Error reading accel: {:?}", e),
    }

    accel
}
```

**Por que existe:** é o maior bloco de setup da `main` atual. Extrair melhora muito a legibilidade.

**Síncrona ou assíncrona?** `async fn`, porque usa `.await` em:
- `Timer::after_millis`
- `i2c.write_read`
- `accel.setup()`
- `accel.get_accel_raw()`

**Segura ou `unsafe`?** Segura.

---

## 10. Função do loop do LED

Extrai o loop infinito do LED PA5 da `main`.

```rust
/// Loop infinito que pisca o LED em PA5 (500 ms ligado, 500 ms desligado).
///
/// Retorna o tipo ! (nunca retorna), porque fica em loop eterno.
async fn run_led_loop(pa5: Peri<'static, peripherals::PA5>) -> ! {
    let mut led = Output::new(pa5, Level::High, Speed::Low);

    loop {
        led.set_high();
        Timer::after_millis(500).await;

        led.set_low();
        Timer::after_millis(500).await;
    }
}
```

**Por que existe:** separa o heartbeat visual da `main`, deixando claro que é uma responsabilidade independente.

**Síncrona ou assíncrona?** `async fn`, porque usa `Timer::after_millis(...).await`.

**Segura ou `unsafe`?** Segura.

---

## 11. A `main` reorganizada

Com todas as funções acima, a `main` fica assim:

```rust
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // === 1. Configuracao do chip ===
    // Cria a configuracao de clock e inicializa o microcontrolador.
    let config = stm32_config();
    let p = embassy_stm32::init(config);

    // === 2. Logs de startup ===
    // Exibe Hello World e verifica as secoes especiais de memoria.
    log_startup();

    // === 3. Self-checks de hardware ===
    // Leitura unica do ADC em PA7 e do sensor de temperatura interno.
    // Serve para validar que o ADC responde ja no boot.
    let (mut adc, mut adc_channel) = init_adc(p.ADC2, p.PA7);
    log_initial_adc_sample(&mut adc, &mut adc_channel);
    log_temperature_sample(p.ADC1);

    // === 4. Inicializacao dos perifericos ===
    // Cada periferico ganha sua propria funcao de setup.
    let button = init_button(p.PC13, p.EXTI13);
    let lpuart = init_uart(p.LPUART1, p.PA3, p.PA2, p.DMA1_CH1, p.DMA1_CH2);
    let pwm = init_pwm(p.TIM1, p.PC0);
    let accel = init_accelerometer(
        p.I2C1,
        p.PB8,
        p.PB9,
        p.DMA1_CH3,
        p.DMA1_CH4,
        p.PC9,
    )
    .await;

    // === 5. Runtime concorrente ===
    // Cada periferico e entregue a sua task que roda em background.
    spawner.spawn(unwrap!(adc_task(adc, adc_channel)));
    spawner.spawn(unwrap!(button_task(button)));
    spawner.spawn(unwrap!(uart_task(lpuart)));
    spawner.spawn(unwrap!(pwm_task(pwm)));
    spawner.spawn(unwrap!(accel_task(accel)));

    // === 6. Loop principal ===
    // Fica piscando o LED, nunca retorna.
    run_led_loop(p.PA5).await;
}
```

---

## Mudança em relação ao código atual

O comportamento final é **exatamente o mesmo**:

- mesmos clocks (170 MHz)
- mesmos periféricos (ADC, botão, UART, PWM, ADXL345)
- mesmas tasks executando em background
- mesmo loop do LED
- mesmos self-tests de startup

O que muda é a estrutura:

| Aspecto | Código atual | Código reorganizado |
|---|---|---|
| Configuração de clock | Dentro da `main` | Função `stm32_config()` |
| Logs de startup | Solto na `main` | Função `log_startup()` |
| Setup do botão | Linha solta na `main` | Função `init_button()` |
| Setup do ADC | Misturado com leitura inicial | Função `init_adc()` |
| Leitura inicial ADC | Misturado no setup | Função `log_initial_adc_sample()` |
| Leitura temperatura | Bloco na `main` | Função `log_temperature_sample()` |
| Setup da UART | Bloco na `main` | Função `init_uart()` |
| Setup do PWM | Bloco na `main` | Função `init_pwm()` |
| Setup do ADXL345 | Bloco grande na `main` | Função `init_accelerometer()` |
| Loop do LED | Loop no fim da `main` | Função `run_led_loop()` |
| `main()` | 140 linhas com tudo misturado | ~30 linhas de orquestração |

---

## Resumo

| Função | Tipo | `async?` | `unsafe?` | O que faz |
|---|---|---|---|---|
| `stm32_config()` | `fn` | Não | Não | Configura clocks e RCC |
| `log_startup()` | `fn` | Não | Não | Loga Hello World e seções especiais |
| `init_button()` | `fn` | Não | Não | Cria `ExtiInput` para PC13 |
| `init_adc()` | `fn` | Não | Não | Cria ADC2 e canal PA7 |
| `log_initial_adc_sample()` | `fn` | Não | Não | Leitura única de validação do ADC |
| `log_temperature_sample()` | `fn` | Não | Não | Leitura do sensor interno |
| `init_uart()` | `fn` | Não | Não | Configura LPUART1 com DMA |
| `init_pwm()` | `fn` | Não | Não | Configura TIM1/PWM em PC0 |
| `init_accelerometer()` | `async fn` | Sim | Não | Sobe I2C, detecta e configura ADXL345 |
| `run_led_loop()` | `async fn` | Sim | Não | Loop infinito piscando LED |
| `main()` | `async fn` | Sim | Não | Orquestra a inicialização |
| `before_main()` | `fn` | Não | **Sim** | Cópia manual de `.ccmram` e `.data2` |

A única função `unsafe` do arquivo continua sendo `before_main()`, que não foi alterada na reorganização.
