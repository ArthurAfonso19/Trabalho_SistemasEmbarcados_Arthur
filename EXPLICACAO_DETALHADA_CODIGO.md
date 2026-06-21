# Explicacao Detalhada Do Codigo

Este arquivo foi escrito para quem esta comecando em Rust e precisa entender, com bastante contexto, o exemplo deste repositorio.

O foco principal esta em `src/main.rs`, mas alguns arquivos de configuracao tambem sao importantes para entender por que o codigo compila e como ele roda na placa.

## 1. O que este projeto e

Este projeto e um firmware embarcado em Rust para um microcontrolador STM32G474RE.

Em vez de ser um programa comum de computador:
- ele nao roda no Windows ou Linux
- ele roda diretamente no microcontrolador
- ele acessa perifericos fisicos, como GPIO, ADC, UART, I2C, PWM e interrupcoes

O programa faz varias coisas ao mesmo tempo:
- le uma entrada analogica com ADC
- detecta um botao via EXTI
- faz eco pela UART
- gera PWM
- conversa com um acelerometro ADXL345 por I2C
- pisca um LED no pino `PA5`

Ou seja: ele serve como demonstracao de varios recursos de firmware usando Rust async com Embassy.

## 2. Arquivos importantes para entender o projeto

## `Cargo.toml`

Este e o manifesto do projeto Rust.

Ele define:
- nome do pacote
- versao
- edicao do Rust
- dependencias

As dependencias mais importantes aqui sao:
- `cortex-m`: utilitarios para microcontroladores ARM Cortex-M
- `cortex-m-rt`: runtime de baixo nivel, incluindo inicializacao e suporte a `#[pre_init]`
- `panic-probe`: tratamento de panic com saida integrada ao `defmt`
- `embassy-stm32`: HAL/abstracao async para STM32
- `embassy-executor`: executor async para tarefas embarcadas
- `embassy-time`: temporizadores async
- `embassy-sync` e `embassy-futures`: apoio ao ecossistema Embassy
- `defmt` e `defmt-rtt`: logging otimizado para sistemas embarcados
- `adxl345-async`: driver async para o acelerometro ADXL345

Resumo pratico: `Cargo.toml` diz quais bibliotecas o codigo usa para conversar com o hardware e para rodar tarefas async.

## `.cargo/config.toml`

Esse arquivo fixa configuracoes de build importantes:

- o target e `thumbv7em-none-eabihf`
- `cargo run` usa `probe-rs run --chip STM32G474RETx`
- a variavel `DEFMT_LOG` fica em `info`

Isso significa:
- o projeto esta configurado para compilar para ARM embarcado, nao para o PC
- `cargo run` nao abre um programa local; ele compila e grava na placa

## `build.rs`

Esse arquivo injeta argumentos do linker:

- `--nmagic`
- `-Tlink.x`
- `-Tdefmt.x`

Na pratica, ele ajuda o processo de linkagem a usar o layout correto de memoria e o suporte de `defmt`.

## `memory.x`

Esse arquivo define o mapa de memoria.

Ele declara regioes como:
- `FLASH`
- `RAM`
- `SRAM2`
- `CCMRAM`

Tambem declara secoes customizadas:
- `.data2` em `SRAM2`
- `.ccmdata` em `CCMRAM`

Isso importa porque o codigo usa uma rotina manual em `#[pre_init]` para copiar dados dessas secoes antes de `main` comecar.

## `Embed.toml`

Define o chip `STM32G474RETx` e habilita RTT.

RTT e um mecanismo de comunicacao com o host usado com frequencia para logs embarcados, incluindo `defmt-rtt`.

## `src/main.rs`

Esse e o coracao do projeto.

E nele que estao:
- os imports
- as tarefas async
- o mapeamento de interrupcoes
- constantes e estaticos globais
- a rotina `#[pre_init]`
- a funcao `main`

## 3. Antes de ler o codigo: o modelo mental certo

Se voce vem de C, Python ou Java, este programa parece estranho no inicio porque mistura:
- Rust embarcado
- runtime especial
- async/await
- tipos genericos muito longos
- manipulacao direta de hardware
- uma pequena parte `unsafe`

O jeito mais facil de entender e pensar assim:

1. O runtime inicializa o microcontrolador.
2. O codigo configura clock e perifericos.
3. O executor Embassy roda varias tarefas async em paralelo cooperativo.
4. Cada tarefa cuida de um periferico.
5. O loop principal continua piscando o LED.

## 4. Conceitos de Rust que aparecem neste codigo

Antes do walkthrough, vale entender as estruturas da linguagem que aparecem aqui.

## 4.1. Atributos: `#![...]` e `#[...]`

Rust usa atributos para dar instrucoes especiais ao compilador.

Exemplos deste codigo:

- `#![no_std]`
- `#![no_main]`
- `#[embassy_executor::task]`
- `#[embassy_executor::main]`
- `#[pre_init]`
- `#[link_section = "..."]`

### `#![no_std]`

Desliga a biblioteca padrao `std`.

Por que isso e usado?
- em microcontroladores normalmente nao existe sistema operacional
- nao ha heap, arquivos, sockets e outras facilidades da `std`

Entao o codigo usa `core` e crates especificos para embarcados.

### `#![no_main]`

Significa que o programa nao usa a `main` padrao de programas desktop.

Quem controla a inicializacao e o runtime embarcado (`cortex-m-rt` + Embassy).

### `#[embassy_executor::task]`

Marca uma funcao async como tarefa gerenciada pelo executor Embassy.

### `#[embassy_executor::main]`

Marca a funcao principal async do firmware.

### `#[pre_init]`

Marca uma funcao que roda muito cedo, antes de `main`, durante a inicializacao do programa.

### `#[link_section = "..."]`

Pede para uma variavel ser colocada em uma secao especifica de memoria pelo linker.

## 4.2. `use`

Linhas de `use` importam nomes para o escopo atual.

Exemplo:

```rust
use embassy_time::Timer;
```

Sem isso, seria preciso escrever o caminho completo sempre.

## 4.3. `mut`

Em Rust, variaveis sao imutaveis por padrao.

Quando voce quer alterar uma variavel, precisa marcar com `mut`.

Exemplo:

```rust
let mut buffer = [0u8; 1];
```

Isso significa: `buffer` pode ser modificado.

## 4.4. `const` e `static`

### `const`

Define um valor constante conhecido em tempo de compilacao.

Exemplo:

```rust
const ADDRESS: u8 = 0x53;
```

### `static`

Define uma variavel global com localizacao fixa na memoria.

Exemplo:

```rust
static TESTE: i32 = 60;
```

Em firmware, `static` aparece bastante porque muitas coisas vivem durante toda a execucao do programa.

## 4.5. Tipos explicitos

Rust tem inferencia de tipos, mas neste codigo muitos tipos aparecem explicitamente, especialmente em funcoes genericas de perifericos.

Exemplo:

```rust
async fn uart_task(mut lpuart: Uart<'static, embassy_stm32::mode::Async>)
```

Isso indica com precisao que tipo a funcao recebe.

## 4.6. Lifetimes, como `'static`

`'static` significa que o dado pode viver durante toda a execucao do programa.

No contexto de Embassy, tarefas spawnadas geralmente exigem dados com lifetime `'static`, porque o executor pode mantelas vivas indefinidamente.

## 4.7. Generics

Rust permite escrever tipos parametrizados.

Exemplo mais pesado deste codigo:

```rust
Adxl345Async<I2cBus<I2c<'static, embassy_stm32::mode::Async, embassy_stm32::i2c::mode::Master>>>
```

Isso parece assustador no inicio, mas a ideia e simples:
- existe um driver `Adxl345Async`
- ele depende de um barramento `I2cBus`
- esse barramento usa um `I2c` especifico do STM32

Ou seja, o tipo descreve exatamente de que implementacao concreta o driver depende.

## 4.8. `async` e `await`

Esses dois conceitos sao centrais no codigo.

### `async fn`

Declara uma funcao assincrona.

Ela pode pausar sem bloquear tudo.

### `.await`

Espera uma operacao assincrona terminar.

Exemplo:

```rust
Timer::after_millis(500).await;
```

Aqui a tarefa espera 500 ms, mas o executor pode rodar outras tarefas nesse meio tempo.

## 4.9. `loop`

`loop` cria um laco infinito.

Em firmware isso e normal, porque o programa costuma rodar para sempre.

## 4.10. `match`

`match` faz casamento de padroes.

Exemplo:

```rust
match i2c.write_read(...).await {
    Ok(()) => { ... }
    Err(e) => error!(...)
}
```

Isso significa:
- se der certo, execute um bloco
- se der erro, execute outro

## 4.11. `if let`

Forma compacta de tratar apenas um caso de um enum.

Exemplo:

```rust
if let Ok((x, y, z)) = accel.get_accel().await {
    ...
}
```

Se o retorno for `Ok(...)`, ele extrai os valores; senao, ignora.

## 4.12. `Result`

Em Rust, erros normalmente sao representados por:

```rust
Result<T, E>
```

Que pode ser:
- `Ok(valor)`
- `Err(erro)`

Isso aparece em varias operacoes de hardware.

## 4.13. Macros: `info!`, `error!`, `println!`, `unwrap!`

Macros terminam com `!`.

No codigo aparecem:
- `info!`
- `error!`
- `println!`
- `unwrap!`

### `info!` e `error!`

Sao macros de log.

### `println!`

Aqui esta vindo do ecossistema de logging embarcado, nao da `std` tradicional.

### `unwrap!`

Usada para extrair um valor de um `Result`. Se vier erro, o programa entra em panic.

Em exemplos didaticos isso e comum para simplificar o codigo.

## 4.14. `unsafe`

Rust tem um modelo forte de seguranca, mas algumas operacoes de baixo nivel exigem `unsafe`.

Neste projeto, isso acontece em `#[pre_init]`, com assembly inline:

- Rust nao consegue provar a seguranca dessa operacao sozinho
- o programador assume a responsabilidade

`unsafe` nao significa que o codigo esta errado. Significa apenas que o compilador exige cuidado extra.

## 5. Walkthrough de `src/main.rs`

Agora vamos percorrer o codigo em blocos.

## 5.1. Cabecalho do crate

Trecho:

```rust
#![no_std]
#![no_main]
```

Interpretacao:
- o programa nao usa a biblioteca padrao
- a inicializacao e controlada pelo ambiente embarcado

Isso e tipico de firmware bare-metal.

## 5.2. Imports

O bloco de imports traz para o arquivo tudo o que sera usado.

Alguns imports merecem destaque.

### `use cortex_m_rt::pre_init;`

Importa o atributo/ferramenta de inicializacao antecipada.

### `use core::arch::asm;`

Importa assembly inline.

Isso e necessario para a rotina que copia dados para secoes customizadas de memoria.

### `use defmt::*;`

Traz macros e utilitarios de logging.

### `use embassy_executor::Spawner;`

O `Spawner` e o objeto usado para iniciar tarefas async.

### Imports de `embassy_stm32`

Esses imports trazem:
- configuracao de clock
- ADC
- GPIO
- UART
- I2C
- EXTI
- timer PWM
- tipos de perifericos

### `use {defmt_rtt as _, panic_probe as _};`

Esse estilo com `as _` e interessante.

Ele importa crates apenas pelos seus efeitos de ligacao/integracao, sem criar nomes para uso direto.

No caso:
- `defmt_rtt` integra logs RTT
- `panic_probe` integra o tratamento de panic

## 5.3. Tarefa `adc_task`

Assinatura:

```rust
async fn adc_task(
    mut adc: Adc<'static, ADC2>,
    mut adc_pin: AnyAdcChannel<'static, ADC2>
)
```

### O que essa tarefa recebe

- um ADC2
- um canal analogico associado a esse ADC

### O que ela faz

Dentro de um `loop` infinito:
- le o valor analogico do pino
- imprime o valor no log
- espera 500 ms

Trecho principal:

```rust
let measured = adc.blocking_read(&mut adc_pin, SampleTime::CYCLES247_5);
defmt::info!("ADC Valor: {}", measured);
embassy_time::Timer::after_millis(500).await;
```

### Conceitos importantes aqui

- `blocking_read`: a leitura e bloqueante
- `&mut adc_pin`: emprestimo mutavel do canal
- `SampleTime::CYCLES247_5`: tempo de amostragem do ADC
- `.await`: a tarefa cede o controle ao executor durante o delay

Observacao didatica: embora o projeto use async, a leitura do ADC nesse ponto e bloqueante. Isso simplifica o exemplo, mas em sistemas mais sofisticados essa escolha pode afetar latencia.

## 5.4. Tarefa `button_task`

Assinatura:

```rust
async fn button_task(mut button: ExtiInput<'static, embassy_stm32::mode::Async>)
```

### O que e EXTI

EXTI e o mecanismo de interrupcao externa dos pinos GPIO.

### O que a tarefa faz

- espera borda de subida
- registra `Pressed!`
- espera borda de descida
- registra `Released!`

Trecho central:

```rust
button.wait_for_rising_edge().await;
button.wait_for_falling_edge().await;
```

Isso e um otimo exemplo de async bem natural em firmware: a tarefa fica parada esperando evento de hardware, sem consumir CPU em loop ativo.

## 5.5. Tarefa `uart_task`

Assinatura:

```rust
async fn uart_task(mut lpuart: Uart<'static, embassy_stm32::mode::Async>)
```

### O que ela faz

1. envia uma mensagem inicial
2. cria um buffer de 1 byte
3. entra em loop infinito
4. le 1 byte da UART
5. escreve o mesmo byte de volta

Isso caracteriza um eco serial.

Trechos importantes:

```rust
lpuart.write("UART started, type something...".as_bytes()).await.unwrap();
let mut buffer = [0u8; 1];
lpuart.read(&mut buffer).await.unwrap();
lpuart.write(&buffer).await.unwrap();
```

### Conceitos de Rust aqui

- string literal: `"..."`
- conversao para bytes: `.as_bytes()`
- array fixo: `[0u8; 1]`
- `unwrap()`: assume sucesso, senao panic

## 5.6. Tarefa `pwm_task`

Assinatura:

```rust
async fn pwm_task(mut pwm: SimplePwm<'static, embassy_stm32::peripherals::TIM1>)
```

### O que e PWM

PWM significa Pulse Width Modulation.

Ele gera um sinal digital pulsante cujo duty cycle pode variar.

### O que a tarefa faz

Ela pega o canal 1 do PWM (`ch1`) e varia o duty cycle em niveis diferentes:
- totalmente desligado
- 25%
- 50%
- quase 100%

Trecho:

```rust
let mut ch1 = pwm.ch1();
ch1.enable();
```

Depois, no loop:

```rust
ch1.set_duty_cycle_fully_off();
Timer::after_millis(300).await;
ch1.set_duty_cycle_fraction(1, 4);
Timer::after_millis(300).await;
ch1.set_duty_cycle_fraction(1, 2);
Timer::after_millis(300).await;
ch1.set_duty_cycle(ch1.max_duty_cycle() - 1);
Timer::after_millis(300).await;
```

Esse trecho e bom para entender como um timer controla saida PWM.

## 5.7. Tarefa `accel_task`

Essa e a assinatura mais pesada do arquivo:

```rust
async fn accel_task(mut accel: Adxl345Async<I2cBus<I2c<'static, embassy_stm32::mode::Async, embassy_stm32::i2c::mode::Master>>>)
```

No nivel conceitual, isso significa apenas:

- a funcao recebe um driver async do ADXL345
- esse driver usa um barramento I2C
- esse barramento usa o I2C async do STM32

### O que a tarefa faz

1. configura o range para `G2`
2. configura taxa de dados para `100 Hz`
3. entra em loop
4. le aceleracao
5. loga os valores `x`, `y`, `z`
6. espera 1 segundo

Trecho:

```rust
let _ = accel.set_range(Range::G2).await;
let _ = accel.set_data_rate(DataRate::Rate100Hz).await;
```

O padrao `let _ = ...` significa: execute a operacao e descarte o valor de retorno.

Depois:

```rust
if let Ok((x, y, z)) = accel.get_accel().await {
    info!("ADXL345 Accel Raw: x={}, y={}, z={}", x, y, z);
}
```

### O que `if let Ok((x, y, z))` quer dizer

Se a leitura retornar sucesso (`Ok`), desempacote a tupla `(x, y, z)`.

Se der erro, esse bloco simplesmente nao executa.

## 5.8. `bind_interrupts!`

Trecho:

```rust
bind_interrupts!(struct Irqs {
    LPUART1 => ...
    DMA1_CHANNEL1 => ...
    DMA1_CHANNEL2 => ...
    I2C1_EV => ...
    I2C1_ER => ...
    DMA1_CHANNEL3 => ...
    DMA1_CHANNEL4 => ...
    EXTI15_10 => ...
});
```

### O que isso faz

Esse macro cria uma estrutura chamada `Irqs` associando nomes de interrupcoes de hardware aos handlers corretos do Embassy.

Por que isso e necessario?
- UART async depende de interrupcoes e DMA
- I2C async depende de interrupcoes e DMA
- EXTI depende de interrupcao de linha externa

Sem isso, as APIs async desses perifericos nao saberiam como receber os eventos do hardware.

## 5.9. Variaveis globais e constantes

Trechos:

```rust
#[link_section = ".ccmram"]
static TESTE: i32 = 60;

#[link_section = ".data2"]
static TESTE2: i32 = 70;

const ADDRESS: u8 = 0x53;
const WHOAMI: u8 = 0;
```

### `TESTE` e `TESTE2`

Sao variaveis globais colocadas em secoes especiais.

Objetivo didatico aparente:
- demonstrar uso de memoria especial
- demonstrar copia manual em `pre_init`

### `ADDRESS`

Endereco I2C do ADXL345.

### `WHOAMI`

Registrador usado para identificar o dispositivo.

## 5.10. `#[pre_init] unsafe fn before_main()`

Esse e um dos trechos mais avancados do arquivo.

### O que essa funcao faz

Ela roda antes de `main` e copia dados da flash para secoes customizadas de RAM.

Sem esse tipo de rotina, variaveis colocadas em secoes especiais podem nao ser inicializadas como esperado.

### Por que existe assembly aqui

Porque o autor quer controlar diretamente a copia de dados usando simbolos do linker:

- `__sccmdata`
- `__eccmdata`
- `__siccmdata`
- `__sdata2`
- `__edata2`
- `__sidata2`

Esses simbolos sao definidos no script de linker (`memory.x`).

### Logica do primeiro bloco de assembly

Ele copia os dados da secao de carga na flash para a secao `.ccmdata` em RAM.

### Logica do segundo bloco

Ele copia os dados da secao de carga na flash para a secao `.data2` em `SRAM2`.

### Por que isso e `unsafe`

Porque envolve:
- ponteiros/endereco de memoria
- assembly inline
- pressupostos sobre layout de memoria

Rust nao consegue verificar isso automaticamente.

### Observacao importante

Em `memory.x`, a secao customizada aparece como `.ccmdata`, enquanto em `main.rs` a variavel `TESTE` usa `#[link_section = ".ccmram"]`.

Isso parece uma diferenca relevante para revisar com o professor, porque o nome da secao no codigo e o nome da secao no linker normalmente precisam estar alinhados para o comportamento ficar exatamente como o esperado.

## 5.11. Funcao principal `main`

Assinatura:

```rust
#[embassy_executor::main]
async fn main(spawner: Spawner)
```

### O que significa

- essa e a funcao principal do firmware
- ela e async
- recebe um `Spawner` para iniciar tarefas

## 5.12. Configuracao de clock

Trecho resumido:

```rust
let mut config = Config::default();
{
    use embassy_stm32::rcc::*;
    config.rcc.hse = Some(Hse {
        freq: Hertz(24_000_000),
        mode: HseMode::Oscillator,
    });
    config.rcc.pll = Some(Pll {
        source: PllSource::HSE,
        prediv: PllPreDiv::DIV6,
        mul: PllMul::MUL85,
        divp: None,
        divq: None,
        divr: Some(PllRDiv::DIV2),
    });
    config.rcc.sys = Sysclk::PLL1_R;
    config.rcc.mux.adc12sel = mux::Adcsel::SYS;
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV1;
    config.rcc.apb2_pre = APBPrescaler::DIV1;
}
```

### O que isso faz

Configura o clock do chip.

Pontos principais:
- usa HSE externo de 24 MHz
- configura PLL
- seleciona a saida da PLL como clock do sistema
- mantem prescalers em divisao 1

### Por que isso importa

Quase todos os perifericos dependem do clock correto:
- UART depende do clock para baudrate
- timers dependem do clock para PWM
- ADC depende do clock para temporizacao
- I2C tambem depende do clock para frequencia correta

## 5.13. Inicializacao dos perifericos STM32

Trecho:

```rust
let mut p: embassy_stm32::Peripherals = embassy_stm32::init(config);
```

Isso inicializa o HAL com a configuracao definida e devolve a estrutura `Peripherals`, que contem os perifericos e pinos do chip.

Voce pode pensar em `p` como a "caixa" com todos os recursos de hardware disponiveis.

Depois o codigo consome itens dessa caixa, por exemplo:
- `p.PA5`
- `p.PC13`
- `p.ADC2`
- `p.LPUART1`
- `p.I2C1`

Em Rust, isso e importante: cada periferico costuma ter dono unico. Quando um periferico e passado para uma API, ele deixa de poder ser usado em outro lugar de forma indevida.

## 5.14. Logs iniciais e leitura de variaveis especiais

Trecho:

```rust
info!("Hello World!");
println!("Teste de variavel na memoria CCMRAM {}", TESTE);
println!("Teste de variavel na memoria SRAM2 {}", TESTE2);
```

Objetivo:
- provar que o firmware iniciou
- mostrar o valor das variaveis globais em secoes especiais

## 5.15. Botao com EXTI

Trecho:

```rust
let button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down, Irqs);
spawner.spawn(unwrap!(button_task(button)));
```

### O que acontece aqui

1. o pino `PC13` vira entrada com interrupcao externa
2. o pull-down e habilitado
3. a tarefa `button_task` e iniciada

### Sobre `spawner.spawn(...)`

`spawn` agenda a tarefa para ser executada pelo Embassy.

O retorno e tratado com `unwrap!`, assumindo que a criacao da tarefa vai funcionar.

## 5.16. ADC externo e sensor de temperatura interna

Trechos:

```rust
let mut adc = Adc::new(p.ADC2, Default::default());
let mut adc_temp = Adc::new(p.ADC1, Default::default());
let mut temperature = adc_temp.enable_temperature();
```

Aqui o codigo cria:
- um ADC2 para ler o pino `PA7`
- um ADC1 para ler a temperatura interna do chip

Depois ele faz leituras iniciais bloqueantes:

```rust
let measured = adc.blocking_read(&mut p.PA7, SampleTime::CYCLES247_5);
let temp = adc_temp.blocking_read(&mut temperature, SampleTime::CYCLES247_5);
```

Essas leituras servem como demonstracao imediata antes de criar a tarefa periodica de ADC.

Em seguida:

```rust
let adc_channel = p.PA7.degrade_adc();
spawner.spawn(unwrap!(adc_task(adc, adc_channel)));
```

### O que `degrade_adc()` faz

Converte o pino em uma forma mais generica de canal ADC, adequada para passar a APIs mais abstratas.

## 5.17. UART assincrona

Trecho:

```rust
let mut config = usart::Config::default();
config.baudrate = 115_200;
let lpusart = Uart::new(p.LPUART1, p.PA3, p.PA2, p.DMA1_CH1, p.DMA1_CH2, Irqs, config).unwrap();
spawner.spawn(unwrap!(uart_task(lpusart)));
```

### O que isso configura

- periferico `LPUART1`
- pino RX/TX
- canais DMA
- interrupcoes
- baudrate de `115200`

### O que isso mostra sobre Rust embarcado

As APIs costumam ser bem explicitas: voce precisa fornecer exatamente os recursos de hardware que serao usados.

Isso ajuda a evitar configuracoes ambiguas ou conflitos silenciosos.

## 5.18. PWM no TIM1

Trecho:

```rust
let ch1_pin = PwmPin::new(p.PC0, OutputType::PushPull);
let pwm: SimplePwm<'_, embassy_stm32::peripherals::TIM1> =
    SimplePwm::new(p.TIM1, Some(ch1_pin), None, None, None, khz(10), Default::default());
spawner.spawn(unwrap!(pwm_task(pwm)));
```

### O que isso faz

- transforma `PC0` em saida PWM
- usa o timer `TIM1`
- configura frequencia de `10 kHz`
- inicia a tarefa que altera duty cycle

### `Some(...)` e `None`

Isso e outro conceito importante de Rust.

`Option<T>` representa um valor que pode ou nao existir.

- `Some(valor)` = existe valor
- `None` = nao existe

Como esse timer pode ter varios canais, o codigo informa:
- canal 1 existe
- os outros nao estao sendo usados

## 5.19. Configuracao do I2C

Trecho:

```rust
let mut config: i2c::Config = Default::default();
config.scl_pullup = true;
config.sda_pullup = true;
config.frequency = Hertz(100_000);
config.timeout = embassy_time::Duration::from_millis(1000);
```

Aqui a configuracao do I2C e ajustada para:
- pull-up em SCL
- pull-up em SDA
- 100 kHz
- timeout de 1 segundo

Depois o barramento e criado:

```rust
let mut i2c = I2c::new(
    p.I2C1,
    p.PB8,
    p.PB9,
    p.DMA1_CH3,
    p.DMA1_CH4,
    Irqs,
    config,
);
```

## 5.20. Pino `PC9` e leitura do registrador WHOAMI

Trecho:

```rust
let mut i2c_cs = Output::new(p.PC9, Level::Low, Speed::Low);
Timer::after_millis(50).await;

let mut data = [0u8; 1];
i2c_cs.set_high();
Timer::after_millis(5).await;
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
```

### O que isso faz

1. configura `PC9` como saida
2. aguarda um pouco
3. coloca `PC9` em nivel alto
4. le o registrador `WHOAMI` do ADXL345
5. verifica se o valor retornado e `0xE5`

### Sobre `&[WHOAMI]`

Esse trecho cria uma fatia de array com um byte, usada para escrever o endereco do registrador antes de ler a resposta.

## 5.21. Criacao do driver do acelerometro

Trecho:

```rust
let bus = I2cBus::new(i2c, Some(Address::SECONDARY));
let mut accel = Adxl345Async::new(bus);
let _ = accel.setup().await;
```

### O que isso faz

- encapsula o I2C num tipo aceito pelo driver
- cria o driver do ADXL345
- executa a configuracao inicial do sensor

Depois o codigo faz uma leitura de teste:

```rust
match accel.get_accel_raw().await {
    Ok((x, y, z)) => {
        info!("ADXL345 Accel Raw: x={}, y={}, z={}", x, y, z);
    }
    Err(e) => error!("Error reading accel: {:?}", e),
}
```

E finalmente inicia a tarefa periodica:

```rust
spawner.spawn(unwrap!(accel_task(accel)));
```

## 5.22. LED principal

Trecho:

```rust
let mut led = Output::new(p.PA5, Level::High, Speed::Low);

loop {
    led.set_high();
    Timer::after_millis(500).await;
    led.set_low();
    Timer::after_millis(500).await;
}
```

### O que isso faz

Configura `PA5` como saida e alterna o estado do LED a cada 500 ms.

Esse e o loop principal mais visivel do firmware.

Mesmo com esse loop infinito, as outras tarefas continuam funcionando porque o Embassy intercala execucao quando as tarefas chegam em pontos de espera (`await`).

## 6. Ordem real de execucao do programa

Uma boa forma de consolidar o entendimento e visualizar a ordem dos eventos.

1. O firmware inicia.
2. `before_main()` roda e copia dados para secoes customizadas.
3. `main()` comeca.
4. O clock do chip e configurado.
5. `embassy_stm32::init(config)` inicializa os perifericos.
6. Logs iniciais sao emitidos.
7. Botao EXTI e configurado e sua tarefa e spawnada.
8. ADC e temperatura interna sao lidos.
9. Tarefa periodica de ADC e spawnada.
10. UART e configurada e sua tarefa de eco e spawnada.
11. PWM e configurado e sua tarefa e spawnada.
12. I2C e configurado.
13. O ADXL345 e testado e configurado.
14. A tarefa periodica do acelerometro e spawnada.
15. O loop principal passa a piscar o LED para sempre.

## 7. O que este codigo ensina sobre Rust

Este exemplo ensina varias ideias importantes da linguagem e do ecossistema embarcado.

## Ownership

Os perifericos saem de `p` e passam a ter dono unico.

Exemplo:
- quando `p.LPUART1` e passado para `Uart::new`, ele deixa de estar disponivel para uso em outro lugar

Isso evita conflito de uso duplo de hardware.

## Borrowing

Quando o codigo faz algo como `&mut adc_pin`, ele empresta acesso mutavel temporario.

## Tipagem forte

Os tipos longos parecem verbosos, mas deixam muito claro:
- qual periferico esta sendo usado
- em qual modo ele esta
- que combinacao de componentes forma o driver final

## Seguranca por padrao

A maior parte do codigo e segura por padrao.

Somente a parte realmente sensivel de baixo nivel usa `unsafe`.

## Async cooperativo

O firmware consegue lidar com varias tarefas sem um sistema operacional tradicional.

Isso acontece porque cada tarefa espera eventos ou timers usando `await`.

## 8. O que este codigo ensina sobre firmware

Tambem ha varias licoes importantes de sistemas embarcados.

## Clock importa muito

Configuracao de clock errada afeta praticamente tudo.

## Interrupcoes sao fundamentais

As APIs async de UART, I2C e EXTI dependem de interrupcoes corretamente ligadas.

## Memoria embarcada nao e so "RAM e flash"

O projeto mostra areas especiais como `SRAM2` e `CCMRAM`.

## Hardware define o software

Os pinos escolhidos e o chip alvo determinam fortemente a forma do programa.

## 9. Dificuldades comuns para iniciantes

Se este arquivo parece complexo, isso e normal. Os pontos que mais costumam travar iniciantes sao estes.

## Tipos muito longos

Nao tente decorar tudo de uma vez. Leia de fora para dentro.

Exemplo:
- `Adxl345Async<...>` = driver do sensor
- `I2cBus<...>` = barramento usado pelo driver
- `I2c<...>` = implementacao concreta do barramento

## Macros

Lembre-se: tudo com `!` e macro, nao funcao normal.

## `unsafe`

Nao precisa dominar assembly para entender o objetivo geral. Primeiro foque na intencao:
- copiar dados para secoes de memoria especiais antes de `main`

## Async

Embarcado async nao e a mesma coisa que multithreading de desktop. Aqui o modelo e mais leve e cooperativo.

## 10. Como estudar este codigo de forma eficiente

Se voce quiser realmente dominar o exemplo, estude nesta ordem.

1. Entenda o que `main()` faz em alto nivel.
2. Entenda cada tarefa separadamente.
3. Entenda como `spawn` conecta as tarefas ao executor.
4. Entenda `bind_interrupts!`.
5. Depois estude `#[pre_init]` e `memory.x`.

Essa ordem ajuda porque o trecho de memoria customizada e o mais avancado do projeto.

## 11. Resumo final em linguagem simples

Este firmware:
- configura o microcontrolador STM32
- inicia varios perifericos
- roda varias tarefas async em paralelo cooperativo
- le ADC
- detecta botao
- faz eco pela UART
- gera PWM
- conversa com um ADXL345 via I2C
- pisca um LED continuamente

Em termos de Rust, ele demonstra:
- atributos especiais de firmware
- imports
- variaveis mutaveis
- constantes e estaticos
- tipos genericos
- lifetimes
- funcoes async
- `await`
- `loop`
- `match`
- `if let`
- `Result`
- `Option`
- macros
- `unsafe`

Se voce entender bem este arquivo, ja tera contato com varios dos fundamentos mais importantes de Rust embarcado.
